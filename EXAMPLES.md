# Usage Examples

This document provides detailed examples of how to use zellij-notify in various scenarios.

## Table of Contents
- [Basic Usage](#basic-usage)
- [Shell Integration](#shell-integration)
- [CI/CD Integration](#cicd-integration)
- [IDE/Editor Hooks](#ideeditor-hooks)
- [Advanced Patterns](#advanced-patterns)

## Basic Usage

### Append Status Emoji to Current Tab

**Direct commands** - use these! For background commands, always pass `pane_id` to ensure the emoji appears on the correct tab:

```bash
# Direct command with pane_id (works for background commands!)
zellij pipe -n "notify" -a "pane_id=$ZELLIJ_PANE_ID" ""              # Default ✅
zellij pipe -n "notify" -a "pane_id=$ZELLIJ_PANE_ID" "stop"          # Use preset ✅
zellij pipe -n "notify" -a "pane_id=$ZELLIJ_PANE_ID" "notification"  # Use preset ⚡
zellij pipe -n "notify" -a "pane_id=$ZELLIJ_PANE_ID" "subagent-stop" # Use preset 🔴

# Background command (will work even if you switch tabs!)
sleep 10 && zellij pipe -n "notify" -a "pane_id=$ZELLIJ_PANE_ID" "stop"

# Without pane_id (unreliable for background commands)
zellij pipe -n "notify" "stop"
```

### What Happens

1. The emoji is appended to the source tab name (the tab where the command was run)
2. When you switch to another tab, that tab's trailing emoji (if any) is automatically cleaned
3. The emoji persists on unfocused tabs until you return to them
4. **For background commands**: The plugin uses pane-to-tab mapping to identify the correct tab

**Example:**
```bash
# Tab name: "cargo"
zellij pipe -n "notify" -a "pane_id=$ZELLIJ_PANE_ID" "stop"
# Tab name: "cargo ✅"

# Switch to another tab
# The "cargo ✅" tab keeps its emoji until you focus it again

# Background command example
sleep 5 && zellij pipe -n "notify" -a "pane_id=$ZELLIJ_PANE_ID" "stop"  # Works!
```

## Shell Integration

### Direct Command (Recommended)

**No setup needed!** Just use the direct command:

```bash
# Always pass pane_id for reliable background command support
zellij pipe -n "notify" -a "pane_id=$ZELLIJ_PANE_ID" "stop"

# Background commands work perfectly
sleep 10 && zellij pipe -n "notify" -a "pane_id=$ZELLIJ_PANE_ID" "stop"
```

### Optional: Shell Helper Function

If you prefer shorter commands, add a helper function:

**Bash/Zsh** (`~/.bashrc` or `~/.zshrc`):
```bash
zellij-notify() {
    zellij pipe -n "notify" -a "pane_id=$ZELLIJ_PANE_ID" "${1:-}"
}
```

**Fish** (`~/.config/fish/config.fish`):
```fish
function zellij-notify
    zellij pipe -n "notify" -a "pane_id=$ZELLIJ_PANE_ID" $argv[1]
end
```

Then use: `zellij-notify "stop"` instead of the full command.

### Track Command Success/Failure

Add to your `~/.bashrc`, `~/.zshrc`, or `~/.config/fish/config.fish`:

**Bash/Zsh:**
```bash
# Notify when long-running commands complete (works even if you switch tabs!)
notify_done() {
    "$@"
    local status=$?
    if [ $status -eq 0 ]; then
        zellij-notify "stop"
    else
        zellij-notify "subagent-stop"
    fi
    return $status
}

# Usage
notify_done cargo build --release
notify_done npm install
notify_done python train_model.py
```

**Fish:**
```fish
function notify_done
    $argv
    set status $status
    if test $status -eq 0
        zellij-notify "stop"
    else
        zellij-notify "subagent-stop"
    end
    return $status
end

# Usage
notify_done cargo build --release
```

### Auto-notify for Long Commands

Automatically add status emojis to commands that take >5 seconds:

**Bash/Zsh:**
```bash
preexec() {
    timer=${timer:-$SECONDS}
}

precmd() {
    if [ -n "$timer" ]; then
        elapsed=$((SECONDS - timer))
        if [ $elapsed -gt 5 ]; then
            if [ $? -eq 0 ]; then
                zellij-notify "stop"
            else
                zellij-notify "subagent-stop"
            fi
        fi
        unset timer
    fi
}
```

## CI/CD Integration

### GitHub Actions

```yaml
name: Build and Test

on: [push, pull_request]

jobs:
  build:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3

      - name: Set up build notification
        run: |
          if command -v zellij &> /dev/null; then
            zellij pipe -n "notify" -a "pane_id=$ZELLIJ_PANE_ID" "notification"
          fi

      - name: Build
        run: cargo build --release

      - name: Test
        run: cargo test

      - name: Update status
        if: always()
        run: |
          if command -v zellij &> /dev/null; then
            if [ $? -eq 0 ]; then
              zellij pipe -n "notify" -a "pane_id=$ZELLIJ_PANE_ID" "stop"
            else
              zellij pipe -n "notify" -a "pane_id=$ZELLIJ_PANE_ID" "subagent-stop"
            fi
          fi
```

### Local CI Scripts

Make sure to source the `zellij-notify` function first (from your `.bashrc` or directly in the script):

```bash
#!/bin/bash
# ci-build.sh

set -e

# Source the helper function
zellij-notify() {
    zellij pipe -n "notify" -a "pane_id=$ZELLIJ_PANE_ID" "${1:-}"
}

# Notify start
zellij-notify "notification"

# Run build steps
echo "Running tests..."
cargo test

echo "Building release..."
cargo build --release

echo "Running clippy..."
cargo clippy -- -D warnings

# Success!
zellij-notify "stop"
```

Usage:
```bash
./ci-build.sh || zellij-notify "subagent-stop"
```

## IDE/Editor Hooks

### Claude Code

Add to `~/.claude/settings.json`:

```json
{
  "hooks": {
    "Notification": [{
      "matcher": "",
      "hooks": [{
        "type": "command",
        "command": "zellij pipe -n \"notify\" \"notification\""
      }]
    }],
    "PostToolUse": [{
      "matcher": "",
      "hooks": [{
        "type": "command",
        "command": "zellij pipe -n \"notify\" \"posttooluse\""
      }]
    }],
    "Stop": [{
      "matcher": "",
      "hooks": [{
        "type": "command",
        "command": "zellij pipe -n \"notify\" \"stop\""
      }]
    }],
    "SubagentStop": [{
      "matcher": "",
      "hooks": [{
        "type": "command",
        "command": "zellij pipe -n \"notify\" \"subagent-stop\""
      }]
    }]
  }
}
```

This shows:
- ⚡ when Claude is working
- ✅ when Claude completes a task
- 🔴 when a sub-agent stops

### VS Code Tasks

Add to `.vscode/tasks.json`:

```json
{
  "version": "2.0.0",
  "tasks": [
    {
      "label": "build with notification",
      "type": "shell",
      "command": "cargo build && zellij pipe -n 'notify' 'stop' || zellij pipe -n 'notify' 'subagent-stop'",
      "group": {
        "kind": "build",
        "isDefault": true
      }
    }
  ]
}
```

### Neovim/Vim

Add to your `init.vim` or `init.lua`:

**Lua:**
```lua
-- Notify on successful save and format
vim.api.nvim_create_autocmd("BufWritePost", {
  pattern = "*",
  callback = function()
    local pane_id = vim.env.ZELLIJ_PANE_ID
    if pane_id then
      vim.fn.system(string.format('zellij pipe -n "notify" -a "pane_id=%s" "stop"', pane_id))
    end
  end,
})
```

**VimScript:**
```vim
" Notify on successful save
autocmd BufWritePost * silent execute '!zellij pipe -n "notify" -a "pane_id=' . $ZELLIJ_PANE_ID . '" "stop"'
```

## Advanced Patterns

### Multi-Step Build Pipeline

```bash
#!/bin/bash
# build-pipeline.sh

set -e

# Source the helper function
zellij-notify() {
    zellij pipe -n "notify" -a "pane_id=$ZELLIJ_PANE_ID" "${1:-}"
}

steps=(
    "cargo fmt -- --check"
    "cargo clippy -- -D warnings"
    "cargo test"
    "cargo build --release"
)

total=${#steps[@]}
current=0

for step in "${steps[@]}"; do
    current=$((current + 1))
    echo "[$current/$total] Running: $step"

    # Update progress indicator
    zellij-notify "notification"

    if eval "$step"; then
        echo "✓ Step $current passed"
    else
        echo "✗ Step $current failed"
        zellij-notify "subagent-stop"
        exit 1
    fi
done

# All steps passed!
zellij-notify "stop"
echo "🎉 Pipeline complete!"
```

### Watch Mode with Notifications

```bash
#!/bin/bash
# watch-and-notify.sh

# Watch source files and run tests on change
cargo watch -x test -s 'zellij pipe -n "notify" "stop"' -s 'zellij pipe -n "notify" "subagent-stop"'
```

### Custom Presets for Different Projects

**Zellij config for a web project:**
```kdl
load_plugins {
    "file:~/.config/zellij/plugins/zellij-notify.wasm" {
        presets r#"{
            "build": {"emoji": "🔨"},
            "test": {"emoji": "🧪"},
            "deploy": {"emoji": "🚀"},
            "error": {"emoji": "💥"}
        }"#
    }
}
```

**Usage:**
```bash
# Building
zellij pipe -n "notify" "build"

# Running tests
zellij pipe -n "notify" "test"

# Deploying
zellij pipe -n "notify" "deploy"
```

### Integration with tmux/screen Users

If you're migrating from tmux/screen:

```bash
# Create an alias that works in both
alias tab-notify='if [ -n "$ZELLIJ" ]; then zellij pipe -n "notify" "stop"; fi'

# Use in your scripts
make build && tab-notify
```

### Docker Build Notifications

```bash
#!/bin/bash
# docker-build.sh

# Source the helper function
zellij-notify() {
    zellij pipe -n "notify" -a "pane_id=$ZELLIJ_PANE_ID" "${1:-}"
}

IMAGE_NAME="myapp"
TAG="latest"

echo "Building Docker image..."
zellij-notify "notification"

if docker build -t "$IMAGE_NAME:$TAG" .; then
    echo "✓ Docker build successful"
    zellij-notify "stop"
else
    echo "✗ Docker build failed"
    zellij-notify "subagent-stop"
    exit 1
fi
```

### Background Job Tracking

```bash
# Source the helper function
zellij-notify() {
    zellij pipe -n "notify" -a "pane_id=$ZELLIJ_PANE_ID" "${1:-}"
}

# Start background job with notification
long_running_task &
job_pid=$!

# Monitor the job
wait $job_pid
if [ $? -eq 0 ]; then
    zellij-notify "stop"
else
    zellij-notify "subagent-stop"
fi
```

## Tips and Tricks

### 1. Silent Mode for Scripts

If you want conditional notifications:

```bash
NOTIFY=${NOTIFY:-true}

# Source the helper function
zellij-notify() {
    zellij pipe -n "notify" -a "pane_id=$ZELLIJ_PANE_ID" "${1:-}"
}

notify_if_enabled() {
    if [ "$NOTIFY" = "true" ]; then
        zellij-notify "$1"
    fi
}

# Usage
notify_if_enabled "stop"

# Disable for a single run
NOTIFY=false ./my-script.sh
```

### 2. Emoji Priority System

Use different emojis for different priority levels:

```kdl
presets r#"{
    "p0-critical": {"emoji": "🚨"},
    "p1-high": {"emoji": "🔴"},
    "p2-medium": {"emoji": "⚠️"},
    "p3-low": {"emoji": "ℹ️"}
}"#
```

### 3. Time-based Status

Show how long something took:

```bash
start_time=$(date +%s)
cargo build --release
end_time=$(date +%s)
duration=$((end_time - start_time))

if [ $? -eq 0 ]; then
    zellij pipe -n "notify" "stop"
    echo "Build completed in ${duration}s"
else
    zellij pipe -n "notify" "subagent-stop"
fi
```

### 4. Clean Before Important Commands

Ensure a clean slate before running:

```bash
# Switch tabs to trigger auto-cleanup, then run command
zellij action go-to-next-tab
sleep 0.1
zellij action go-to-previous-tab
zellij pipe -n "notify" "notification"
cargo build
```

## Troubleshooting

### Emoji Not Appearing

1. Check that the plugin is loaded:
   ```bash
   zellij action list-plugins
   ```

2. Enable debug logging in config:
   ```kdl
   "file:~/.config/zellij/plugins/zellij-notify.wasm" {
       debug "true"
   }
   ```

3. View logs:
   ```bash
   task logs
   # or manually find the log
   find /tmp /var/folders -name "zellij.log" 2>/dev/null
   ```

### Emoji Not Cleaning

The auto-cleanup only triggers when you **switch to** a tab. If you're already on the tab, it won't clean until you leave and return.

### Unknown Preset Error

If you see ❓ instead of your expected emoji, check:
1. The preset name matches exactly (case-sensitive)
2. The JSON syntax is valid in your config
3. The preset is defined in the config

## Further Reading

- [Zellij Documentation](https://zellij.dev/documentation/)
- [Zellij Pipe Commands](https://zellij.dev/documentation/cli-actions#pipe)
- [Plugin Development Guide](CLAUDE.md)
