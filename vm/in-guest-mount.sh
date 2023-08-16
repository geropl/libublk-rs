#!/bin/bash

mkdir -p /mnt/host_files
mount -t 9p -o trans=virtio host0 /mnt/host_files

# Copy bin over
#cp /mnt/host_files/libublk-rs/ublks3-rs/target/debug/ublks3-rs .
cp /mnt/host_files/libublk-rs/ublks3-rs/target/release/ublks3-rs .