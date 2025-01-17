#!/usr/bin/env bash

script_dirname="$( cd "$( dirname "${BASH_SOURCE[0]}" )" &> /dev/null && pwd )"
outdir="${script_dirname}/_output"

sudo qemu-system-x86_64 -kernel "${outdir}/vmlinux" \
    -boot c -m 2049M \
    -drive file="${outdir}/server-cloudimg-amd64.img",format=qcow2 \
    -cdrom "${outdir}/user-data.img" \
    -net user \
    -smp 6 \
    -append "root=/dev/sda1 rw console=ttyS0,115200 acpi=off nokaslr" \
    -nic user,hostfwd=tcp::2222-:22 \
    -virtfs local,path=..,mount_tag=host0,security_model=mapped,id=host0 \
    -serial mon:stdio -display none
