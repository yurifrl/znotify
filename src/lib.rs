pub mod config;
pub mod state;

use std::collections::{BTreeMap, HashMap, HashSet};
use zellij_tile::prelude::*;
use zellij_tile::shim::{rename_tab, unblock_cli_pipe_input};

use crate::config::NotificationConfig;
use crate::state::NotificationState;

#[no_mangle]
pub unsafe extern "C" fn _start() {}

#[derive(Default)]
pub struct State {
    permissions_granted: bool,
    pub(crate) tabs: Vec<TabInfo>,
    pub(crate) panes: PaneManifest,
    pub(crate) notification_state: HashMap<u32, NotificationState>,
    pub(crate) original_tab_names: HashMap<usize, String>,
    pub(crate) config: NotificationConfig,
    pub(crate) debug: bool,
    updating_tabs: bool,
    pub(crate) last_focused_pane_id: Option<u32>,
    /// Tab positions where we've issued a rename to strip stale icons.
    /// Prevents re-stripping on the bounced TabUpdate before Zellij catches up.
    pub(crate) pending_strips: HashSet<usize>,
    next_sequence: u64,
}

register_plugin!(State);

impl State {
    fn determine_focused_pane(&self) -> Option<u32> {
        let active_tab = self.tabs.iter().find(|t| t.active)?;
        let panes = self.panes.panes.get(&active_tab.position)?;
        let focused = panes.iter().find(|p| {
            !p.is_plugin
                && p.is_focused
                && (p.is_floating == active_tab.are_floating_panes_visible)
        })?;
        Some(focused.id)
    }

    /// Clears notifications for the pane that just lost focus.
    /// Returns true if any notification was cleared.
    pub(crate) fn check_and_clear_blur(&mut self) -> bool {
        let Some(focused_pane_id) = self.determine_focused_pane() else {
            return false;
        };

        if self.last_focused_pane_id == Some(focused_pane_id) {
            return false;
        }

        let previous_focused_pane_id = self.last_focused_pane_id.replace(focused_pane_id);

        if let Some(previous_focused_pane_id) = previous_focused_pane_id {
            if self
                .notification_state
                .remove(&previous_focused_pane_id)
                .is_some()
            {
                if self.debug {
                    eprintln!(
                        "[zellij-notify] Cleared notifications for blurred pane {}",
                        previous_focused_pane_id
                    );
                }
                return true;
            }
        }

        false
    }

    /// Removes notification entries for pane IDs that no longer exist.
    /// Returns true if any stale entries were removed.
    pub(crate) fn clean_stale_notifications(&mut self) -> bool {
        if self.notification_state.is_empty() || self.panes.panes.is_empty() {
            return false;
        }

        let current_pane_ids: HashSet<u32> = self
            .panes
            .panes
            .values()
            .flat_map(|panes| panes.iter().filter(|p| !p.is_plugin).map(|p| p.id))
            .collect();

        let stale_ids: Vec<u32> = self
            .notification_state
            .keys()
            .filter(|id| !current_pane_ids.contains(id))
            .copied()
            .collect();

        if stale_ids.is_empty() {
            return false;
        }

        for id in &stale_ids {
            self.notification_state.remove(id);
            if self.debug {
                eprintln!("[zellij-notify] Removed stale notification for pane {}", id);
            }
        }

        true
    }

    /// Returns true if there are original_tab_names entries waiting to be
    /// restored (ie. their tab positions have no active notifications).
    pub(crate) fn has_pending_restores(&self) -> bool {
        self.original_tab_names
            .keys()
            .any(|pos| self.get_tab_notification_state(*pos).is_none())
    }

    /// Returns true if any tab has a stale icon suffix with no active notification.
    pub(crate) fn has_stale_icons(&self) -> bool {
        for tab in &self.tabs {
            if self.get_tab_notification_state(tab.position).is_some() {
                continue;
            }
            if self.original_tab_names.contains_key(&tab.position) {
                continue; // handled by restore logic
            }
            if self.pending_strips.contains(&tab.position) {
                continue; // already issued a strip, waiting for Zellij to catch up
            }
            if self.tab_name_has_icon(&tab.name) {
                return true;
            }
        }
        false
    }

    /// Checks if a tab name ends with one of our notification icon suffixes.
    pub(crate) fn tab_name_has_icon(&self, name: &str) -> bool {
        self.config
            .all_icons()
            .into_iter()
            .any(|icon| name.ends_with(&format!(" {}", icon)))
    }

    /// Strips notification icon suffixes from a tab name.
    pub(crate) fn strip_icons(&self, name: &str) -> String {
        let icons = self.config.all_icons();
        let mut cleaned = name.to_string();

        loop {
            let original_len = cleaned.len();
            cleaned = cleaned.trim_end().to_string();

            let mut found_icon = false;
            for icon in &icons {
                let suffix = format!(" {}", icon);
                if cleaned.ends_with(&suffix) {
                    cleaned.truncate(cleaned.len() - suffix.len());
                    found_icon = true;
                    break;
                }
            }

            if !found_icon && cleaned.len() == original_len {
                break;
            }
        }

        cleaned
    }

