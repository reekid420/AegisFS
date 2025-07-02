# AegisFS GUI Design Plan

**Framework:** Tauri + HTML/CSS/JS Frontend + Rust Backend
**Target Platforms:** Linux, Windows, macOS
**Design Philosophy:** Modern filesystem management with native desktop feel

---

## 1. Overall Architecture & Design Philosophy

### Technical Foundation
- **Tauri Framework**: Rust backend + web frontend for small binaries (~10-40MB)
- **Frontend**: HTML/CSS/JS with modern web standards
- **Backend Integration**: Shared API endpoints with CLI tool
- **Cross-platform**: Single codebase for all desktop platforms

### Design Principles
- **Native Look & Feel**: Follow platform-specific UI conventions
- **Clean & Modern**: Inspired by system administration dashboards and file managers
- **Information Density**: Balance between detail and readability
- **Performance-First**: Responsive UI that doesn't block on long operations
- **Context-Aware**: Show relevant information based on current state

---

## 2. Main Window Layout

### 2.1 Window Structure
```
┌─────────────────────────────────────────────────────┐
│ [≡] AegisFS Management              [─] [□] [×]     │ ← Title Bar
├─────────────────────────────────────────────────────┤
│ Dashboard | Snapshots | Tiering | Backup | Logs | Settings | Help │ ← Top Navigation Tabs
├─────────────────────────────────────────────────────┤
│                                                     │
│                 Main Content Area                   │
│                                                     │
│                                                     │
├─────────────────────────────────────────────────────┤
│ Status: Connected to /dev/nvme0n1p6 │ [Logs] [About] │ ← Status Bar
└─────────────────────────────────────────────────────┘
```

### 2.2 Native Platform Adaptations
- **macOS**: Transparent title bar with traffic lights, native vibrancy
- **Windows**: Custom title bar with Windows 11 styling
- **Linux**: GTK-style window decorations

---

## 3. Tab-by-Tab Design Specifications

### 3.1 Dashboard Tab (Main Overview)

**Purpose**: System overview and real-time monitoring

**Layout**: Grid-based dashboard with cards

```
┌─────────────┬─────────────┬─────────────┐
│ Filesystem  │  I/O Stats  │ Health      │
│ Overview    │             │ Status      │
│             │             │             │
├─────────────┼─────────────┼─────────────┤
│ Recent      │ Storage     │ Quick       │
│ Activity    │ Usage       │ Actions     │
│             │             │             │
└─────────────┴─────────────┴─────────────┘
```

**Cards Include**:
1. **Filesystem Overview**
   - Mount point and device info
   - Filesystem size and usage
   - Online/offline status indicator

2. **I/O Statistics** 
   - Real-time read/write graphs
   - IOPS and throughput metrics
   - Cache hit ratios

3. **Health Status**
   - Checksum verification status
   - Last scrub information
   - Error counts and alerts

4. **Recent Activity**
   - Latest snapshots
   - Recent file operations
   - System events log

5. **Storage Usage**
   - Visual disk usage (pie chart or treemap)
   - Free space alerts
   - Growth trends

6. **Quick Actions**
   - Create snapshot button
   - Run scrub button
   - Mount/unmount toggle

### 3.2 Snapshots Tab

**Purpose**: Snapshot management and browsing

**Layout**: Master-detail with timeline

```
┌─────────────────┬─────────────────────────────┐
│ Snapshot List   │ Snapshot Details            │
│                 │                             │
│ ┌─────────────┐ │ Name: snapshot_2024_12_30   │
│ │ Today       │ │ Created: 30 Dec 2024, 15:30 │
│ │ ○ 15:30     │ │ Size: 2.1 GB               │
│ │ ○ 12:15     │ │ State: Ready               │
│ │             │ │                             │
│ │ Yesterday   │ │ ┌─────────────────────────┐ │
│ │ ○ 18:45     │ │ │ Actions                 │ │
│ │ ○ 09:00     │ │ │ [Browse] [Rollback]     │ │
│ │             │ │ │ [Delete] [Clone]        │ │
│ └─────────────┘ │ └─────────────────────────┘ │
│                 │                             │
│ [Create New]    │ Files Changed: 127          │
└─────────────────┴─────────────────────────────┘
```

