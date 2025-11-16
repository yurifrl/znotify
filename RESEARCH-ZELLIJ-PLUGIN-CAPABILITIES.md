# Comprehensive Research Report: Zellij Tab Names, Pipe Messages, and Environment Variables

**Date**: 2025-11-15
**Context**: Research for zellij-notify plugin development

---

## 1. Getting Current Tab Name in Zellij

### A. From Terminal/CLI (Outside Plugin)

**Status**: ❌ No `ZELLIJ_TAB_NAME` environment variable exists

**Available Environment Variables**:
- `ZELLIJ_SESSION_NAME` - Contains the session name
- `ZELLIJ` - Set to `0` inside a Zellij session
- `ZELLIJ_PANE_ID` - Contains the pane ID

**CLI Actions Available**:

1. **`zellij action query-tab-names`**
   - Queries all tab names and outputs a textual list
   - Returns a parseable list of tab names
   - Use case: Check if tabs exist before performing operations
   - **Example**:
     ```bash
     zellij action query-tab-names
     ```
   - **Reference**: https://zellij.dev/documentation/cli-actions

2. **`zellij action go-to-tab-name <name>`**
   - Navigate to tab by name

3. **`zellij action rename-tab <name>`**
   - Rename the current tab

**Workarounds for Dynamic Tab Naming**:
- Users commonly create shell functions using `zellij action rename-tab` to set tab names based on current directory or running processes
- No built-in way to retrieve the **current** tab's name directly from shell without using `query-tab-names` and parsing the output

**References**:
- https://zellij.dev/documentation/cli-actions
- https://zellij.dev/documentation/integration.html
- https://github.com/zellij-org/zellij/discussions/2889

---

### B. From Within a Plugin

**Primary Method**: Subscribe to `TabUpdate` Event

Plugins can get tab information by subscribing to the `TabUpdate` event, which provides:
- **Tab positions** - Where each tab is located (0-based index)
- **Tab names** - The identifier/label for each tab
- **Active status** - Which tab is currently focused
- **Fullscreen status** - Whether a tab contains a fullscreen pane
- **Hidden panes** - How many hidden panes each tab contains
- **Swap layouts** - Information on available swap layouts

**Permission Required**: `ReadApplicationState`

**Implementation Example**:
```rust
impl ZellijPlugin for State {
    fn load(&mut self, _configuration: BTreeMap<String, String>) {
        // Subscribe to tab updates
        subscribe(&[EventType::TabUpdate]);
        request_permission(&[PermissionType::ReadApplicationState]);
    }

    fn update(&mut self, event: Event) -> bool {
        match event {
            Event::TabUpdate(tabs) => {
                // tabs: Vec<TabInfo>
                // Each TabInfo contains: position, name, active, etc.
                for tab in &tabs {
                    eprintln!("Tab {}: '{}'", tab.position, tab.name);
                }
                false
            }
            _ => false
        }
    }
}
```

**Related Plugin API Commands**:
- `go_to_tab_name(name)` - Change focus to tab with specified name
- `focus_or_create_tab(name)` - Focus tab by name or create it if doesn't exist
- `rename_tab(position, name)` - Change the name of tab at position (1-based!)
- `switch_tab_to(index)` - Change focused tab to specified index (1-based)
- `toggle_tab()` - Focus previously focused tab

**Important Note**: Unlike the CLI, there's no plugin API command to directly query tab names on-demand. Plugins must subscribe to `TabUpdate` events and maintain state based on received updates.

**References**:
- https://zellij.dev/documentation/plugin-api-events
- https://zellij.dev/documentation/plugin-api-commands

---

## 2. Plugin Pipe Message Data

### Complete PipeMessage Struct

Based on Zellij documentation and `zellij_tile` crate, the `PipeMessage` struct contains:

```rust
pub struct PipeMessage {
    pub source: PipeSource,
    pub name: String,
    pub payload: Option<String>,
    pub args: HashMap<String, String>,
    pub is_private: bool,
}
```

### Field Descriptions

#### 1. `source: PipeSource`

The origin of the pipe message.

**Enum variants**:
```rust
pub enum PipeSource {
    Cli(String),    // From CLI, contains pipe ID (UUID)
    Plugin(u32),    // From another plugin, contains plugin ID
    Keybind,        // From a keybinding action
}
```

**Examples**:
- `Cli("c82fc6e8-064f-4c91-b494-912041339867")` - User ran `zellij pipe` command
- `Plugin(42)` - Another plugin sent the message
- `Keybind` - Triggered by keyboard shortcut

