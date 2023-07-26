#!/bin/bash

set -eoux pipefail

ORGDIR=$PWD
tmpdir=$(mktemp -d)
pushd $tmpdir

sudo apt-get install -yq bc flex bison yacc libelf-dev
curl -OL https://cdn.kernel.org/pub/linux/kernel/v6.x/linux-6.1.41.tar.xz
tar xvf linux-6.1.41.tar.xz
cd linux-6.1.41
cp "$ORGDIR/linux-config" .config
make -j 4