    /// Returns the latest notification state for a tab.
    pub(crate) fn get_tab_notification_state(
        &self,
        tab_position: usize,
    ) -> Option<NotificationState> {
        let panes = self.panes.panes.get(&tab_position)?;

        panes
            .iter()
            .filter(|pane| !pane.is_plugin)
            .filter_map(|pane| self.notification_state.get(&pane.id))
            .max_by_key(|notification| notification.sequence)
            .cloned()
    }

    fn parse_pipe_message(pipe_message: &PipeMessage) -> Result<(String, u32), &'static str> {
        if pipe_message.name.starts_with("notify::") {
            let mut parts = pipe_message.name.splitn(3, "::");
            let _ = parts.next();
            let event_name = parts.next().ok_or("Missing event name in pipe name")?;
            let pane_id = parts
                .next()
                .ok_or("Missing pane_id in pipe name")?
                .parse::<u32>()
                .map_err(|_| "Invalid pane_id in pipe name")?;
            return Ok((event_name.to_string(), pane_id));
        }

        if let Some(ref payload) = pipe_message.payload {
            if payload.starts_with("notify::") {
                let mut parts = payload.splitn(3, "::");
                let _ = parts.next();
                let event_name = parts.next().ok_or("Missing event name in payload")?;
                let pane_id = parts
                    .next()
                    .ok_or("Missing pane_id in payload")?
                    .parse::<u32>()
                    .map_err(|_| "Invalid pane_id in payload")?;
                return Ok((event_name.to_string(), pane_id));
            }

            if pipe_message.name == "notify" {
                let pane_id = pipe_message
                    .args
                    .get("pane_id")
                    .ok_or("Missing pane_id arg")?
                    .parse::<u32>()
                    .map_err(|_| "Invalid pane_id arg")?;
                return Ok((payload.clone(), pane_id));
            }

            return Err("ignoring pipe, no notify:: prefix");
        }

        if pipe_message.name == "notify" {
            let pane_id = pipe_message
                .args
                .get("pane_id")
                .ok_or("Missing pane_id arg")?
                .parse::<u32>()
                .map_err(|_| "Invalid pane_id arg")?;
            return Ok((String::new(), pane_id));
        }

        Err("ignoring pipe, no match")
    }

    /// Updates tab names to show notification icons or restore original names.
    /// Only called when notification state changes (pipe received, notification cleared).
    fn update_tab_names(&mut self) {
        if self.updating_tabs || !self.config.enabled {
            return;
        }
        self.updating_tabs = true;

        let mut notified_positions: HashSet<usize> = HashSet::new();

        for tab in &self.tabs {
            if let Some(notification) = self.get_tab_notification_state(tab.position) {
                notified_positions.insert(tab.position);

                if !self.original_tab_names.contains_key(&tab.position) {
                    let original = if tab.name.is_empty() {
                        format!("Tab #{}", tab.position + 1)
                    } else {
                        self.strip_icons(&tab.name)
                    };
                    self.original_tab_names.insert(tab.position, original);
                }

                let original = self
                    .original_tab_names
                    .get(&tab.position)
                    .cloned()
                    .unwrap_or_else(|| format!("Tab #{}", tab.position + 1));
                let new_name = format!("{} {}", original, notification.emoji);

                if tab.name != new_name {
                    if self.debug {
                        eprintln!(
                            "[zellij-notify] RENAME tab pos={} '{}' -> '{}' ({})",
                            tab.position, tab.name, new_name, notification.name
                        );
                    }
                    rename_tab((tab.position + 1) as u32, &new_name);
                }
            }
        }

        // Restore original names for tabs whose notifications were cleared.
        let positions_to_restore: Vec<usize> = self
            .original_tab_names
            .keys()
            .filter(|pos| !notified_positions.contains(pos))
            .cloned()
            .collect();

        for pos in positions_to_restore {
            if let Some(tab) = self.tabs.iter().find(|t| t.position == pos) {
                if let Some(original_name) = self.original_tab_names.remove(&pos) {
                    if tab.name != original_name {
                        if self.debug {
                            eprintln!(
                                "[zellij-notify] RESTORE tab pos={} '{}' -> '{}'",
                                pos, tab.name, original_name
                            );
                        }
                        rename_tab((pos + 1) as u32, &original_name);
                    }
                }
            }
        }

        // Strip stale icons from tabs that have no notification and no pending restore.
        for tab in &self.tabs {
            if notified_positions.contains(&tab.position) {
                self.pending_strips.remove(&tab.position);
                continue;
            }
            if self.original_tab_names.contains_key(&tab.position) {
                self.pending_strips.remove(&tab.position);
                continue;
            }
            if self.pending_strips.contains(&tab.position) {
                if !self.tab_name_has_icon(&tab.name) {
                    self.pending_strips.remove(&tab.position);
                }
                continue;
            }
            if self.tab_name_has_icon(&tab.name) {
                let clean_name = self.strip_icons(&tab.name);
                if self.debug {
                    eprintln!(
                        "[zellij-notify] STRIP stale icon from tab pos={} '{}' -> '{}'",
                        tab.position, tab.name, clean_name
                    );
                }
                self.pending_strips.insert(tab.position);
                rename_tab((tab.position + 1) as u32, &clean_name);
            }
        }

        // Clean up cached names for tabs that no longer exist.
        if !self.tabs.is_empty() {
            let valid_positions: HashSet<usize> = self.tabs.iter().map(|t| t.position).collect();
            self.original_tab_names
                .retain(|pos, _| valid_positions.contains(pos));
            self.pending_strips.retain(|pos| valid_positions.contains(pos));
        }

        self.updating_tabs = false;
    }
}