#### 2. `name: String`

The name identifier of the pipe.

**How it's set**:
- Explicitly provided by user via `-n` flag: `zellij pipe -n "notify"`
- If not provided, Zellij auto-assigns a random UUID

**Usage**: Plugins use this to filter which pipes they handle:
```rust
fn pipe(&mut self, pipe_message: PipeMessage) -> bool {
    if pipe_message.name != "notify" {
        return false; // Ignore pipes not meant for us
    }
    // Handle the pipe...
}
```

#### 3. `payload: Option<String>`

Optional arbitrary string data - the main content being piped.

**How it's set**: The positional argument after all flags
```bash
zellij pipe -n "notify" "this is the payload"
#                        ^^^^^^^^^^^^^^^^^^^^
```

**Common uses**:
- Command names: `"stop"`, `"start"`, `"error"`
- Short messages: `"Build complete"`
- JSON data: `'{"status": "ok", "count": 42}'`
- Empty/None: Just using args for all data

#### 4. `args: HashMap<String, String>`

Key-value arguments for the pipe.

**How it's set**: Via `-a` flag (can be used multiple times)
```bash
zellij pipe -n "notify" \
    -a "pane_id=$ZELLIJ_PANE_ID" \
    -a "session_name=$ZELLIJ_SESSION_NAME" \
    -a "tab_name=my-tab" \
    "stop"
```

Results in:
```rust
args: {
    "pane_id": "123",
    "session_name": "dotfiles",
    "tab_name": "my-tab",
}
```

**Access in plugin**:
```rust
if let Some(pane_id) = pipe_message.args.get("pane_id") {
    eprintln!("Pane ID: {}", pane_id);
}
```

#### 5. `is_private: bool`

Controls message broadcast behavior.

**Values**:
- `false` (default) - Broadcast to **all** running plugins
- `true` - Directed specifically at **one** plugin (requires additional setup)

**CLI usage**:
```bash
# Broadcast to all plugins (default)
zellij pipe -n "notify" "stop"

# Private message (requires pipe_id and plugin configuration)
# (Advanced usage, rarely needed)
```

### Complete Example

**CLI Command**:
```bash
zellij pipe -n "notify" -a "pane_id=$ZELLIJ_PANE_ID" -a "tab_position=0" "stop"
```

**Received PipeMessage**:
```rust
PipeMessage {
    source: Cli("c82fc6e8-064f-4c91-b494-912041339867"),
    name: "notify",
    payload: Some("stop"),
    args: {
        "pane_id": "123",
        "tab_position": "0",
    },
    is_private: false,
}
```

**Plugin Implementation**:
```rust
fn pipe(&mut self, pipe_message: PipeMessage) -> bool {
    // Only handle "notify" pipes
    if pipe_message.name != "notify" {
        return false;
    }

    // Check the source
    match pipe_message.source {
        PipeSource::Cli(ref pipe_id) => {
            eprintln!("Received from CLI: {}", pipe_id);
        }
        PipeSource::Plugin(plugin_id) => {
            eprintln!("Received from plugin: {}", plugin_id);
        }
        PipeSource::Keybind => {
            eprintln!("Received from keybinding");
        }
    }

    // Get the command/preset from payload
    let command = pipe_message.payload.as_deref().unwrap_or("default");
    eprintln!("Command: {}", command);

    // Get arguments
    if let Some(pane_id) = pipe_message.args.get("pane_id") {
        eprintln!("Pane ID: {}", pane_id);
    }

    // Process the pipe...

    false // No UI re-render needed
}
```

### Additional Pipe Capabilities

**Backpressure Control**:
```rust
// Block CLI pipe input temporarily
block_cli_pipe_input(pipe_id);

// Unblock when ready
unblock_cli_pipe_input(pipe_id);
```

**Write to CLI Pipe's STDOUT**:
```rust
// Send output back to the CLI caller
write_to_cli_pipe(pipe_id, "Processing complete");
```

**Plugin-to-Plugin Communication**:
```rust
// Send pipe to another plugin
let message = PipeMessage {
    name: "data_sync".to_string(),
    payload: Some("update".to_string()),
    args: HashMap::new(),
    is_private: false,
};
pipe_message_to_plugin(message);
```

