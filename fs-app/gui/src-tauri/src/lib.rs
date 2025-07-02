// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
use serde::{Deserialize, Serialize};
use std::sync::Mutex;
use tauri::{State, Manager};

// State management for filesystem instances
struct AppState {
    instances: Mutex<Vec<FilesystemInstance>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct FilesystemInstance {
    id: String,
    device: String,
    mount_point: String,
    status: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct ApiResponse<T> {
    success: bool,
    data: Option<T>,
    error: Option<String>,
}

// Command to get filesystem status
#[tauri::command]
async fn get_status(state: State<'_, AppState>) -> Result<ApiResponse<Vec<FilesystemInstance>>, String> {
    let instances = state.instances.lock().unwrap();
    Ok(ApiResponse {
        success: true,
        data: Some(instances.clone()),
        error: None,
    })
}

// Command to get performance statistics
#[tauri::command]
async fn get_stats() -> Result<ApiResponse<serde_json::Value>, String> {
    // Mock data for now - will be replaced with actual AegisFS stats
    let stats = serde_json::json!({
        "read_rate": 125.3,
        "write_rate": 89.7,
        "iops": 2847,
        "cache_hit_ratio": 0.85,
        "total_size": 1000000000,
        "used_size": 750000000,
        "free_size": 250000000
    });
    
    Ok(ApiResponse {
        success: true,
        data: Some(stats),
        error: None,
    })
}

// Command to list snapshots
#[tauri::command]
async fn list_snapshots() -> Result<ApiResponse<Vec<serde_json::Value>>, String> {
    // Mock data for now
    let snapshots = vec![
        serde_json::json!({
            "id": "snapshot_20241230_1530",
            "name": "Daily backup",
            "created": "2024-12-30T15:30:00Z",
            "size": 2100000000,
            "state": "ready",
            "filesChanged": 127
        })
    ];
    
    Ok(ApiResponse {
        success: true,
        data: Some(snapshots),
        error: None,
    })
}

// Command to create a snapshot
#[tauri::command]
async fn create_snapshot(_name: String, _description: Option<String>) -> Result<ApiResponse<String>, String> {
    // Mock implementation
    Ok(ApiResponse {
        success: true,
        data: Some(format!("snapshot_{}", chrono::Utc::now().timestamp())),
        error: None,
    })
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    env_logger::init();
    
    tauri::Builder::default()
        .setup(|app| {
            // Initialize app state
            app.manage(AppState {
                instances: Mutex::new(vec![
                    // Mock instance for development
                    FilesystemInstance {
                        id: "1".to_string(),
                        device: "/dev/nvme0n1p6".to_string(),
                        mount_point: "/mnt/aegisfs".to_string(),
                        status: "online".to_string(),
                    }
                ]),
            });
            Ok(())
        })
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_websocket::init())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_notification::init())
        .invoke_handler(tauri::generate_handler![
            get_status,
            get_stats,
            list_snapshots,
            create_snapshot
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
