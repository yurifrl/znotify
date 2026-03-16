# zellij-notify

A Zellij plugin that adds status emojis to tab names and automatically removes them when focus changes or state becomes stale.

## Features

### 🎯 Pipe Command Integration
Append a status emoji to the tab that owns the pane:

```bash
# Preferred: put the emoji directly in the pipe name
zellij pipe --name "notify::✅::$ZELLIJ_PANE_ID" -- ""
zellij pipe --name "notify::⚡::$ZELLIJ_PANE_ID" -- ""
zellij pipe --name "notify::🔴::$ZELLIJ_PANE_ID" -- ""

# Preset keys also work
zellij pipe --name "notify::stop::$ZELLIJ_PANE_ID" -- ""
zellij pipe --name "notify::notification::$ZELLIJ_PANE_ID" -- ""
zellij pipe --name "notify::subagent-stop::$ZELLIJ_PANE_ID" -- ""
```

This works well with shell scripts, hooks, and background commands.

### 🧹 Auto-Cleanup
The plugin removes trailing status emojis when they become stale, so tab names do not accumulate old status markers.

Known icons cleaned automatically include: `✅ ❌ 🔴 ⚠️ ⚡ 💼 🎉 ❓ ⏳`

### ⚙️ Configurable Presets
You can still define named presets in Zellij config and trigger them by key, while using raw emoji names when convenient.

## Installation

### Prerequisites
- Rust toolchain with `wasm32-wasip1` target
- [Task](https://taskfile.dev)
- Zellij terminal multiplexer

Add the WASM target if needed:

```bash
rustup target add wasm32-wasip1
```

### Build, Install, and Reload

Preferred development workflow:

```bash
task build
```

This builds the WASM plugin, installs it, clears logs, and reloads it in Zellij.

### Manual Build

If you only want the artifact:

```bash
cargo build --release --target wasm32-wasip1
```

Output:

```text
target/wasm32-wasip1/release/zellij_notify.wasm
```

## Configuration

Add to `~/.config/zellij/config.kdl`:

```kdl
load_plugins {
    "file:~/.config/zellij/plugins/zellij-notify.wasm" {
        debug "false"
        presets r#"{
            "notification": {"emoji": "⚡"},
            "posttooluse": {"emoji": "⚡"},
            "stop": {"emoji": "✅"},
            "subagent-stop": {"emoji": "🔴"}
        }"#
    }
}
```

### Config Keys
- `debug`: `"true"` enables verbose logging
- `presets`: JSON object mapping preset name to `{ "emoji": "..." }`

Built-in preset aliases include:
- `notification` → `⚡`
- `posttooluse` → `⚡`
- `stop` → `✅`
- `subagent-stop` → `🔴`
- `waiting` → `⏳`
- `completed` → `✅`

## Usage

### Basic Pipe Commands

```bash
# Preferred: direct emoji format
zellij pipe --name "notify::✅::$ZELLIJ_PANE_ID" -- ""  # tab -> "myproject ✅"
zellij pipe --name "notify::⚡::$ZELLIJ_PANE_ID" -- ""  # tab -> "myproject ⚡"
zellij pipe --name "notify::🔴::$ZELLIJ_PANE_ID" -- ""  # tab -> "myproject 🔴"

# Preset keys also work
zellij pipe --name "notify::stop::$ZELLIJ_PANE_ID" -- ""
zellij pipe --name "notify::notification::$ZELLIJ_PANE_ID" -- ""
zellij pipe --name "notify::subagent-stop::$ZELLIJ_PANE_ID" -- ""

# Legacy format is still supported
zellij pipe -n "notify" -a "pane_id=$ZELLIJ_PANE_ID" "stop"
zellij pipe -n "notify" -a "pane_id=$ZELLIJ_PANE_ID" "notification"
```

### Why include `pane_id`?

When a command finishes in the background, you may already be on another tab. Including `ZELLIJ_PANE_ID` lets the plugin map the event back to the correct tab.

With the preferred format, the pane id is embedded in the pipe name:

```bash
zellij pipe --name "notify::✅::$ZELLIJ_PANE_ID" -- ""
```

With the legacy format, it is passed as an argument:

```bash
zellij pipe -n "notify" -a "pane_id=$ZELLIJ_PANE_ID" "stop"
```

### Custom Presets

Define your own presets:

```kdl
presets r#"{
    "success": {"emoji": "🎉"},
    "error": {"emoji": "💥"},
    "warning": {"emoji": "⚠️"},
    "info": {"emoji": "ℹ️"}
}"#
```

Then trigger them by key:

```bash
zellij pipe --name "notify::success::$ZELLIJ_PANE_ID" -- ""
```

You can also skip presets and send the emoji directly:

```bash
zellij pipe --name "notify::🎉::$ZELLIJ_PANE_ID" -- ""
```

### Claude Code Hook Integration

Example `~/.claude/settings.json`:

```json
{
  "hooks": {
    "PostToolUse": [{
      "matcher": "",
      "hooks": [{
        "type": "command",
        "command": "zellij pipe --name \"notify::⚡::$ZELLIJ_PANE_ID\" -- \"\""
      }]
    }],
    "Stop": [{
      "matcher": "",
      "hooks": [{
        "type": "command",
        "command": "zellij pipe --name \"notify::✅::$ZELLIJ_PANE_ID\" -- \"\""
      }]
    }]
  }
}
```

If you use the bundled CLI, these are equivalent:

```bash
znotify notify notification
znotify notify stop
znotify notify ✅
znotify notify ⚡
```

## How It Works

1. The plugin subscribes to `TabUpdate` and `PaneUpdate` events.
2. It keeps a pane-to-tab mapping from Zellij's `PaneManifest`.
3. A pipe message stores notification state per pane.
4. The latest notification for panes in a tab determines the tab suffix.
5. When state clears or becomes stale, the original tab name is restored.

## Development

```bash
# Build, install, and reload
task build

# View logs
task logs
```

## License

MIT License - see `LICENSE` for details.