**Features**:
- Chronological timeline grouping (Today, Yesterday, Last Week, etc.)
- Snapshot state indicators (Creating, Ready, Error)
- Batch operations (delete multiple snapshots)
- Snapshot browsing in separate window
- Rollback confirmation dialogs
- Auto-snapshot scheduling interface

### 3.3 Tiering Tab

**Purpose**: Storage tiering configuration and monitoring

**Layout**: Tier overview with rules management

```
┌─────────────────────────────────────────────────────┐
│ Storage Tiers Overview                              │
│                                                     │
│ ┌─────────────┐ ┌─────────────┐ ┌─────────────┐     │
│ │ Hot Tier    │ │ Warm Tier   │ │ Cold Tier   │     │
│ │ NVMe SSD    │ │ SATA SSD    │ │ HDD         │     │
│ │ 89% full    │ │ 34% full    │ │ 12% full    │     │
│ │ 1.2TB used  │ │ 856GB used  │ │ 445GB used  │     │
│ └─────────────┘ └─────────────┘ └─────────────┘     │
├─────────────────────────────────────────────────────┤
│ Tiering Rules                           [Add Rule]  │
│                                                     │
│ ┌─ Files older than 30 days ─ Move to Warm ──[×]─┐ │
│ ┌─ Files larger than 1GB ──── Move to Cold ──[×]─┐ │
│ ┌─ .tmp files ─────────────── Keep in Hot ───[×]─┐ │
└─────────────────────────────────────────────────────┘
```

**Features**:
- Visual tier utilization bars
- Drag-and-drop rule creation
- Migration progress monitoring
- Cost analysis per tier
- Performance impact visualization

### 3.4 Settings Tab

**Purpose**: System configuration and preferences

**Layout**: Categorized settings with sidebar navigation

```
┌─────────────┬─────────────────────────────────────┐
│ General     │ Mount Settings                      │
│ Mount       │                                     │
│ Snapshots   │ Device: /dev/nvme0n1p6             │
│ Tiering     │ Mount Point: /mnt/aegisfs          │
│ Advanced    │ ☑ Auto-mount on startup            │
│ Security    │ ☑ Enable compression               │
│ Logging     │ ☐ Read-only mode                   │
│             │                                     │
│             │ Block Size: [4096] bytes           │
│             │ Cache Size: [512] MB               │
│             │                                     │
│             │        [Apply] [Reset]             │
└─────────────┴─────────────────────────────────────┘
```

**Categories**:
- **General**: Theme, startup behavior, notifications
- **Mount**: Device selection, mount options, auto-mount
- **Snapshots**: Auto-snapshot schedules, retention policies
- **Tiering**: Tier definitions, migration thresholds
- **Advanced**: Debug options, cache settings, performance tuning
- **Security**: Encryption settings, access controls
- **Logging**: Log levels, file locations, rotation

### 3.5 Help Tab

**Purpose**: Documentation and system information

**Layout**: Split view with navigation and content

```
┌─────────────┬─────────────────────────────────────┐
│ Contents    │ Getting Started                     │
│             │                                     │
│ Getting     │ Welcome to AegisFS! This guide     │
│ Started     │ will help you set up and manage    │
│             │ your advanced filesystem.          │
│ Snapshots   │                                     │
│             │ ## Mounting a Filesystem            │
│ Tiering     │                                     │
│             │ 1. Select your block device         │
│ Troublesho  │ 2. Choose mount point               │
│ oting       │ 3. Click "Mount"                    │
│             │                                     │
│ About       │ [Next: Creating Snapshots]         │
└─────────────┴─────────────────────────────────────┘
```

