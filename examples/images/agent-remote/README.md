# agent-remote

Docker image for running Claude Code on a remote VM. Launch a VM, send your repo, run an agent prompt, and sync changes back.

## Prerequisites

- `oct` CLI built and in PATH (`cargo install --path crates/oct-cli`)
- SSH key pair at `~/.ssh/id_ed25519`
- `ANTHROPIC_API_KEY` set in environment
- Image pushed to a registry (e.g. `ghcr.io/opencloudtool/agent-remote:latest`)

## Build and Push

```bash
docker build --no-cache --platform linux/amd64 -t ghcr.io/opencloudtool/agent-remote:latest examples/images/agent-remote/
docker push ghcr.io/opencloudtool/agent-remote:latest
```

## Step-by-Step Usage

### 1. Launch VM

```bash
oct run \
  --image ghcr.io/opencloudtool/agent-remote:latest \
  --external-port 80 \
  --internal-port 22 \
  --cpus 4000 \
  --memory 8192 \
  -e "SSH_PUBLIC_KEY=$(cat ~/.ssh/id_ed25519.pub)" \
  -e "ANTHROPIC_API_KEY=$ANTHROPIC_API_KEY"
```

Note the IP from the output (e.g. `34.210.55.163`).

### 2. Sync Repo to VM

```bash
rsync -az \
  --exclude=.git --exclude=target --exclude=.venv \
  --exclude=node_modules --exclude=.env --exclude=__pycache__ \
  --exclude=.claude --exclude=coverage.html \
  -e "ssh -p 80 -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null" \
  ./ agent@<VM_IP>:/home/agent/cloudtool/
```

### 3. Run Agent (fire and forget)

Start Claude in the background — SSH returns immediately:

```bash
ssh -p 80 -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null \
  agent@<VM_IP> \
  "bash -lc 'cd ~/cloudtool && nohup claude -p \"your prompt here\" --dangerously-skip-permissions > /tmp/claude-output.txt 2>&1 &'"
```

### 4. Poll for Completion

Check if the agent is still running:

```bash
ssh -p 80 -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null \
  agent@<VM_IP> "pgrep -f 'claude.*-p' > /dev/null && echo RUNNING || echo DONE"
```

### 5. Read Agent Output

Once done, grab the result:

```bash
ssh -p 80 -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null \
  agent@<VM_IP> "cat /tmp/claude-output.txt"
```

### 6. Sync Changes Back

```bash
rsync -az \
  --exclude=.git --exclude=target --exclude=.venv \
  --exclude=node_modules --exclude=.env --exclude=__pycache__ \
  --exclude=.claude --exclude=coverage.html \
  -e "ssh -p 80 -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null" \
  agent@<VM_IP>:/home/agent/cloudtool/ ./
```

View what the agent changed:

```bash
git diff
```

### 7. Destroy VM

```bash
oct destroy --state-path ./oct-run-state.json
```

## How It Works

The agent runs **asynchronously** on the remote VM. There is no long-lived SSH connection.

1. **Send prompt** — SSH starts Claude in the background with `nohup` and exits immediately. The agent continues running on the VM independently.
2. **Poll for completion** — You periodically check if the `claude` process is still alive using `pgrep`. When the process exits, the agent is done. You are not searching output for a "done" message — you are checking if the **process is still running**.
3. **Read output** — Once `pgrep` reports the process is gone, read the output file to see what the agent did.
4. **Sync back** — Pull changed files with rsync and use `git diff` locally to review.

This avoids SSH timeout issues that occur with long-running prompts over a persistent connection.

## SSH Tips

- The VM exposes SSH on port **80** (external) mapped to port 22 (internal).
- Use `-o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null` to skip host key verification since VMs are ephemeral.
- For long-running prompts use the nohup approach (step 3) instead of a persistent SSH connection to avoid timeouts.
