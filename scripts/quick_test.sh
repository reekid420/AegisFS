#!/bin/bash

echo "=== AegisFS Comprehensive Test Suite ==="
echo "This script combines basic operations, large file integrity, and persistence tests."
echo

# Exit on first error
set -e

# --- Pre-flight check and setup ---
PROJECT_ROOT=$(pwd)

# --- Configuration ---
MOUNT_POINT="/tmp/aegisfs"
declare -A MD5_HASHES

# --- Helper Functions ---
function fail {
    echo "❌ Test Failed: $1"
    cleanup
    exit 1
}

function cleanup {
    echo "--- Cleaning up ---"
    # Check if mount point exists and is mounted before trying to unmount
    if mountpoint -q "$MOUNT_POINT"; then
        echo "Unmounting $MOUNT_POINT..."
        sudo fusermount -u "$MOUNT_POINT" || sudo umount -f "$MOUNT_POINT"
    fi
    echo "Cleanup complete."
}

# --- Phase 1: Build Project ---
echo "▶️ Phase 1: Building project..."
if ! ./scripts/build-cross-platform.sh; then
    fail "Project build failed."
fi
echo "✅ Project built successfully."
echo

# --- Phase 2: Setup ---
echo "▶️ Phase 2: Setting up fresh filesystem..."

# Set default flag and handle user input. This test should not be interactive.
DEPLOY_FLAG="-p" # Default to partition
if [[ "$1" == "-i" ]]; then
    DEPLOY_FLAG="-i"
elif [[ -n "$1" && "$1" != "-p" ]]; then
    fail "Invalid argument for test: '$1'. Use '-i' for an image file or '-p' (default) for a partition."
fi

echo "Deploying with flag: $DEPLOY_FLAG"
if ! ./scripts/quick-deploy.sh "$DEPLOY_FLAG"; then
    fail "Filesystem deployment failed with flag $DEPLOY_FLAG."
fi
sleep 2
cd "$MOUNT_POINT"
echo "✅ Filesystem mounted at $MOUNT_POINT"
echo

# --- Phase 3: Basic File Operations ---
echo "▶️ Phase 3: Testing basic file operations..."
echo "hello" > test_basic.txt
if ! grep -q "hello" test_basic.txt; then
    fail "Basic write/read failed."
fi
echo "✅ Basic write/read successful."

echo "more data" > test_another.txt
if ! (grep -q "hello" test_basic.txt && grep -q "more data" test_another.txt); then
    fail "File consistency check failed."
fi
echo "✅ File consistency check successful."
echo

# --- Phase 4: Progressive Large File Test ---
echo "▶️ Phase 4: Progressive large file creation and integrity check..."
TEST_SIZES_MB=(10 50 100 512) # Reduced 1GB to 512MB for speed, can be changed to 1024

for SIZE in "${TEST_SIZES_MB[@]}"; do
    FILENAME="test_${SIZE}mb.bin"
    echo "Creating ${SIZE}MB file: $FILENAME..."
    dd if=/dev/urandom of="$FILENAME" bs=1M count="$SIZE" status=none
    
    FILE_SIZE=$(stat -c%s "$FILENAME")
    EXPECTED_SIZE=$((SIZE * 1024 * 1024))
    
    if [ "$FILE_SIZE" -ne "$EXPECTED_SIZE" ]; then
        fail "File size for $FILENAME is incorrect. Got $FILE_SIZE, expected $EXPECTED_SIZE."
    fi
    echo "✅ $FILENAME size is correct."

    echo "Calculating MD5 for $FILENAME..."
    MD5_HASHES["$FILENAME"]=$(md5sum "$FILENAME" | cut -d' ' -f1)
    echo "   MD5: ${MD5_HASHES[$FILENAME]}"
done
echo

# --- Phase 5: Persistence Test (Remount) ---
echo "▶️ Phase 5: Testing persistence after remount..."
echo "Syncing filesystem..."
sync
sleep 1

echo "Unmounting filesystem..."
cd "$PROJECT_ROOT" # Return to project root to ensure correct paths for subsequent scripts
sudo fusermount -u "$MOUNT_POINT" || sudo umount -f "$MOUNT_POINT"
sleep 2

echo "Remounting filesystem..."
if ! ./scripts/quick-deploy.sh -m; then # Mount only
    fail "Filesystem remount failed."
fi
sleep 2
cd "$MOUNT_POINT"
echo "✅ Filesystem remounted."
echo

# --- Phase 6: Verification after Remount ---
echo "▶️ Phase 6: Verifying all files and integrity after remount..."

# Verify basic files
if ! (grep -q "hello" test_basic.txt && grep -q "more data" test_another.txt); then
    fail "Basic files are inconsistent after remount."
fi
echo "✅ Basic files verified."

# Verify large files
for FILENAME in "${!MD5_HASHES[@]}"; do
    echo "Verifying $FILENAME..."
    if [ ! -f "$FILENAME" ]; then
        fail "File $FILENAME not found after remount."
    fi
    
    # Verify size
    SIZE_MB_STR=${FILENAME//[^0-9]/}
    EXPECTED_SIZE=$((SIZE_MB_STR * 1024 * 1024))
    ACTUAL_SIZE=$(stat -c%s "$FILENAME")
    if [ "$ACTUAL_SIZE" -ne "$EXPECTED_SIZE" ]; then
        fail "File size for $FILENAME is incorrect after remount. Got $ACTUAL_SIZE, expected $EXPECTED_SIZE."
    fi
    echo "✅ $FILENAME size is correct after remount."

    # Verify integrity
    echo "Calculating new MD5 for $FILENAME..."
    NEW_MD5=$(md5sum "$FILENAME" | cut -d' ' -f1)
    if [ "${MD5_HASHES[$FILENAME]}" != "$NEW_MD5" ]; then
        fail "MD5 mismatch for $FILENAME after remount. Expected ${MD5_HASHES[$FILENAME]}, got $NEW_MD5."
    fi
    echo "✅ $FILENAME integrity verified (MD5: $NEW_MD5)."
done
echo

# --- Final Cleanup ---
cd - > /dev/null
cleanup

echo "🎉🎉🎉 ALL TESTS PASSED! 🎉🎉🎉"
echo
exit 0