**Content Sections**:
- **Getting Started**: Quick setup guide
- **Snapshots**: Snapshot creation and management
- **Tiering**: Storage tier configuration
- **Troubleshooting**: Common issues and solutions
- **About**: Version info, licenses, credits

---

## 4. Visual Design System

### 4.1 Color Palette

**Light Theme**:
- Primary: #2563eb (Blue)
- Secondary: #64748b (Slate)
- Success: #16a34a (Green)
- Warning: #d97706 (Amber)
- Error: #dc2626 (Red)
- Background: #ffffff
- Surface: #f8fafc
- Text: #1e293b

**Dark Theme**:
- Primary: #3b82f6 (Blue)
- Secondary: #64748b (Slate)
- Success: #22c55e (Green)
- Warning: #f59e0b (Amber)
- Error: #ef4444 (Red)
- Background: #0f172a
- Surface: #1e293b
- Text: #f1f5f9

### 4.2 Typography

**Font Stack**: System fonts for native feel
- **macOS**: -apple-system, BlinkMacSystemFont
- **Windows**: "Segoe UI"
- **Linux**: "Ubuntu", "Liberation Sans"
- **Fallback**: sans-serif

**Font Sizes**:
- Title: 24px (1.5rem)
- Heading: 20px (1.25rem)
- Body: 16px (1rem)
- Caption: 14px (0.875rem)
- Small: 12px (0.75rem)

### 4.3 Spacing & Layout

**Grid System**: 8px base unit
- Micro: 4px (0.5 units)
- Small: 8px (1 unit)
- Medium: 16px (2 units)
- Large: 24px (3 units)
- XL: 32px (4 units)

**Component Sizing**:
- Button height: 32px
- Input height: 36px
- Card padding: 16px
- Section spacing: 24px

### 4.4 Icons & Graphics

**Icon System**: 
- 16px and 24px variants
- Consistent stroke width (1.5px)
- Rounded corners where appropriate
- Platform-specific system icons where available

**Status Indicators**:
- 🟢 Online/Healthy
- 🟡 Warning/Degraded
- 🔴 Offline/Error
- 🔵 Info/Processing

---

## 5. Interactive Elements & Behaviors

### 5.1 Navigation

**Tab Switching**:
- Smooth transitions between tabs
- Maintain state across tab switches
- Keyboard navigation (Ctrl+Tab)

**Breadcrumbs**: For deep navigation (e.g., snapshot browsing)

### 5.2 Data Loading & Feedback

**Loading States**:
- Skeleton screens for initial loads
- Progress bars for known operations
- Spinners for indeterminate operations

**Error Handling**:
- Toast notifications for temporary errors
- Inline validation for forms
- Recovery suggestions where possible

**Real-time Updates**:
- WebSocket primary connection for real-time stats
- 500ms polling fallback if WebSocket unavailable
- Graceful degradation with connection status indicator
- Automatic reconnection on connection loss

### 5.3 User Actions

**Confirmations**: For destructive actions (delete snapshot, format device)

**Batch Operations**: Multi-select with bulk actions

**Keyboard Shortcuts**:
- Ctrl+N: New snapshot
- Ctrl+R: Refresh
- F5: Refresh current view
- Ctrl+,: Settings (macOS: Cmd+,)

---

## 6. Platform-Specific Adaptations

### 6.1 macOS

**Native Elements**:
- Transparent title bar with traffic lights
- Native context menus
- Vibrancy effects for sidebar
- System notification integration

**Menu Bar**: 
- AegisFS menu in menu bar
- Standard macOS keyboard shortcuts

### 6.2 Windows

**Native Elements**:
- Windows 11 styled title bar
- Native notifications (Action Center)
- System tray integration
- Windows-style context menus

**Title Bar**: Custom controls with native Windows styling

### 6.3 Linux

**Native Elements**:
- GTK-style window decorations
- Desktop notification integration
- System theme compatibility
- Standard Linux keyboard shortcuts