**References**:
- https://zellij.dev/documentation/plugin-pipes
- https://zellij.dev/documentation/zellij-pipe
- https://docs.rs/zellij-tile/latest/zellij_tile/prelude/enum.PipeSource.html
- https://github.com/zellij-org/rust-plugin-example

---

## 3. Environment Variables in Plugins

### WASI Environment Model

Zellij plugins are compiled to `wasm32-wasip1` (WebAssembly System Interface) and run in a sandboxed environment.

### Key Findings

#### 1. No Automatic Inheritance

**Critical**: WASI modules do **NOT** automatically inherit the host's environment variables.

- By default, WASI programs start with a **blank environment** (`{}`)
- Environment variables must be **explicitly provided** by the WASI host/runtime when instantiating the module
- This is intentional for security and sandboxing

#### 2. How `std::env::var()` Works in Plugins

When you call `std::env::var("SOME_VAR")` in a Zellij plugin:

```rust
// In plugin code
if let Ok(value) = std::env::var("ZELLIJ_PANE_ID") {
    eprintln!("Pane ID: {}", value); // ❌ This will NOT work!
}
```

**What happens**:
- The function accesses the **plugin's isolated environment**
- This is **NOT** the same as the calling pane's environment
- The plugin only sees variables that Zellij explicitly passed to the WASM runtime when loading the plugin

**Implications**:
- ❌ Cannot access `$ZELLIJ_PANE_ID` from the calling terminal
- ❌ Cannot access `$ZELLIJ_SESSION_NAME` from the calling terminal
- ❌ Cannot access any environment variables from the pane that executed `zellij pipe`

#### 3. What Plugins CAN Access

**Configuration Variables**:
- Passed via plugin config in KDL layout files
- Available in the `load()` method's `configuration` parameter

```kdl
plugin location="file:/path/to/plugin.wasm" {
    my_config_var "value"
    another_var "123"
}
```

```rust
fn load(&mut self, configuration: BTreeMap<String, String>) {
    if let Some(value) = configuration.get("my_config_var") {
        eprintln!("Config: {}", value); // ✅ This works!
    }
}
```

**Zellij-Provided Variables**:
- Any environment variables Zellij explicitly sets when loading the plugin
- What exactly Zellij provides is internal implementation detail
- Not documented publicly

**NOT Accessible**:
- ❌ Environment variables from the terminal pane
- ❌ Shell variables (`$HOME`, `$PATH`, etc.)
- ❌ User-defined variables from calling context

#### 4. WASI Architecture Details

**How WASI Environment Works**:
- Environment access uses WASI syscalls: `environ_get` and `environ_sizes_get`
- The host (Zellij) controls what environment the plugin sees
- The WASI runtime maintains an isolated environment map
- No inheritance from host process

**Security Model**:
- Sandboxing prevents plugins from accessing arbitrary system resources
- Plugins cannot read files outside allowed directories
- Plugins cannot access host environment without explicit permission
- This isolation is a core WASI design principle

### Implications for Plugin Development

#### Problem Scenario

User runs command in terminal pane:
```bash
# In a terminal pane with these env vars:
# ZELLIJ_PANE_ID=123
# ZELLIJ_SESSION_NAME=dotfiles
# MY_CUSTOM_VAR=hello

zellij pipe -n "notify" "stop"
```

Plugin tries to access environment:
```rust
fn pipe(&mut self, pipe_message: PipeMessage) -> bool {
    // ❌ This will NOT see the pane's ZELLIJ_PANE_ID!
    match std::env::var("ZELLIJ_PANE_ID") {
        Ok(id) => eprintln!("Pane ID: {}", id),
        Err(_) => eprintln!("No pane ID in environment"),
    }
    // Will always hit the Err branch
}
```

#### Solution Approaches

**1. Pass Data via Pipe Args** (Recommended):
```bash
# Explicitly pass needed data
zellij pipe -n "notify" \
    -a "pane_id=$ZELLIJ_PANE_ID" \
    -a "session_name=$ZELLIJ_SESSION_NAME" \
    "stop"
```

```rust
fn pipe(&mut self, pipe_message: PipeMessage) -> bool {
    // ✅ Access via args
    if let Some(pane_id) = pipe_message.args.get("pane_id") {
        eprintln!("Pane ID: {}", pane_id);
    }
}
```

