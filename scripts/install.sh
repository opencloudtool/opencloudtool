#!/bin/sh

set -e

echo "Creating $HOME/.local/bin if it doesn't exist"

mkdir -p $HOME/.local/bin

echo "Installing oct-cli to $HOME/.local/bin"

curl \
    --output $HOME/.local/bin/oct-cli \
    -fsSL \
    https://github.com/opencloudtool/opencloudtool/releases/download/tip/oct-cli

chmod +x $HOME/.local/bin/oct-cli

echo "oct-cli is available at $HOME/.local/bin/oct-cli"

cat <<EOF

For Linux users, to add ~/.local/bin to your PATH permanently:

1. For bash users, run:
   echo 'export PATH="\$HOME/.local/bin:\$PATH"' >> ~/.bashrc && source ~/.bashrc

2. For zsh users, run:
   echo 'export PATH="\$HOME/.local/bin:\$PATH"' >> ~/.zshrc && source ~/.zshrc

EOF