---

## 7. Performance Considerations

### 7.1 Optimization Strategies

**Data Virtualization**: For large lists (snapshots, files)

**Lazy Loading**: Load tab content on demand

**Caching**: Cache API responses with appropriate TTL

**Debouncing**: For search and filter inputs

### 7.2 Resource Management

**Memory Usage**: Target <100MB RAM usage

**CPU Usage**: Background tasks don't block UI

**Network**: Efficient API calls with minimal payloads

---

## 8. Development Phases

### Phase 1: Foundation (Week 1-2)
- [ ] Tauri project setup with WebSocket plugin
- [ ] Basic window structure and 7-tab navigation
- [ ] Theme system implementation
- [ ] WebSocket connection with 500ms polling fallback
- [ ] Multi-instance dropdown in header

### Phase 2: Core Features (Week 3-4)
- [ ] Dashboard tab with enhanced cards (including instance overview)
- [ ] Snapshots tab with master-detail view
- [ ] Settings tab with performance monitoring slider
- [ ] Basic error handling and notifications
- [ ] Built-in terminal panel (toggleable)

### Phase 3: Data & Monitoring (Week 5-6)
- [ ] Logs tab with real-time streaming
- [ ] Enhanced performance monitoring with historical charts
- [ ] Real-time WebSocket event handling
- [ ] Multi-instance management features
- [ ] Tiering tab and rule management

### Phase 4: Backup & Advanced (Week 7-8)
- [ ] Backup tab with job scheduling
- [ ] Advanced settings categories
- [ ] Terminal command integration and auto-completion
- [ ] Log filtering and export functionality
- [ ] Snapshot browsing and rollback

### Phase 5: Polish & Platform (Week 9-10)
- [ ] Platform-specific adaptations (macOS/Windows/Linux)
- [ ] Performance optimization and caching
- [ ] Comprehensive error handling
- [ ] Help system with integrated documentation
- [ ] Cross-instance operations (batch snapshots, sync)

---

## 9. API Integration Points

### 9.1 Required Backend APIs
- `GET /api/status` - System status and health
- `GET /api/stats` - Real-time I/O statistics
- `GET /api/snapshots` - List snapshots
- `POST /api/snapshots` - Create snapshot
- `DELETE /api/snapshots/:id` - Delete snapshot
- `POST /api/snapshots/:id/rollback` - Rollback to snapshot
- `GET /api/settings` - Get configuration
- `PUT /api/settings` - Update configuration
- `GET /api/logs` - System logs
- `WebSocket /api/events` - Real-time events

### 9.2 Data Models

**Filesystem Status**:
```javascript
{
  device: "/dev/nvme0n1p6",
  mountPoint: "/mnt/aegisfs", 
  status: "online|offline|error",
  totalSize: 1000000000,
  usedSize: 750000000,
  freeSize: 250000000
}
```

**Snapshot**:
```javascript
{
  id: "snapshot_20241230_1530",
  name: "Daily backup",
  created: "2024-12-30T15:30:00Z",
  size: 2100000000,
  state: "ready|creating|error",
  filesChanged: 127
}
```

---

## 10. Accessibility & Usability

### 10.1 Accessibility Features
- **Keyboard Navigation**: Full keyboard accessibility
- **Screen Reader Support**: Proper ARIA labels and roles
- **High Contrast**: Support for high contrast themes
- **Focus Management**: Clear focus indicators

### 10.2 Internationalization
- **Text Externalization**: All UI text in language files
- **RTL Support**: Basic right-to-left language support
- **Number/Date Formatting**: Locale-appropriate formatting

---

## 11. WebSocket Implementation

### 11.1 Tauri WebSocket Integration

