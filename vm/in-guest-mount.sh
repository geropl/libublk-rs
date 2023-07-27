#!/bin/bash

mkdir /mnt/host_files
mount -t 9p -o trans=virtio host0 /mnt/host_files
