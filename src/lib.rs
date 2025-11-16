use std::collections::{BTreeMap, HashMap};
use zellij_tile::prelude::*;
use serde::Deserialize;

const VERSION: &str = env!("CARGO_PKG_VERSION");

// Manual WASM entry point for cdylib
#[no_mangle]
pub unsafe extern "C" fn _start() {}

#[derive(Deserialize, Clone)]
struct PresetConfig {
    emoji: String,
}

#[derive(Default)]
struct State {
    all_tabs: Vec<TabInfo>,  // Store ALL tabs, not just the active one
    focused_tab_position: Option<usize>,  // Track which tab is currently focused
    pane_manifest: Option<PaneManifest>,  // Map panes to their tab positions
    presets: HashMap<String, PresetConfig>,
    debug: bool,
}

register_plugin!(State);

impl ZellijPlugin for State {
    fn load(&mut self, configuration: BTreeMap<String, String>) {
        // Parse debug flag from config (default: false)
        self.debug = configuration.get("debug")
            .and_then(|s| s.parse().ok())
            .unwrap_or(false);

        if self.debug {
            eprintln!("[zellij-notify] 🚀 Plugin loaded - Version {}", VERSION);
        }

        subscribe(&[EventType::TabUpdate, EventType::PaneUpdate]);
        request_permission(&[
            PermissionType::ReadApplicationState,
            PermissionType::ChangeApplicationState
        ]);

        // Parse presets from config
        if let Some(presets_json) = configuration.get("presets") {
            match serde_json::from_str(presets_json) {
                Ok(presets) => {
                    self.presets = presets;
                    if self.debug {
                        eprintln!("[zellij-notify] ✅ Loaded {} presets from config", self.presets.len());
                    }
                }
                Err(e) => {
                    if self.debug {
                        eprintln!("[zellij-notify] ⚠️  Failed to parse presets: {}", e);
                    }
                }
            }
        }
    }

    fn update(&mut self, event: Event) -> bool {
        match event {
            Event::TabUpdate(tabs) => {
                if self.debug {
                    eprintln!("[zellij-notify] v{}", VERSION);
                    eprintln!("[zellij-notify] 📋 TAB UPDATE: {} tabs total", tabs.len());
                }

                // Store ALL tabs (not just the active one)
                self.all_tabs = tabs.clone();

                // Find the currently focused tab
                for (idx, tab) in tabs.iter().enumerate() {
                    if tab.active {
                        let is_new_focus = self.focused_tab_position != Some(tab.position);

                        // Only clean emojis when first focusing on a tab (prevents loops)
                        if is_new_focus {
                            if self.debug {
                                eprintln!("[zellij-notify] 🎯 FOCUS: Tab {} '{}' (idx={}, previous: {:?})",
                                    tab.position, tab.name, idx, self.focused_tab_position);
                            }

                            self.focused_tab_position = Some(tab.position);

                            // Check if this tab has emojis
                            let cleaned = remove_trailing_emojis(&tab.name);
                            if cleaned != tab.name {
                                if self.debug {
                                    eprintln!("[zellij-notify] 🔄 CLEAN: '{}' → '{}'", tab.name, cleaned);
                                }

                                // Zellij uses 1-based indexing, tab.position is 0-based
                                let tab_index = tab.position as u32 + 1;
                                rename_tab(tab_index, cleaned);
                            }
                        }
                        break;
                    }
                }
                false
            }
            Event::PaneUpdate(pane_manifest) => {
                if self.debug {
                    eprintln!("[zellij-notify] 🗂️  PANE UPDATE: Received PaneManifest");
                    eprintln!("[zellij-notify]   Number of tabs with panes: {}", pane_manifest.panes.len());
                }

                // Store the pane manifest so we can map pane IDs to tabs
                self.pane_manifest = Some(pane_manifest);
                false
            }
            _ => false
        }
    }

    fn render(&mut self, _rows: usize, _cols: usize) {}

