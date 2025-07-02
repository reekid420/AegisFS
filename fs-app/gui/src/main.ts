import { invoke } from "@tauri-apps/api/core";
import WebSocket from '@tauri-apps/plugin-websocket';
import { Chart, ChartConfiguration, registerables } from 'chart.js';
import { formatDistanceToNow } from 'date-fns';
import './styles.css';

// Tauri type declarations
declare global {
  interface Window {
    __TAURI_INTERNALS__?: any;
  }
}

// Register Chart.js components
Chart.register(...registerables);

// Type definitions
interface FilesystemInstance {
  id: string;
  device: string;
  mount_point: string;
  status: string;
}

interface ApiResponse<T> {
  success: boolean;
  data?: T;
  error?: string;
}

interface IoStats {
  read_rate: number;
  write_rate: number;
  iops: number;
  cache_hit_ratio: number;
}

interface SnapshotInfo {
  id: string;
  name: string;
  created_at: string;
  size: number;
  state: string;
  files_changed: number;
}

interface WebSocketMessage {
  type: string;
  data: string | ArrayBuffer;
}

// Global state
let ws: WebSocket | null = null;
let ioChart: Chart | null = null;
let storageChart: Chart | null = null;
let currentInstance: string = '/dev/nvme0n1p6';
let reconnectAttempts = 0;
const MAX_RECONNECT_ATTEMPTS = 5;

// Initialize the application
document.addEventListener('DOMContentLoaded', async () => {
  console.log('DOM Content Loaded - initializing AegisFS GUI');
  console.log('Running in Tauri:', isTauri());
  
  // Hide loading message
  const loadingDiv = document.getElementById('loading');
  if (loadingDiv) {
    loadingDiv.style.display = 'none';
  }
  
  try {
    setupTabNavigation();
    setupEventListeners();
    await initializeCharts();
    await loadFilesystemStatus();
    await connectWebSocket();
    startPollingFallback();
    
    console.log('AegisFS GUI initialization complete');
  } catch (error) {
    console.error('Error during initialization:', error);
    if (loadingDiv) {
      loadingDiv.innerHTML = `<div style="padding: 20px; color: red;">
        <h1>Initialization Error</h1>
        <p>Error: ${error}</p>
        <p>Check the developer console for more details.</p>
      </div>`;
      loadingDiv.style.display = 'block';
    }
  }
});

// Add early debugging
console.log('AegisFS main.ts loaded');
console.log('Document ready state:', document.readyState);

// Tab navigation
function setupTabNavigation() {
  const tabButtons = document.querySelectorAll('.tab-button');
  const tabContents = document.querySelectorAll('.tab-content');

  tabButtons.forEach(button => {
    button.addEventListener('click', () => {
      const targetTab = button.getAttribute('data-tab');
      
      // Update active states
      tabButtons.forEach(btn => btn.classList.remove('active'));
      tabContents.forEach(content => content.classList.remove('active'));
      
      button.classList.add('active');
      document.getElementById(`${targetTab}-tab`)?.classList.add('active');
      
      // Load tab-specific data
      loadTabData(targetTab);
    });
  });
}

// Event listeners
function setupEventListeners() {
  // Instance selector
  const instanceDropdown = document.getElementById('instance-dropdown') as HTMLSelectElement;
  instanceDropdown?.addEventListener('change', (e) => {
    currentInstance = (e.target as HTMLSelectElement).value;
    loadFilesystemStatus();
  });

  // Quick actions
  document.getElementById('create-snapshot')?.addEventListener('click', createSnapshot);
  document.getElementById('run-scrub')?.addEventListener('click', runScrub);
  document.getElementById('toggle-mount')?.addEventListener('click', toggleMount);

  // Snapshot actions
  document.getElementById('new-snapshot')?.addEventListener('click', () => {
    createSnapshot();
  });

  // Status bar buttons
  document.getElementById('show-logs')?.addEventListener('click', () => {
    switchToTab('logs');
  });

  document.getElementById('show-about')?.addEventListener('click', () => {
    showAboutDialog();
  });
}

