#!/bin/sh

set -e

echo "Installing oct-ctl to $HOME/.local/bin"

curl \
    --output $HOME/.local/bin/oct-ctl \
    -fsSL \
    https://github.com/opencloudtool/opencloudtool/releases/download/tip/oct-ctl \
    && chmod +x $HOME/.local/bin/oct-ctl

echo "oct-ctl is available at $HOME/.local/bin/oct-ctl"