Using the [Tauri WebSocket plugin](https://v2.tauri.app/reference/javascript/websocket/) for real-time communication:

```javascript
import { WebSocket } from '@tauri-apps/plugin-websocket';

// Connect to AegisFS WebSocket server
const ws = await WebSocket.connect('ws://localhost:8080/api/events', {
  headers: { 'Authorization': 'Bearer token' },
  maxMessageSize: 1024 * 1024, // 1MB max message
  readBufferSize: 128 * 1024,   // 128KB buffer
});

// Listen for real-time events
ws.addListener((message) => {
  switch (message.type) {
    case 'Text':
      const data = JSON.parse(message.data);
      handleRealtimeUpdate(data);
      break;
  }
});
```

### 11.2 Real-time Event Types

**Filesystem Events**:
- `status_change`: Mount/unmount events
- `io_stats`: Real-time I/O metrics (every 500ms)
- `health_update`: Checksum verification results
- `disk_usage`: Storage utilization changes

**Snapshot Events**:
- `snapshot_created`: New snapshot completion
- `snapshot_deleted`: Snapshot removal
- `snapshot_progress`: Creation/deletion progress

**System Events**:
- `error_occurred`: System errors and warnings
- `scrub_progress`: Scrub operation updates
- `tier_migration`: File tier movement updates

### 11.3 Fallback Strategy

```javascript
class RealtimeConnection {
  constructor() {
    this.preferWebSocket = true;
    this.pollInterval = 500; // 500ms as specified
    this.reconnectAttempts = 0;
    this.maxReconnectAttempts = 5;
  }

  async connect() {
    try {
      if (this.preferWebSocket) {
        await this.connectWebSocket();
      } else {
        this.startPolling();
      }
    } catch (error) {
      console.warn('WebSocket failed, falling back to polling');
      this.startPolling();
    }
  }

  startPolling() {
    setInterval(() => {
      this.fetchUpdates();
    }, this.pollInterval);
  }
}
```

---

## 12. Multi-Instance Management

### 12.1 Instance Discovery & Selection

**Main Window Header Enhancement**:
```
┌─────────────────────────────────────────────────────┐
│ [≡] AegisFS Management [Instance: /dev/nvme0n1p6 ▼] │
├─────────────────────────────────────────────────────┤
│ Dashboard | Snapshots | Tiering | Settings | Help   │
```

**Instance Dropdown**:
- Auto-discover mounted AegisFS instances
- Manual instance addition via device path
- Color-coded status indicators per instance
- Quick switch between instances

### 12.2 Instance Management Features

**Instance Overview Card** (on Dashboard):
```
┌─────────────────────────────────────────────────────┐
│ Connected Instances                    [Add New]    │
│                                                     │
│ 🟢 /dev/nvme0n1p6  → /mnt/aegisfs    [Select]       │
│ 🟡 /dev/sdb1       → /mnt/backup     [Select]       │
│ 🔴 /dev/sdc1       → /mnt/archive    [Reconnect]    │
└─────────────────────────────────────────────────────┘
```

**Multi-Instance Operations**:
- Batch snapshot creation across instances
- Cross-instance backup synchronization
- Unified monitoring dashboard
- Instance-specific settings

---

## 13. Built-in Terminal

### 13.1 Terminal Integration

**Location**: Expandable panel at bottom of main window (similar to VS Code)

**Toggle Options**:
- Keyboard shortcut: `` Ctrl+` `` (backtick)
- Menu option: View → Terminal
- Settings toggle: "Show terminal panel"

### 13.2 Terminal Layout

```
├─────────────────────────────────────────────────────┤
│ Status: Connected to /dev/nvme0n1p6 │ [Terminal ▲]  │
├─────────────────────────────────────────────────────┤
│ $ aegisfs snapshot create "manual-backup"          │
│ Creating snapshot: manual-backup                   │
│ ✓ Snapshot created successfully: snap_1234567890   │
│ $ ls /mnt/aegisfs                                  │
│ file1.txt  file2.txt  .snapshots/                 │
│ $                                                   │ ← Active cursor
└─────────────────────────────────────────────────────┘
```

### 13.3 Terminal Features

**Command Integration**:
- Direct access to `aegisfs` CLI commands
- Auto-completion for AegisFS commands
- Command history with up/down arrows
- Real-time command output

**Settings Options**:
- Terminal font size and family
- Color scheme (light/dark/custom)
- Shell selection (bash, zsh, fish, etc.)
- Working directory behavior

---

## 14. Built-in Log Viewer

### 14.1 Log Viewer Tab Addition

**New Tab**: Add "Logs" tab to main navigation:
```
│ Dashboard | Snapshots | Tiering | Logs | Settings | Help │
```

### 14.2 Log Viewer Layout

```
┌─────────────┬─────────────────────────────────────┐
│ Log Sources │ Log Content                         │
│             │                                     │
│ ☑ System    │ [2024-12-30 15:30:25] INFO: Mount   │
│ ☑ I/O       │ operation completed successfully    │
│ ☑ Snapshots │ [2024-12-30 15:30:23] WARN: High   │
│ ☑ Errors    │ disk usage detected (89%)           │
│ ☐ Debug     │ [2024-12-30 15:30:20] INFO: Snapshot│
│             │ 'daily_backup' created              │
│ [Filter...] │                                     │
│ [Export]    │ ┌─────────────────────────────────┐ │
│ [Clear]     │ │ 🔍 Search logs...               │ │
│             │ └─────────────────────────────────┘ │
└─────────────┴─────────────────────────────────────┘
```

### 14.3 Log Features

**Real-time Updates**: Live log streaming via WebSocket
**Filtering**: By log level, source, time range, keywords
**Export**: Save filtered logs to file
**Search**: Full-text search with highlighting
**Auto-scroll**: Option to follow latest logs

---

## 15. Backup Integration

### 15.1 Backup Tab Addition

**Enhanced Navigation**:
```
│ Dashboard | Snapshots | Tiering | Backup | Logs | Settings | Help │
```

### 15.2 Backup Management Layout

```
┌─────────────────────────────────────────────────────┐
│ Backup Jobs                             [New Job]   │
│                                                     │
│ ┌─ Daily System Backup ──────────── [●] Active ──┐ │
│ │ Target: /mnt/backup-drive/daily                 │ │
│ │ Schedule: Every day at 2:00 AM                  │ │
│ │ Last run: 2024-12-30 02:00 (Success)           │ │
│ │ [Edit] [Run Now] [View Logs] [Delete]          │ │
│ └─────────────────────────────────────────────────┘ │
├─────────────────────────────────────────────────────┤
│ Backup Status                                       │
│                                                     │
│ ┌─ Current Operation ────────────────────────────┐  │
│ │ Backing up to /mnt/backup-drive/daily         │  │
│ │ Progress: ████████░░ 78% (1.2GB / 1.5GB)      │  │
│ │ Files: 1,247 / 1,590 processed                │  │
│ │ ETA: 3 minutes remaining                      │  │
│ └───────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────┘
```

### 15.3 Backup Features

**Backup Types**:
- Full filesystem backup
- Incremental backups
- Snapshot-based backups
- Selective directory backup

**Scheduling**:
- Cron-style scheduling
- Visual schedule editor
- Backup retention policies
- Compression options

**Targets**:
- Local drives and network shares
- Cloud storage integration (future)
- Multiple backup destinations
- Backup verification

---

## 16. Enhanced Performance Monitoring

### 16.1 Configurable Detail Levels

**Settings Slider** (in Advanced Settings):
```
Performance Monitoring Detail Level:
Basic ────●────────────── Advanced
      Low   Medium   High   Maximum

Current: Medium
- I/O operations: ✓ (1 second intervals)
- Cache statistics: ✓ (5 second intervals)  
- Detailed breakdowns: ✗
- Historical data: ✓ (24 hours)
- Per-file tracking: ✗
```

### 16.2 Historical Charts

**Dashboard Enhancement** - I/O Stats Card:
```
┌─────────────────────────────────────┐
│ I/O Statistics        [⚙] [📊] [⏱] │
│                                     │
│ Read:  125.3 MB/s    ████████░░     │
│ Write:  89.7 MB/s    ██████░░░░     │
│ IOPS:   2,847        ████████░░     │
│                                     │
│ ┌─ Last 24 Hours ─────────────────┐ │
│ │     ╭─╮                         │ │
│ │   ╭─╯ ╰╮     ╭─╮                │ │
│ │ ╭─╯    ╰─────╯ ╰╮               │ │
│ │╱                ╰─╮             │ │
│ └─────────────────────────────────┘ │
│ [1H] [6H] [24H] [7D] [30D]         │
└─────────────────────────────────────┘
```

### 16.3 Performance Detail Levels

**Basic Level**:
- Overall I/O rates (read/write MB/s)
- Storage usage percentages
- System health status

**Medium Level** (Default):
- IOPS and latency metrics
- Cache hit ratios
- 24-hour historical charts
- Error counts

**High Level**:
- Per-tier performance breakdown
- Detailed cache statistics
- Queue depth and utilization
- 7-day historical data

**Maximum Level**:
- Per-file operation tracking
- Kernel-level statistics
- Advanced profiling data
- 30-day historical retention
- Real-time trace logging

---

## 17. Updated API Integration

### 17.1 Additional Required APIs

**Multi-Instance Support**:
- `GET /api/instances` - List all AegisFS instances
- `POST /api/instances` - Add new instance
- `DELETE /api/instances/:id` - Remove instance tracking

**Terminal Integration**:
- `POST /api/terminal/execute` - Execute command
- `WebSocket /api/terminal/session` - Terminal session

**Enhanced Logging**:
- `GET /api/logs` - Get logs with filtering
- `WebSocket /api/logs/stream` - Real-time log stream

**Backup Management**:
- `GET /api/backup/jobs` - List backup jobs
- `POST /api/backup/jobs` - Create backup job
- `PUT /api/backup/jobs/:id` - Update backup job
- `POST /api/backup/jobs/:id/run` - Start backup
- `GET /api/backup/status` - Current backup status

**Enhanced Performance**:
- `GET /api/performance/history` - Historical performance data
- `PUT /api/performance/settings` - Update monitoring level

### 17.2 WebSocket Event Extensions

**Additional Event Types**:
```javascript
{
  type: "backup_progress",
  data: {
    jobId: "daily_backup",
    progress: 0.78,
    filesProcessed: 1247,
    totalFiles: 1590,
    bytesTransferred: 1200000000,
    eta: 180 // seconds
  }
}

{
  type: "terminal_output",
  data: {
    sessionId: "term_123",
    output: "✓ Snapshot created successfully\n",
    exitCode: null // null if command still running
  }
}

{
  type: "log_entry",
  data: {
    timestamp: "2024-12-30T15:30:25Z",
    level: "INFO",
    source: "snapshot",
    message: "Snapshot 'manual_backup' created successfully"
  }
}
```

---

## Requirements Finalized ✅

1. **Real-time Updates**: ✅ WebSocket connection with 500ms polling fallback
2. **Snapshot Browser**: ✅ Integrated in same window (master-detail view)
3. **Advanced Features**: ✅ Built-in terminal (toggleable in settings)
4. **Customization**: ✅ Minimal initially, expandable later
5. **Multi-device**: ✅ Support multiple AegisFS instances simultaneously
6. **Logging**: ✅ Built-in log viewer with real-time updates
7. **Backup Integration**: ✅ Full backup scheduling and management
8. **Performance Monitoring**: ✅ Configurable detail levels with historical charts

---

*This plan provides a comprehensive foundation for building a modern, native-feeling GUI for AegisFS. The design emphasizes usability, performance, and platform integration while maintaining the advanced functionality required by the filesystem's feature set.* 