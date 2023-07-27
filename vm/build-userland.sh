#!/usr/bin/env bash

set -euo pipefail

img_url="https://cloud-images.ubuntu.com/releases/22.04/release/ubuntu-22.04-server-cloudimg-amd64.img"

script_dirname="$( cd "$( dirname "${BASH_SOURCE[0]}" )" &> /dev/null && pwd )"
outdir="${script_dirname}/_output"

sudo apt-get install -yq qemu-utils libguestfs-tools cloud-image-utils

rm -Rf $outdir
mkdir -p $outdir

curl -L -o "${outdir}/server-cloudimg-amd64.img" $img_url

cd $outdir

qemu-img resize server-cloudimg-amd64.img +20G

cat >user-data <<EOF
#cloud-config
password: asdfqwer
chpasswd: { expire: False }
ssh_pwauth: True
EOF

cloud-localds user-data.img user-data