impl ZellijPlugin for State {
    fn load(&mut self, configuration: BTreeMap<String, String>) {
        self.debug = configuration
            .get("debug")
            .and_then(|s| s.parse().ok())
            .unwrap_or(false);

        request_permission(&[
            PermissionType::ReadApplicationState,
            PermissionType::ChangeApplicationState,
            PermissionType::MessageAndLaunchOtherPlugins,
            PermissionType::ReadCliPipes,
        ]);

        subscribe(&[
            EventType::PermissionRequestResult,
            EventType::TabUpdate,
            EventType::PaneUpdate,
        ]);

        self.config = NotificationConfig::from_configuration(&configuration);

        if self.debug {
            eprintln!(
                "[zellij-notify] v{} loaded with {} presets",
                env!("CARGO_PKG_VERSION"),
                self.config.presets.len()
            );
        }
    }

    fn update(&mut self, event: Event) -> bool {
        match event {
            Event::PermissionRequestResult(status) => {
                self.permissions_granted = status == PermissionStatus::Granted;
                set_selectable(false);

                if self.debug {
                    eprintln!(
                        "[zellij-notify] permissions {:?}",
                        if self.permissions_granted {
                            "granted"
                        } else {
                            "denied"
                        }
                    );
                }

                // Strip any stale icons on startup.
                self.update_tab_names();
                true
            }
            Event::TabUpdate(tab_info) => {
                self.tabs = tab_info;
                let blur_cleared = self.check_and_clear_blur();
                let stale_cleaned = self.clean_stale_notifications();
                if blur_cleared
                    || stale_cleaned
                    || self.has_pending_restores()
                    || self.has_stale_icons()
                {
                    self.update_tab_names();
                }
                false
            }
            Event::PaneUpdate(pane_manifest) => {
                self.panes = pane_manifest;
                let blur_cleared = self.check_and_clear_blur();
                let stale_cleaned = self.clean_stale_notifications();
                if blur_cleared
                    || stale_cleaned
                    || self.has_pending_restores()
                    || self.has_stale_icons()
                {
                    self.update_tab_names();
                }
                false
            }
            _ => false,
        }
    }

    fn render(&mut self, _rows: usize, _cols: usize) {}

    fn pipe(&mut self, pipe_message: PipeMessage) -> bool {
        if self.debug {
            eprintln!(
                "[zellij-notify] pipe received name='{}' payload={:?} args={:?}",
                pipe_message.name, pipe_message.payload, pipe_message.args
            );
        }

        let parsed = Self::parse_pipe_message(&pipe_message);

        // Unblock the CLI pipe immediately so the caller never hangs.
        unblock_cli_pipe_input(&pipe_message.name);

        let (event_name, pane_id) = match parsed {
            Ok(parsed) => parsed,
            Err(err) => {
                if self.debug {
                    eprintln!("[zellij-notify] {}", err);
                }
                return false;
            }
        };

        let emoji = self.config.resolve_emoji(Some(&event_name));
        self.next_sequence = self.next_sequence.saturating_add(1);
        self.notification_state.insert(
            pane_id,
            NotificationState {
                name: if event_name.is_empty() {
                    "default".to_string()
                } else {
                    event_name.clone()
                },
                emoji: emoji.clone(),
                sequence: self.next_sequence,
            },
        );

        if self.debug {
            eprintln!(
                "[zellij-notify] Set pane {} -> '{}' ({})",
                pane_id,
                emoji,
                if event_name.is_empty() {
                    "default"
                } else {
                    &event_name
                }
            );
        }

        self.update_tab_names();
        false
    }
}
