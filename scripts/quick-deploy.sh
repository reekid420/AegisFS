#!/bin/bash

./build-cross-platform.sh
echo "Build complete"
mkdir -p /tmp/aegisfs
cd ../fs-app/cli/target/release/

# Try to find the correct device by PARTUUID or fallback to device names
DEVICE=""

# Default PARTUUID
DEFAULT_PARTITION_UUID="2e9536a5-bcbd-4a60-a52d-47c5a6fb4b2c"
PARTITION_UUID=""

# Check if script is run with -i flag for image file
if [[ "$1" == "-i" ]]; then
    echo "Creating 3GB test image file in /tmp"
    TEST_IMG="/tmp/aegisfs_test.img"
    truncate -s 3G "$TEST_IMG"
    DEVICE="$TEST_IMG"
    echo "Using test image file: $DEVICE"
# Check if script is run with -p flag
elif [[ "$1" == "-p" ]]; then
    PARTITION_UUID="$DEFAULT_PARTITION_UUID"
    echo "Using default PARTUUID: $PARTITION_UUID"
    
    # Try to find device by PARTUUID
    if [ -e "/dev/disk/by-partuuid/$PARTITION_UUID" ]; then
        DEVICE="/dev/disk/by-partuuid/$PARTITION_UUID"
        echo "Found device by PARTUUID: $DEVICE -> $(readlink -f $DEVICE)"
    # Fallback to device name detection
    elif [ -e "/dev/nvme1n1p6" ]; then
        DEVICE="/dev/nvme1n1p6"  
        echo "Using fallback device: $DEVICE"
    elif [ -e "/dev/nvme0n1p6" ]; then
        DEVICE="/dev/nvme0n1p6"
        echo "Using fallback device: $DEVICE"
    else
        echo "ERROR: No suitable AegisFS partition found!"
        echo "Searched for:"
        echo "  - PARTUUID: $PARTITION_UUID" 
        echo "  - /dev/nvme1n1p6"
        echo "  - /dev/nvme0n1p6"
        exit 1
    fi
else
    # Interactive mode when not using -p flag
    
    # Try PARTUUID first if provided
    if [ -n "$PARTITION_UUID" ] && [ -e "/dev/disk/by-partuuid/$PARTITION_UUID" ]; then
        read -p "Found device by PARTUUID: $(readlink -f /dev/disk/by-partuuid/$PARTITION_UUID). Use this? (Y/n): " USE_PARTUUID
        if [[ "$USE_PARTUUID" == "" || "$USE_PARTUUID" == "Y" || "$USE_PARTUUID" == "y" ]]; then
            DEVICE="/dev/disk/by-partuuid/$PARTITION_UUID"
            echo "Using device: $DEVICE -> $(readlink -f $DEVICE)"
        fi
    fi
    
    
    # If still no device, ask for custom path
    if [ -z "$DEVICE" ]; then
        read -p "Enter custom device path: " CUSTOM_DEVICE
        if [ -e "$CUSTOM_DEVICE" ]; then
            DEVICE="$CUSTOM_DEVICE"
            echo "Using custom device: $DEVICE"
        else
            echo "ERROR: Device $CUSTOM_DEVICE does not exist!"
            exit 1
        fi
    fi
fi

# Format the detected device
echo "Formatting device: $DEVICE"
if ! sudo ./aegisfs format "$DEVICE" --debug --force; then
    echo "ERROR: Failed to format $DEVICE"
    exit 1
fi

echo "Format complete for $DEVICE"

# Clean up old logs
rm -f ../../../../mount-log.log.old
if [ -f ../../../../mount-log.log ]; then
    mv ../../../../mount-log.log ../../../../mount-log.log.old
fi

# Mount the same device that was formatted
echo "Mounting device: $DEVICE"
sudo ./aegisfs mount "$DEVICE" /tmp/aegisfs --debug > ../../../../mount-log.log 2>&1 &
echo "Mount complete - logs redirected to ../../../../mount-log.log"
echo "Device used: $DEVICE"