// Initialize charts
async function initializeCharts() {
  // I/O Statistics Chart
  const ioCtx = document.getElementById('io-chart') as HTMLCanvasElement;
  if (ioCtx) {
    const ioConfig: ChartConfiguration = {
      type: 'line',
      data: {
        labels: [],
        datasets: [
          {
            label: 'Read MB/s',
            data: [],
            borderColor: '#3b82f6',
            backgroundColor: 'rgba(59, 130, 246, 0.1)',
            tension: 0.4
          },
          {
            label: 'Write MB/s',
            data: [],
            borderColor: '#10b981',
            backgroundColor: 'rgba(16, 185, 129, 0.1)',
            tension: 0.4
          }
        ]
      },
      options: {
        responsive: true,
        maintainAspectRatio: false,
        animation: {
          duration: 400,
          easing: 'easeInOutQuart'
        },
        interaction: {
          intersect: false
        },
        plugins: {
          legend: {
            display: true,
            position: 'bottom'
          }
        },
        scales: {
          x: {
            display: false
          },
          y: {
            beginAtZero: true,
            suggestedMax: 300,
            title: {
              display: true,
              text: 'MB/s'
            },
            grid: {
              color: 'rgba(255, 255, 255, 0.1)'
            }
          }
        }
      }
    };
    ioChart = new Chart(ioCtx, ioConfig);
  }

  // Storage Usage Chart
  const storageCtx = document.getElementById('storage-chart') as HTMLCanvasElement;
  if (storageCtx) {
    const storageConfig: ChartConfiguration = {
      type: 'doughnut',
      data: {
        labels: ['Used', 'Free'],
        datasets: [{
          data: [0, 100],
          backgroundColor: ['#3b82f6', '#e5e7eb'],
          borderWidth: 0
        }]
      },
      options: {
        responsive: true,
        maintainAspectRatio: false,
        plugins: {
          legend: {
            display: true,
            position: 'bottom'
          }
        }
      }
    };
    storageChart = new Chart(storageCtx, storageConfig);
  }
}

// WebSocket connection
async function connectWebSocket() {
  if (!isTauri()) {
    console.log('Web mode: Skipping WebSocket connection');
    updateConnectionStatus('Web development mode');
    return;
  }

  try {
    ws = await WebSocket.connect('ws://localhost:8080/api/events', {
      headers: {}
    });

    ws.addListener((message) => {
      if (message.type === 'Text' && typeof message.data === 'string') {
        const event = JSON.parse(message.data);
        handleRealtimeEvent(event);
      }
    });

    reconnectAttempts = 0;
    updateConnectionStatus('Connected via WebSocket');
  } catch (error) {
    console.warn('WebSocket connection failed, falling back to polling', error);
    updateConnectionStatus('Connected via polling');
  }
}

// Handle real-time events
function handleRealtimeEvent(event: any) {
  switch (event.type) {
    case 'io_stats':
      updateIoStats(event.data);
      break;
    case 'status_change':
      updateFilesystemStatus(event.data);
      break;
    case 'snapshot_created':
      addSnapshotToList(event.data);
      break;
    case 'scrub_progress':
      updateScrubProgress(event.data);
      break;
    default:
      console.log('Unknown event type:', event.type);
  }
}

// Polling fallback (500ms as specified)
function startPollingFallback() {
  if (!ws) {
    setInterval(async () => {
      await updateStats();
    }, 500);
  }
}

// Check if we're running in Tauri
function isTauri(): boolean {
  return typeof window !== 'undefined' && window.__TAURI_INTERNALS__ !== undefined;
}

// API calls
async function loadFilesystemStatus() {
  if (!isTauri()) {
    console.log('Running in web mode - using mock data');
    // Use mock data for web development
    const mockInstance: FilesystemInstance = {
      id: '1',
      device: '/dev/nvme0n1p6',
      mount_point: '/mnt/aegisfs',
      status: 'online'
    };
    updateFilesystemInfo(mockInstance);
    return;
  }

  try {
    const response = await invoke<ApiResponse<FilesystemInstance[]>>('get_status');
    
    if (response.success && response.data) {
      const instance = response.data.find(fs => fs.device === currentInstance);
      if (instance) {
        updateFilesystemInfo(instance);
      }
    }
  } catch (error) {
    console.error('Failed to load filesystem status:', error);
  }
}

async function updateStats() {
  if (!isTauri()) {
    // Use mock data for web development with more dynamic variations
    const time = Date.now() / 1000;
    const mockStats = {
      read_rate: 50 + 100 * Math.sin(time / 10) + 50 * Math.random(),
      write_rate: 30 + 80 * Math.cos(time / 8) + 40 * Math.random(),
      iops: Math.floor(1000 + 3000 * Math.sin(time / 15) + 1000 * Math.random()),
      cache_hit_ratio: 0.85 + 0.1 * Math.sin(time / 20) + 0.05 * Math.random()
    };
    updateIoStats(mockStats);
    return;
  }

  try {
    const response = await invoke<ApiResponse<any>>('get_stats');
    
    if (response.success && response.data) {
      updateIoStats(response.data);
    }
  } catch (error) {
    console.error('Failed to update stats:', error);
  }
}

