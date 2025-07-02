# AegisFS Management GUI

A modern, cross-platform desktop application for managing AegisFS filesystems built with Tauri.

## Features

- **Real-time Monitoring**: Live I/O statistics, performance metrics, and health monitoring
- **Snapshot Management**: Create, view, and rollback filesystem snapshots
- **Storage Tiering**: Configure and monitor storage tier policies
- **Backup Integration**: Schedule and manage filesystem backups
- **System Logs**: Real-time log viewer with filtering capabilities
- **WebSocket Support**: Real-time updates with automatic polling fallback
- **Cross-Platform**: Works on Linux, Windows, and macOS

## Prerequisites

- Node.js 16+ and npm
- Rust 1.70+
- Platform-specific development tools:
  - **Linux**: `pkg-config`, `libwebkit2gtk-4.1-dev`, `libgtk-3-dev`
  - **Windows**: Windows Build Tools
  - **macOS**: Xcode Command Line Tools

## Installation

1. Install dependencies:
```bash
npm install
```

2. Build the application:
```bash
npm run tauri build
```

The built application will be in `src-tauri/target/release/bundle/`.

## Development

To run in development mode with hot-reload:

```bash
npm run tauri dev
```

## Architecture

### Frontend (TypeScript + Vite)
- **Framework**: Vanilla TypeScript with modern web APIs
- **Styling**: SCSS with CSS variables for theming
- **Charts**: Chart.js for real-time performance graphs
- **State Management**: Simple global state with TypeScript

### Backend (Rust + Tauri)
- **Framework**: Tauri v2 with plugin system
- **Plugins**: WebSocket, File System, Dialog, Notification
- **API**: Command-based IPC with type-safe bindings

### Communication
- **Primary**: WebSocket connection for real-time updates
- **Fallback**: 500ms polling when WebSocket unavailable
- **Commands**: Tauri IPC for user-initiated actions

## Usage

### Dashboard Tab
The main overview showing:
- Filesystem status and mount information
- Real-time I/O statistics with graphs
- Health status and last scrub information
- Recent activity log
- Storage usage visualization
- Quick action buttons

### Snapshots Tab
Manage filesystem snapshots:
- Create new snapshots with custom names
- View snapshot timeline grouped by date
- Browse snapshot contents
- Rollback to previous snapshots
- Delete old snapshots

### Tiering Tab
Configure storage tiering:
- View tier utilization
- Create and manage tiering rules
- Monitor migration progress
- Cost analysis per tier

### Backup Tab
Backup management:
- Create backup jobs
- Schedule automatic backups
- Monitor backup progress
- View backup history

### Logs Tab
System log viewer:
- Real-time log streaming
- Filter by log level
- Search functionality
- Export logs

### Settings Tab
Configuration options:
- Mount settings
- Auto-snapshot schedules
- Performance tuning
- Theme preferences

### Help Tab
Documentation and support:
- Getting started guide
- Feature documentation
- Troubleshooting tips
- About information

## Configuration

The GUI stores its configuration in:
- **Linux**: `~/.config/aegisfs-gui/`
- **Windows**: `%APPDATA%\aegisfs-gui\`
- **macOS**: `~/Library/Application Support/aegisfs-gui/`

## Troubleshooting

### WebSocket Connection Issues
If the WebSocket connection fails, the app automatically falls back to polling. Check:
1. AegisFS daemon is running
2. WebSocket server is listening on port 8080
3. No firewall blocking the connection

### Performance Issues
1. Reduce monitoring detail level in Settings
2. Disable real-time chart updates
3. Clear old snapshots and logs

### Build Issues
1. Ensure all prerequisites are installed
2. Clear node_modules and reinstall: `rm -rf node_modules && npm install`
3. Clear Rust build cache: `cargo clean`

## Contributing

See the main AegisFS repository for contribution guidelines.

## License

This project is dual-licensed under MIT OR Apache-2.0.
