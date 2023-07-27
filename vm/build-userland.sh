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

qemu-img create -b server-cloudimg-amd64.img -F qcow2 -f qcow2 server-cloudimg-amd64.qcow2 100G

ssh-keygen -b 2048 -t rsa -f sshkey -q -N ""

cat >user-data <<EOF
#cloud-config
password: foobar
chpasswd: { expire: False }
hostname: ubuntu
ssh_pwauth: True
ssh_authorized_keys:
  - $(cat sshkey.pub)
runcmd:
  - echo foobar | passwd
EOF

cloud-localds user-data.img user-data