async function createSnapshot() {
  const name = prompt('Enter snapshot name:');
  if (!name) return;

  if (!isTauri()) {
    console.log('Web mode: Mock snapshot creation for', name);
    showNotification('Snapshot created successfully (mock)', 'success');
    await loadSnapshots();
    return;
  }

  try {
    const response = await invoke<ApiResponse<string>>('create_snapshot', {
      name,
      description: `Created from GUI at ${new Date().toLocaleString()}`
    });

    if (response.success) {
      showNotification('Snapshot created successfully', 'success');
      await loadSnapshots();
    } else {
      showNotification(response.error || 'Failed to create snapshot', 'error');
    }
  } catch (error) {
    console.error('Failed to create snapshot:', error);
    showNotification('Failed to create snapshot', 'error');
  }
}

async function runScrub() {
  if (!confirm('Start filesystem scrub? This may take a while.')) return;

  try {
    // TODO: Implement scrub API call
    showNotification('Scrub started', 'info');
  } catch (error) {
    console.error('Failed to start scrub:', error);
    showNotification('Failed to start scrub', 'error');
  }
}

async function toggleMount() {
  const button = document.getElementById('toggle-mount');
  const isCurrentlyMounted = button?.textContent === 'Unmount';

  try {
    // TODO: Implement mount/unmount API call
    if (isCurrentlyMounted) {
      showNotification('Filesystem unmounted', 'success');
      button!.textContent = 'Mount';
    } else {
      showNotification('Filesystem mounted', 'success');
      button!.textContent = 'Unmount';
    }
  } catch (error) {
    console.error('Failed to toggle mount:', error);
    showNotification('Failed to toggle mount', 'error');
  }
}

// Tab-specific data loading
async function loadTabData(tab: string | null) {
  switch (tab) {
    case 'snapshots':
      await loadSnapshots();
      break;
    case 'logs':
      await loadLogs();
      break;
    case 'settings':
      await loadSettings();
      break;
  }
}

async function loadSnapshots() {
  if (!isTauri()) {
    // Mock snapshots for web development
    const now = new Date();
    const yesterday = new Date(now.getTime() - 24 * 60 * 60 * 1000);
    const lastWeek = new Date(now.getTime() - 7 * 24 * 60 * 60 * 1000);
    
    const mockSnapshots: SnapshotInfo[] = [
      {
        id: 'snapshot_' + now.getTime(),
        name: 'Daily backup',
        created_at: now.toISOString(),
        size: 2100000000,
        state: 'ready',
        files_changed: 127
      },
      {
        id: 'snapshot_' + yesterday.getTime(),
        name: 'Pre-update backup',
        created_at: yesterday.toISOString(),
        size: 1950000000,
        state: 'ready',
        files_changed: 89
      },
      {
        id: 'snapshot_' + lastWeek.getTime(),
        name: 'Weekly backup',
        created_at: lastWeek.toISOString(),
        size: 1850000000,
        state: 'ready',
        files_changed: 245
      }
    ];
    displaySnapshots(mockSnapshots);
    return;
  }

  try {
    const response = await invoke<ApiResponse<SnapshotInfo[]>>('list_snapshots');
    
    if (response.success && response.data) {
      displaySnapshots(response.data);
    }
  } catch (error) {
    console.error('Failed to load snapshots:', error);
  }
}

async function loadLogs() {
  // TODO: Implement log loading
  const logContent = document.getElementById('log-content');
  if (logContent) {
    logContent.innerHTML = '<div class="log-entry">[2024-12-30 15:30:25] INFO: System initialized</div>';
  }
}

async function loadSettings() {
  // TODO: Implement settings loading
}

// UI update functions
function updateFilesystemInfo(instance: FilesystemInstance) {
  updateElementText('device-path', instance.device);
  updateElementText('mount-point', instance.mount_point);
  
  const statusBadge = document.querySelector('.status-badge');
  if (statusBadge) {
    statusBadge.textContent = instance.status;
    statusBadge.className = `status-badge ${instance.status.toLowerCase()}`;
  }
}