    fn pipe(&mut self, pipe_message: PipeMessage) -> bool {
        // Only handle "notify" commands
        if pipe_message.name != "notify" {
            return false;
        }

        if self.debug {
            eprintln!("[zellij-notify] 📨 PIPE received!");
            eprintln!("[zellij-notify]   Name: {}", pipe_message.name);
            eprintln!("[zellij-notify]   Payload: {:?}", pipe_message.payload);
            eprintln!("[zellij-notify]   Source: {:?}", pipe_message.source);
            eprintln!("[zellij-notify]   Args: {:?}", pipe_message.args);
            eprintln!("[zellij-notify]   Is Private: {}", pipe_message.is_private);

            // Log session_name and tab_name if provided
            if let Some(session_name) = pipe_message.args.get("session_name") {
                eprintln!("[zellij-notify]   Session name: {}", session_name);
            }
            if let Some(tab_name) = pipe_message.args.get("tab_name") {
                eprintln!("[zellij-notify]   Tab name: {}", tab_name);
            }

            eprintln!("[zellij-notify]   Currently focused tab: {:?}", self.focused_tab_position);
            eprintln!("[zellij-notify]   All tabs at pipe time:");
            for tab in &self.all_tabs {
                eprintln!("[zellij-notify]     - Tab {}: '{}' (active={}, is_sync_panes_active={})",
                    tab.position, tab.name, tab.active, tab.is_sync_panes_active);
            }
        }

        // Get preset based on payload (positional argument)
        let preset = match pipe_message.payload.as_deref() {
            None | Some("") => {
                if self.debug {
                    eprintln!("[zellij-notify] ✅ Using default preset");
                }
                PresetConfig { emoji: "✅".to_string() }
            }
            Some(key) => {
                match self.presets.get(key) {
                    Some(preset) => {
                        if self.debug {
                            eprintln!("[zellij-notify] 📦 Using preset '{}': {}", key, preset.emoji);
                        }
                        preset.clone()
                    }
                    None => {
                        if self.debug {
                            eprintln!("[zellij-notify] ❓ Unknown preset '{}', using fallback", key);
                        }
                        PresetConfig { emoji: "❓".to_string() }
                    }
                }
            }
        };

        let emoji = &preset.emoji;

        // Try to identify which tab sent the pipe command
        // Method 1: Check if pane_id was passed via args (from shell wrapper)
        let target_tab_position = if let Some(pane_id) = pipe_message.args.get("pane_id") {
            if self.debug {
                eprintln!("[zellij-notify] 🆔 Pane ID provided: {}", pane_id);
            }

            // Use PaneManifest to find which tab contains this pane
            if let Some(ref manifest) = self.pane_manifest {
                // PaneManifest.panes is a BTreeMap<usize, Vec<PaneInfo>>
                // where the key is the tab position (0-indexed)
                let mut found_tab: Option<usize> = None;
                for (tab_position, panes) in &manifest.panes {
                    // Check if any pane in this tab matches our pane_id
                    for pane in panes {
                        if pane.id.to_string() == *pane_id {
                            found_tab = Some(*tab_position);
                            if self.debug {
                                eprintln!("[zellij-notify] ✅ Found pane {} in tab {}", pane_id, tab_position);
                            }
                            break;
                        }
                    }
                    if found_tab.is_some() {
                        break;
                    }
                }

                if found_tab.is_none() && self.debug {
                    eprintln!("[zellij-notify] ⚠️  Pane ID {} not found in PaneManifest", pane_id);
                }

                found_tab
            } else {
                if self.debug {
                    eprintln!("[zellij-notify] ⚠️  No PaneManifest available yet");
                }
                None
            }
        } else if let Some(pos_str) = pipe_message.args.get("tab_position") {
            // Method 2: Check if tab position was explicitly passed via args
            if self.debug {
                eprintln!("[zellij-notify] 🎯 Tab position explicitly provided: {}", pos_str);
            }
            pos_str.parse::<usize>().ok()
        } else {
            // Method 3: Fall back to the currently active tab from our stored state
            // This is NOT reliable for background commands but works for immediate commands
            let active_tab = self.all_tabs.iter().find(|t| t.active);
            if self.debug {
                if let Some(tab) = active_tab {
                    eprintln!("[zellij-notify] 🎯 Using active tab from state: {} '{}'",
                        tab.position, tab.name);
                } else {
                    eprintln!("[zellij-notify] ⚠️  No active tab found in state");
                }
            }
            active_tab.map(|t| t.position)
        };

        // Update the identified tab
        if let Some(position) = target_tab_position {
            if let Some(tab) = self.all_tabs.iter().find(|t| t.position == position) {
                let cleaned_name = remove_trailing_emojis(&tab.name);
                let new_name = format!("{} {}", cleaned_name, emoji);

                if self.debug {
                    eprintln!("[zellij-notify] 📝 Renaming tab {}: '{}' → '{}'",
                        tab.position, tab.name, new_name);

                    // Summary log: TAB_NAME in SESSION_NAME EMOJI
                    let session_name = pipe_message.args.get("session_name")
                        .map(|s| s.as_str())
                        .unwrap_or("unknown");
                    eprintln!("[zellij-notify] 📍 {} in {} {}",
                        cleaned_name, session_name, emoji);
                }

                // Zellij uses 1-based indexing, position is 0-based
                let tab_index = position as u32 + 1;
                rename_tab(tab_index, new_name);
            } else {
                if self.debug {
                    eprintln!("[zellij-notify] ⚠️  Tab at position {} not found in stored tabs", position);
                }
            }
        } else {
            if self.debug {
                eprintln!("[zellij-notify] ⚠️  Could not identify target tab");
            }
        }

        false // No UI re-render needed
    }
}

fn remove_trailing_emojis(name: &str) -> String {
    let emojis = ["🔴", "✅", "❌", "⚠️", "⚡", "💼", "🎉", "❓"];
    let mut cleaned = name.to_string();

    // Keep removing trailing emojis and whitespace
    loop {
        let original_len = cleaned.len();
        cleaned = cleaned.trim_end().to_string();

        // Try to remove any trailing emoji (check all emojis, don't break early)
        let mut found_emoji = false;
        for emoji in emojis {
            if cleaned.ends_with(emoji) {
                cleaned = cleaned[..cleaned.len() - emoji.len()].to_string();
                found_emoji = true;
                break; // Found one, now trim again and recheck from the start
            }
        }

        // If nothing changed (no whitespace trimmed, no emoji removed), we're done
        if !found_emoji && cleaned.len() == original_len {
            break;
        }
    }

    cleaned
}
