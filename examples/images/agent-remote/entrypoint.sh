#!/bin/bash
set -e

mkdir -p /home/agent/.ssh
echo "${SSH_PUBLIC_KEY}" > /home/agent/.ssh/authorized_keys
chmod 700 /home/agent/.ssh
chmod 600 /home/agent/.ssh/authorized_keys
chown -R agent:agent /home/agent/.ssh

echo "export ANTHROPIC_API_KEY=${ANTHROPIC_API_KEY}" >> /home/agent/.profile
echo "export GITHUB_TOKEN=${GITHUB_TOKEN}" >> /home/agent/.profile
echo 'export PATH="$HOME/.claude/local/bin:$PATH"' >> /home/agent/.profile

# Configure git for the agent user
su - agent -c 'git config --global user.name "agent" && git config --global user.email "agent@oct.dev"'

exec /usr/sbin/sshd -D
