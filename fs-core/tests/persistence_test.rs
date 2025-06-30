use std::fs::{File, OpenOptions};
use std::io::{Read, Write, Seek, SeekFrom};
use std::path::Path;
use std::process::Command;
use tempfile::TempDir;
use tokio::time::{sleep, Duration};

/// Test that verifies data is actually written to disk and not just stored in memory
#[tokio::test]
async fn test_data_persistence() {
    env_logger::builder().is_test(true).try_init().ok();
    
    println!("Testing filesystem persistence...");
    
    // Create a temporary directory for our test
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let device_path = temp_dir.path().join("test_device.img");
    let mount_path = temp_dir.path().join("mount");
    std::fs::create_dir_all(&mount_path).expect("Failed to create mount dir");
    
    // Step 1: Format the device
    println!("Formatting device...");
    let format_output = Command::new("cargo")
        .args(&["run", "--bin", "aegisfs-format", "--", 
               device_path.to_str().unwrap(), "--size", "1", "--force"])
        .output()
        .expect("Failed to run format command");
    
    if !format_output.status.success() {
        panic!("Format failed: {}", String::from_utf8_lossy(&format_output.stderr));
    }
    
    // Step 2: Verify the device was formatted (check magic number)
    println!("Verifying device format...");
    let mut device_file = File::open(&device_path).expect("Failed to open device");
    let mut magic = [0u8; 8];
    device_file.read_exact(&mut magic).expect("Failed to read magic");
    assert_eq!(&magic, b"AEGISFS\x00", "Device should be formatted with AegisFS");
    
    // Step 3: Mount the filesystem in background
    println!("Mounting filesystem...");
    let _mount_child = Command::new("cargo")
        .args(&["run", "--bin", "aegisfs-mount", "--", 
               device_path.to_str().unwrap(), mount_path.to_str().unwrap()])
        .spawn()
        .expect("Failed to start mount process");
    
    // Give the filesystem time to mount
    sleep(Duration::from_secs(2)).await;
    
    // Step 4: Write test data to the mounted filesystem
    println!("Writing test data...");
    let test_file = mount_path.join("test_persistence.txt");
    let test_data = b"This data should persist to disk!";
    
    {
        let mut file = File::create(&test_file).expect("Failed to create test file");
        file.write_all(test_data).expect("Failed to write test data");
        file.sync_all().expect("Failed to sync file");
    }
    
    // Step 5: Verify the file exists and has correct content
    println!("Verifying file in mounted filesystem...");
    {
        let mut file = File::open(&test_file).expect("Failed to open test file");
        let mut content = Vec::new();
        file.read_to_end(&mut content).expect("Failed to read file");
        assert_eq!(content, test_data, "File content should match");
    }
    
    // Step 6: Unmount the filesystem
    println!("Unmounting filesystem...");
    Command::new("fusermount")
        .args(&["-u", mount_path.to_str().unwrap()])
        .output()
        .expect("Failed to unmount");
    
    // Wait for unmount to complete
    sleep(Duration::from_secs(1)).await;
    
    // Step 7: THE CRITICAL TEST - Check the raw device for our data
    println!("Checking raw device for persisted data...");
    
    // Read the entire device to see if our data was written
    let mut device_file = File::open(&device_path).expect("Failed to open device for verification");
    let mut device_content = Vec::new();
    device_file.read_to_end(&mut device_content).expect("Failed to read device");
    
    // Search for our test data in the raw device
    let test_data_found = device_content.windows(test_data.len())
        .any(|window| window == test_data);
    
    if test_data_found {
        println!("✅ SUCCESS: Test data found in raw device - data is persistent!");
    } else {
        println!("❌ FAILURE: Test data NOT found in raw device - data is only in memory!");
        
        // Additional debugging: show what's actually in the device
        println!("Device content (first 1024 bytes):");
        let preview = &device_content[..std::cmp::min(1024, device_content.len())];
        for (i, chunk) in preview.chunks(16).enumerate() {
            print!("{:04x}: ", i * 16);
            for byte in chunk {
                print!("{:02x} ", byte);
            }
            println!();
        }
        
        panic!("Data persistence test failed - FUSE filesystem is not writing to disk!");
    }
    
    // Step 8: Try to remount and verify data is still there
    println!("Remounting to verify persistence...");
    let _mount_child2 = Command::new("cargo")
        .args(&["run", "--bin", "aegisfs-mount", "--", 
               device_path.to_str().unwrap(), mount_path.to_str().unwrap()])
        .spawn()
        .expect("Failed to start second mount process");
    
    sleep(Duration::from_secs(2)).await;
    
    // Try to read the file again
    if test_file.exists() {
        let mut file = File::open(&test_file).expect("Failed to open test file after remount");
        let mut content = Vec::new();
        file.read_to_end(&mut content).expect("Failed to read file after remount");
        
        if content == test_data {
            println!("✅ SUCCESS: Data persisted across remount!");
        } else {
            println!("❌ FAILURE: Data changed after remount");
        }
    } else {
        println!("❌ FAILURE: File does not exist after remount");
    }
    
    // Cleanup
    Command::new("fusermount")
        .args(&["-u", mount_path.to_str().unwrap()])
        .output()
        .ok();
} 