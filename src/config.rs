//! User-configurable notification appearance.

use serde::Deserialize;
use std::collections::{BTreeMap, HashMap, HashSet};

/// Configuration for a single notification preset.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct PresetConfig {
    pub emoji: String,
}

/// Runtime configuration parsed from Zellij layout config.
#[derive(Debug, Clone)]
pub struct NotificationConfig {
    /// Whether notifications are enabled.
    pub enabled: bool,
    /// Preset name -> emoji configuration.
    pub presets: HashMap<String, PresetConfig>,
}

impl Default for NotificationConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            presets: default_presets(),
        }
    }
}

impl NotificationConfig {
    /// Parse configuration from Zellij layout configuration.
    ///
    /// Supported keys:
    /// - `enabled`: "true" enables notifications, anything else disables them
    /// - `presets`: JSON object mapping preset name to `{ "emoji": "..." }`
    pub fn from_configuration(config: &BTreeMap<String, String>) -> Self {
        let mut result = Self::default();

        if let Some(enabled) = config.get("enabled") {
            result.enabled = enabled == "true";
        }

        if let Some(presets_json) = config.get("presets") {
            match serde_json::from_str::<HashMap<String, PresetConfig>>(presets_json) {
                Ok(user_presets) => {
                    for (name, preset) in user_presets {
                        if preset.emoji.chars().count() > 4 {
                            eprintln!(
                                "zellij-notify: Warning: preset '{}' emoji '{}' is longer than 4 chars, may not display well",
                                name, preset.emoji
                            );
                        }
                        result.presets.insert(name, preset);
                    }
                }
                Err(err) => {
                    eprintln!("zellij-notify: Warning: failed to parse presets: {}", err);
                }
            }
        }

        result
    }

    /// Resolve either a preset name or a literal emoji/text payload.
    ///
    /// Empty/absent names fall back to the completed/success icon (✅ by default).
    /// Unknown names are passed through unchanged, which allows pipe names like
    /// `notify::✅::123`.
    pub fn resolve_emoji(&self, name: Option<&str>) -> String {
        match name.map(str::trim) {
            None | Some("") => self
                .presets
                .get("completed")
                .or_else(|| self.presets.get("stop"))
                .map(|preset| preset.emoji.clone())
                .unwrap_or_else(|| "✅".to_string()),
            Some(name) => self
                .presets
                .get(name)
                .map(|preset| preset.emoji.clone())
                .unwrap_or_else(|| name.to_string()),
        }
    }

    /// All known icons that should be stripped from tab suffixes.
    pub fn all_icons(&self) -> Vec<String> {
        let mut seen = HashSet::new();
        let mut icons = Vec::new();

        for preset in self.presets.values() {
            if seen.insert(preset.emoji.clone()) {
                icons.push(preset.emoji.clone());
            }
        }

        for icon in LEGACY_ICONS {
            if seen.insert(icon.to_string()) {
                icons.push(icon.to_string());
            }
        }

        icons
    }
}

const LEGACY_ICONS: &[&str] = &["🔴", "✅", "❌", "⚠️", "⚡", "💼", "🎉", "❓", "⏳"];

fn default_presets() -> HashMap<String, PresetConfig> {
    [
        ("notification", "⚡"),
        ("posttooluse", "⚡"),
        ("stop", "✅"),
        ("subagent-stop", "🔴"),
        ("waiting", "⏳"),
        ("completed", "✅"),
    ]
    .into_iter()
    .map(|(name, emoji)| {
        (
            name.to_string(),
            PresetConfig {
                emoji: emoji.to_string(),
            },
        )
    })
    .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config_contains_builtin_presets() {
        let config = NotificationConfig::default();
        assert!(config.enabled);
        assert_eq!(config.resolve_emoji(Some("stop")), "✅");
        assert_eq!(config.resolve_emoji(Some("notification")), "⚡");
        assert_eq!(config.resolve_emoji(Some("waiting")), "⏳");
    }

    #[test]
    fn test_from_configuration_empty() {
        let config_map = BTreeMap::new();
        let config = NotificationConfig::from_configuration(&config_map);
        assert!(config.enabled);
        assert_eq!(config.resolve_emoji(Some("stop")), "✅");
    }

    #[test]
    fn test_from_configuration_custom_presets_override_defaults() {
        let mut config_map = BTreeMap::new();
        config_map.insert(
            "presets".to_string(),
            r#"{
                "stop": {"emoji": "🎉"},
                "error": {"emoji": "💥"}
            }"#
                .to_string(),
        );

        let config = NotificationConfig::from_configuration(&config_map);
        assert_eq!(config.resolve_emoji(Some("stop")), "🎉");
        assert_eq!(config.resolve_emoji(Some("error")), "💥");
        assert_eq!(config.resolve_emoji(Some("notification")), "⚡");
    }

    #[test]
    fn test_from_configuration_disabled() {
        let mut config_map = BTreeMap::new();
        config_map.insert("enabled".to_string(), "false".to_string());

        let config = NotificationConfig::from_configuration(&config_map);
        assert!(!config.enabled);
    }

    #[test]
    fn test_unknown_notification_is_treated_as_literal_emoji_or_text() {
        let config = NotificationConfig::default();
        assert_eq!(config.resolve_emoji(Some("✅")), "✅");
        assert_eq!(config.resolve_emoji(Some("missing")), "missing");
    }
}
