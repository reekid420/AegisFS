#!/bin/bash

# Test 1: Create two files and check if they are consistent
    cd /tmp/aegisfs
    touch /tmp/aegisfs/test.txt
    echo "test" > /tmp/aegisfs/test.txt
    cat /tmp/aegisfs/test.txt

    # Test 1: Check if test.txt contains 'test'
    if ! cat /tmp/aegisfs/test.txt | grep -q "test"; then
        echo "test.txt does not contain 'test'"
        exit 1
    fi
    echo "test.txt contains 'test'"

    # Test 2: Create second file and check consistency
    touch /tmp/aegisfs/test2.txt
    echo "test2" > /tmp/aegisfs/test2.txt
    if ! (cat /tmp/aegisfs/test2.txt | grep -q "test2" && cat /tmp/aegisfs/test.txt | grep -q "test"); then
        echo "files are inconsistent before large file"
        exit 1
    fi
    echo "files are consistent before large file"

    # Test 3: Create a 2MB test file, calculate MD5 and test consistency
    TEST_FILE="/tmp/test.bin"
    sudo dd if=/dev/urandom of="${TEST_FILE}" bs=1M count=2 status=none
    TEST_FILE_MD5=$(md5sum "${TEST_FILE}" | cut -d' ' -f1)
    export TEST_FILE_PATH="${TEST_FILE}"
    export TEST_FILE_MD5_HASH="${TEST_FILE_MD5}"

    # Copy the test file to the mount point
    cp "${TEST_FILE_PATH}" /tmp/aegisfs/
    cd /tmp/aegisfs
    if ! (cat /tmp/aegisfs/test.txt | grep -q "test" && cat /tmp/aegisfs/test2.txt | grep -q "test2" && echo "${TEST_FILE_MD5_HASH} /tmp/aegisfs/test.bin" | md5sum -c); then
        echo "files are inconsistent after large file"
        echo "Unmounting device"
        sudo fusermount -u /tmp/aegisfs
        echo "Device unmounted"
        echo "Test failed"
        exit 1
    fi
    echo "files are consistent after large file"
    echo "Unmounting device"
    sudo fusermount -u /tmp/aegisfs
    echo "Device unmounted"
    echo "Test complete"