function updateIoStats(stats: IoStats) {
  // Update text displays
  updateElementText('read-rate', `${stats.read_rate.toFixed(1)} MB/s`);
  updateElementText('write-rate', `${stats.write_rate.toFixed(1)} MB/s`);
  updateElementText('iops', stats.iops.toString());

  // Update chart
  if (ioChart && ioChart.data.labels) {
    const now = new Date().toLocaleTimeString();
    ioChart.data.labels.push(now);
    ioChart.data.datasets[0].data.push(stats.read_rate);
    ioChart.data.datasets[1].data.push(stats.write_rate);

    // Keep only last 20 data points
    if (ioChart.data.labels.length > 20) {
      ioChart.data.labels.shift();
      ioChart.data.datasets[0].data.shift();
      ioChart.data.datasets[1].data.shift();
    }

    ioChart.update('active');
  }
}

function displaySnapshots(snapshots: SnapshotInfo[]) {
  const timeline = document.getElementById('snapshot-timeline');
  if (!timeline) return;

  // Group snapshots by date
  const grouped = snapshots.reduce((acc, snapshot) => {
    try {
      const date = new Date(snapshot.created_at);
      if (isNaN(date.getTime())) {
        console.warn('Invalid date for snapshot:', snapshot.created_at);
        return acc;
      }
      const dateStr = date.toDateString();
      if (!acc[dateStr]) acc[dateStr] = [];
      acc[dateStr].push(snapshot);
    } catch (error) {
      console.error('Error parsing date:', snapshot.created_at, error);
    }
    return acc;
  }, {} as Record<string, SnapshotInfo[]>);

  // Generate HTML
  let html = '';
  for (const [date, snaps] of Object.entries(grouped)) {
    html += `<div class="snapshot-group">
      <h4>${formatDateGroup(date)}</h4>`;
    
    for (const snap of snaps) {
      const snapDate = new Date(snap.created_at);
      const timeStr = isNaN(snapDate.getTime()) ? 'Invalid Date' : snapDate.toLocaleTimeString();
      html += `
        <div class="snapshot-item" data-id="${snap.id}">
          <span class="snapshot-time">${timeStr}</span>
          <span class="snapshot-name">${snap.name}</span>
          <span class="snapshot-size">${formatBytes(snap.size)}</span>
        </div>`;
    }
    
    html += '</div>';
  }

  timeline.innerHTML = html;

  // Add click handlers
  timeline.querySelectorAll('.snapshot-item').forEach(item => {
    item.addEventListener('click', () => {
      const id = item.getAttribute('data-id');
      if (id) showSnapshotDetails(id);
  });
});
}

// Helper functions
function updateElementText(id: string, text: string) {
  const element = document.getElementById(id);
  if (element) element.textContent = text;
}

function updateConnectionStatus(status: string) {
  updateElementText('connection-status', `Status: ${status}`);
}

function switchToTab(tabName: string) {
  const button = document.querySelector(`[data-tab="${tabName}"]`) as HTMLElement;
  button?.click();
}

function showNotification(message: string, type: 'success' | 'error' | 'info') {
  // TODO: Implement toast notifications
  console.log(`[${type.toUpperCase()}] ${message}`);
}

function showAboutDialog() {
  // TODO: Implement about dialog
  alert('AegisFS Management v0.1.0\n\nA modern filesystem management interface');
}

function formatBytes(bytes: number): string {
  const units = ['B', 'KB', 'MB', 'GB', 'TB'];
  let value = bytes;
  let unitIndex = 0;
  
  while (value >= 1024 && unitIndex < units.length - 1) {
    value /= 1024;
    unitIndex++;
  }
  
  return `${value.toFixed(1)} ${units[unitIndex]}`;
}

function formatDateGroup(date: string): string {
  const today = new Date().toDateString();
  const yesterday = new Date(Date.now() - 86400000).toDateString();
  
  if (date === today) return 'Today';
  if (date === yesterday) return 'Yesterday';
  return date;
}

function showSnapshotDetails(snapshotId: string) {
  // TODO: Load and display snapshot details
  const detailsDiv = document.getElementById('snapshot-info');
  if (detailsDiv) {
    detailsDiv.innerHTML = `
      <h4>Snapshot Details</h4>
      <p>Loading snapshot ${snapshotId}...</p>
    `;
  }
}

function addSnapshotToList(snapshot: SnapshotInfo) {
  // TODO: Add new snapshot to the list without full reload
  loadSnapshots();
}

function updateScrubProgress(progress: any) {
  // TODO: Update scrub progress in UI
  console.log('Scrub progress:', progress);
}

function updateFilesystemStatus(status: any) {
  // TODO: Update filesystem status
  loadFilesystemStatus();
}
