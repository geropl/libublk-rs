#!/bin/bash

set -eoux pipefail

script_dirname="$( cd "$( dirname "${BASH_SOURCE[0]}" )" &> /dev/null && pwd )"
tmpdir=$(mktemp -d)
pushd $tmpdir

sudo apt-get install -yq bc flex bison yacc libelf-dev
curl -OL https://cdn.kernel.org/pub/linux/kernel/v6.x/linux-6.1.41.tar.xz
tar xvf linux-6.1.41.tar.xz
cd linux-6.1.41
make defconfig
make kvm_guest.config
sed -i 's/# CONFIG_BLK_DEV_UBLK is not set/CONFIG_BLK_DEV_UBLK=y/g' .config
sed -i 's/# CONFIG_PVH is not set/CONFIG_PVH=y/g' .config
make -j 4

mkdir -p $script_dirname/_output
./scripts/extract-vmlinux arch/x86/boot/bzImage > $script_dirname/_output/vmlinux
