#!/bin/bash
#
# Run Claude Code non-interactively on a remote agent-remote container.
#
# Sends a prompt over SSH, polls for streamed output, and prints assistant
# responses as they arrive. Exits cleanly when the result event is received.
#
# Usage:
#   ./scripts/remote-claude.sh <prompt> <host> [port]
#
# Example:
#   ./scripts/remote-claude.sh "fix the bug in auth.rs" 54.218.224.245
#   ./scripts/remote-claude.sh "add tests" 10.0.1.5 2222

if [ -z "$1" ] || [ -z "$2" ]; then
  echo "Usage: $0 <prompt> <host> [port]" >&2
  exit 1
fi

PROMPT="$1"
HOST="$2"
PORT="${3:-80}"

REMOTE="ssh -p $PORT agent@$HOST"
STREAM_FILE="/tmp/claude-stream.jsonl"
WORKDIR="~/cloudtool"
POLL_INTERVAL=1

# Clear previous output
$REMOTE "rm -f $STREAM_FILE"

# Start Claude in the background on the remote host
$REMOTE "bash -lc 'cd $WORKDIR && nohup claude -p \"$PROMPT\" --dangerously-skip-permissions --output-format stream-json --verbose > $STREAM_FILE 2>&1 &'"

# Poll the stream file and print assistant responses
OFFSET=0
while true; do
  CHUNK=$($REMOTE "tail -n +$((OFFSET + 1)) $STREAM_FILE 2>/dev/null") || true

  if [ -n "$CHUNK" ]; then
    NEW_LINES=$(echo "$CHUNK" | wc -l | tr -d ' ')
    OFFSET=$((OFFSET + NEW_LINES))

    DONE=false
    while IFS= read -r line; do
      echo "$line" | python3 -c "
import sys, json
try:
    obj = json.loads(sys.stdin.read())
    if obj.get('type') == 'assistant':
        for c in obj.get('message', {}).get('content', []):
            if c.get('type') == 'text':
                print(c['text'], end='', flush=True)
    elif obj.get('type') == 'result':
        sys.exit(42)
except SystemExit:
    raise
except Exception:
    pass
" 2>/dev/null
      if [ $? -eq 42 ]; then
        DONE=true
        break
      fi
    done <<< "$CHUNK"

    if $DONE; then
      echo
      break
    fi
  fi

  sleep "$POLL_INTERVAL"
done
