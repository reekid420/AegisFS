use aegisfs::AegisFS;
use fuser::MountOption;
use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::path::PathBuf;
use std::thread;
use tempfile::TempDir;

#[cfg(test)]
mod tests {
    use super::*;

// Helper function to create a test filesystem
fn setup_test_fs() -> io::Result<(TempDir, PathBuf)> {
    // Create a temporary directory for mounting
    let mount_dir = tempfile::tempdir()?;
    let mount_path = mount_dir.path().to_path_buf();
    
    // Create a new AegisFS instance
    let fs = AegisFS::new();
    
    // Mount the filesystem in a separate thread
    let mount_path_clone = mount_path.clone();
    let _mount_handle = thread::spawn(move || {
        let options = vec![
            MountOption::RW,
            MountOption::FSName("aegisfs".to_string()),
            MountOption::AutoUnmount,
            MountOption::AllowOther,
        ];
        
        if let Err(e) = fuser::mount2(fs, &mount_path_clone, &options) {
            eprintln!("Failed to mount filesystem: {}", e);
            std::process::exit(1);
        }
    });
    
    // Give the filesystem a moment to mount
    std::thread::sleep(std::time::Duration::from_secs(1));
    
    Ok((mount_dir, mount_path))
}

#[test]
fn test_write_operation() -> io::Result<()> {
    // Set up logging for the test
    env_logger::builder().is_test(true).try_init().ok();
    
    println!("Setting up test filesystem...");
    let (_mount_dir, mount_path) = setup_test_fs()?;
    
    // Test file creation and writing
    let file_path = mount_path.join("test.txt");
    let test_data = b"Hello, AegisFS!";
    
    println!("Creating test file...");
    // Create and write to file using standard filesystem API
    {
        println!("Creating file at: {:?}", file_path);
        let mut file = File::create(&file_path)?;
        file.write_all(test_data)?;
        file.sync_all()?;
    }
    
    // Verify file exists
    assert!(file_path.exists(), "File should exist after creation");
    println!("File created successfully");
    
    // Verify file content
    println!("Verifying file content...");
    {
        let mut file = File::open(&file_path)?;
        let mut content = Vec::new();
        file.read_to_end(&mut content)?;
        
        assert_eq!(
            content, test_data, 
            "File content should match written data"
        );
        
        // Verify file size
        let metadata = file.metadata()?;
        assert_eq!(
            metadata.len(), 
            test_data.len() as u64, 
            "File size should match written data length"
        );
    }
    
    println!("File content verified");
    
    // Test file deletion
    println!("Deleting test file...");
    fs::remove_file(&file_path)?;
    assert!(!file_path.exists(), "File should not exist after deletion");
    println!("File deleted successfully");
    
    // The mount_dir will be automatically cleaned up when it goes out of scope
    Ok(())
}
}