**2. Use Plugin API Events**:
```rust
// Subscribe to events that provide needed information
subscribe(&[EventType::TabUpdate, EventType::PaneUpdate]);

fn update(&mut self, event: Event) -> bool {
    match event {
        Event::TabUpdate(tabs) => {
            // ✅ Get tab names from event
            for tab in tabs {
                self.tab_names.insert(tab.position, tab.name);
            }
        }
        Event::PaneUpdate(manifest) => {
            // ✅ Get pane-to-tab mapping
            self.pane_manifest = Some(manifest);
        }
        _ => {}
    }
    false
}
```

**3. Use PaneManifest for Mapping**:
```rust
// Map pane IDs to tab positions using PaneManifest
if let Some(ref manifest) = self.pane_manifest {
    for (tab_position, panes) in &manifest.panes {
        for pane in panes {
            if pane.id.to_string() == pane_id {
                // ✅ Found which tab this pane belongs to
                eprintln!("Pane {} is in tab {}", pane_id, tab_position);
            }
        }
    }
}
```

### Comparison Table

| Method | Calling Pane | Plugin |
|--------|--------------|--------|
| `$ZELLIJ_PANE_ID` | ✅ Available | ❌ Not available via `env::var()` |
| `$ZELLIJ_SESSION_NAME` | ✅ Available | ❌ Not available via `env::var()` |
| Custom env vars | ✅ Available | ❌ Not available via `env::var()` |
| Pipe args (`-a` flags) | ✅ Can set | ✅ Can read via `pipe_message.args` |
| Plugin events | ❌ N/A | ✅ Subscribe to `TabUpdate`, `PaneUpdate`, etc. |
| Plugin config (KDL) | ❌ N/A | ✅ Available in `load()` method |

### References

- https://doc.rust-lang.org/nightly/rustc/platform-support/wasm32-wasip1.html
- https://github.com/WebAssembly/wasi-libc/issues/181
- https://wasmedge.org/docs/develop/rust/os/
- https://www.secondstate.io/articles/wasi-access-system-resources/
- https://docs.rs/zellij-tile/latest/zellij_tile/

---

## Summary

### Getting Current Tab Name

| Context | Method | Status |
|---------|--------|--------|
| Terminal/CLI | `$ZELLIJ_TAB_NAME` env var | ❌ Does not exist |
| Terminal/CLI | `zellij action query-tab-names` | ✅ Returns all tab names |
| Plugin | Subscribe to `TabUpdate` event | ✅ Provides all tab info |
| Plugin | Direct query command | ❌ Not available |

**Best Practice**: Plugins should subscribe to `TabUpdate` and maintain tab state internally.

### PipeMessage Data

Complete struct with 5 fields:
1. `source: PipeSource` - Where the pipe came from (CLI/Plugin/Keybind)
2. `name: String` - Pipe identifier (from `-n` flag)
3. `payload: Option<String>` - Main content (positional arg)
4. `args: HashMap<String, String>` - Key-value pairs (from `-a` flags)
5. `is_private: bool` - Broadcast vs targeted delivery

**Best Practice**: Use `args` to pass structured data to plugins.

### Environment Variables in Plugins

| Capability | Status | Reason |
|------------|--------|--------|
| Access calling pane's env | ❌ Not possible | WASI sandboxing |
| Access `$ZELLIJ_PANE_ID` via `env::var()` | ❌ Not available | Isolated environment |
| Read plugin config from KDL | ✅ Works | Passed to `load()` |
| Receive data via pipe args | ✅ Works | Access via `pipe_message.args` |
| Subscribe to Zellij events | ✅ Works | Use `TabUpdate`, `PaneUpdate`, etc. |

**Key Insight**: Plugins run in isolated WASI sandbox and cannot access the calling pane's environment. All data must be explicitly passed via pipe args or plugin API events.

**Best Practice**: Always pass needed context explicitly:
```bash
zellij pipe -n "notify" \
    -a "pane_id=$ZELLIJ_PANE_ID" \
    -a "session_name=$ZELLIJ_SESSION_NAME" \
    "command"
```

---

## Conclusion

This research confirms:

1. **No `$ZELLIJ_TAB_NAME` exists** - Must use `query-tab-names` action or subscribe to `TabUpdate` events
2. **PipeMessage is fully documented** - All 5 fields available with clear semantics
3. **Plugins are isolated** - Cannot access caller's environment, must use explicit data passing

The zellij-notify plugin's current approach of using:
- Pipe args for pane_id and tab_name
- PaneManifest for pane-to-tab mapping
- TabUpdate events for tab information

...is the correct architectural pattern for Zellij plugin development.
