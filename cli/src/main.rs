use anyhow::{Context, Result, bail};
use clap::{Parser, Subcommand};
use serde_json::{json, Value};
use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

// Embed WASM binary at compile time
const WASM_BYTES: &[u8] = include_bytes!("../../target/wasm32-wasip1/release/zellij_notify.wasm");

// Default notification presets (name -> emoji)
const NOTIFY_CONFIG: &[(&str, &str)] = &[
    ("notification", "⚡"),
    ("posttooluse", "⚡"),
    ("stop", "✅"),
    ("subagent-stop", "🔴"),
    ("waiting", "⏳"),
    ("completed", "✅"),
];

const ZELLIJ_CONFIG_TEMPLATE: &str = r##"plugin location="file:~/.config/zellij/plugins/zellij-notify.wasm" {
    debug "false"
    presets r#"{
        "notification": {"emoji": "⚡"},
        "posttooluse": {"emoji": "⚡"},
        "stop": {"emoji": "✅"},
        "subagent-stop": {"emoji": "🔴"}
    }"#
}
"##;

#[derive(Parser)]
#[command(name = "znotify")]
#[command(about = "Zellij notification plugin CLI", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Claude Code integration commands
    Claude {
        #[command(subcommand)]
        command: ClaudeCommands,
    },
    /// Send notification to Zellij
    Notify {
        /// Notification preset key or literal emoji (eg. stop, notification, ✅, ⚡)
        name: String,
    },
    /// Install plugin to Zellij
    InstallPlugin,
    /// Show installation status
    Status,
    /// Print Zellij config template
    Config,
}

#[derive(Subcommand)]
enum ClaudeCommands {
    /// Install Claude Code hooks
    InstallHooks,
    /// Uninstall Claude Code hooks
    UninstallHooks,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Claude { command } => match command {
            ClaudeCommands::InstallHooks => claude_install_hooks(),
            ClaudeCommands::UninstallHooks => claude_uninstall_hooks(),
        },
        Commands::Notify { name } => notify(&name),
        Commands::InstallPlugin => install_plugin(),
        Commands::Status => status(),
        Commands::Config => config(),
    }
}

fn claude_install_hooks() -> Result<()> {
    let claude_settings = get_claude_settings_path()?;

    // Read existing settings or create new
    let mut settings: Value = if claude_settings.exists() {
        let content = fs::read_to_string(&claude_settings)
            .context("Failed to read Claude settings")?;
        serde_json::from_str(&content)
            .context("Failed to parse Claude settings JSON")?
    } else {
        json!({})
    };

    // Ensure hooks object exists
    if !settings.get("hooks").is_some() {
        settings["hooks"] = json!({});
    }

    let hooks = settings["hooks"].as_object_mut()
        .context("hooks is not an object")?;

    // Add our hooks
    hooks.insert(
        "Notification".to_string(),
        json!([{
            "matcher": "",
            "hooks": [{
                "type": "command",
                "command": "znotify notify notification"
            }]
        }])
    );

    hooks.insert(
        "Stop".to_string(),
        json!([{
            "matcher": "",
            "hooks": [{
                "type": "command",
                "command": "znotify notify stop"
            }]
        }])
    );

    hooks.insert(
        "PostToolUse".to_string(),
        json!([{
            "matcher": "",
            "hooks": [{
                "type": "command",
                "command": "znotify notify posttooluse"
            }]
        }])
    );

    // Write back
    fs::create_dir_all(claude_settings.parent().unwrap())
        .context("Failed to create .claude directory")?;
    fs::write(&claude_settings, serde_json::to_string_pretty(&settings)?)
        .context("Failed to write Claude settings")?;

    println!("✅ Claude hooks installed to {}", claude_settings.display());
    println!("   Added: Notification, Stop, PostToolUse");
    Ok(())
}

fn claude_uninstall_hooks() -> Result<()> {
    let claude_settings = get_claude_settings_path()?;

    if !claude_settings.exists() {
        println!("No Claude settings file found");
        return Ok(());
    }

    let content = fs::read_to_string(&claude_settings)
        .context("Failed to read Claude settings")?;
    let mut settings: Value = serde_json::from_str(&content)
        .context("Failed to parse Claude settings JSON")?;

    if let Some(hooks) = settings.get_mut("hooks").and_then(|h| h.as_object_mut()) {
        hooks.remove("Notification");
        hooks.remove("Stop");
        hooks.remove("PostToolUse");

        fs::write(&claude_settings, serde_json::to_string_pretty(&settings)?)
            .context("Failed to write Claude settings")?;

        println!("✅ Claude hooks removed from {}", claude_settings.display());
    } else {
        println!("No hooks found in Claude settings");
    }

    Ok(())
}

fn notify(name: &str) -> Result<()> {
    let name = name.trim();
    if name.is_empty() {
        bail!("Notification name cannot be empty");
    }

    let pipe_value = NOTIFY_CONFIG
        .iter()
        .find(|(preset_name, _)| *preset_name == name)
        .map(|(_, emoji)| *emoji)
        .unwrap_or(name);

    let pane_id = env::var("ZELLIJ_PANE_ID")
        .context("ZELLIJ_PANE_ID not found. Are you running inside Zellij?")?;

    let output = Command::new("zellij")
        .arg("pipe")
        .arg("--name")
        .arg(format!("notify::{}::{}", pipe_value, pane_id))
        .arg("--")
        .arg("")
        .output()
        .context("Failed to execute zellij pipe command")?;

    if !output.status.success() {
        bail!("zellij pipe failed: {}", String::from_utf8_lossy(&output.stderr));
    }

    Ok(())
}

fn install_plugin() -> Result<()> {
    let plugin_dir = get_plugin_path()?.parent().unwrap().to_path_buf();
    let plugin_path = get_plugin_path()?;

    fs::create_dir_all(&plugin_dir)
        .context("Failed to create plugin directory")?;

    fs::write(&plugin_path, WASM_BYTES)
        .context("Failed to write plugin file")?;

    println!("✅ Plugin installed to {}", plugin_path.display());

    // Try to reload plugin if in Zellij
    if env::var("ZELLIJ").is_ok() {
        let reload_result = Command::new("zellij")
            .arg("action")
            .arg("start-or-reload-plugin")
            .arg(format!("file:{}", plugin_path.display()))
            .output();

        match reload_result {
            Ok(output) if output.status.success() => {
                println!("✅ Plugin reloaded in Zellij");
            }
            _ => {
                println!("⚠️  Could not reload plugin automatically. Restart Zellij or run:");
                println!("   zellij action start-or-reload-plugin file:{}", plugin_path.display());
            }
        }
    }

    Ok(())
}

fn status() -> Result<()> {
    println!("znotify status\n");

    // Check plugin installation
    let plugin_path = get_plugin_path()?;
    let plugin_installed = plugin_path.exists();
    println!("Plugin: {}", if plugin_installed {
        format!("✅ Installed at {}", plugin_path.display())
    } else {
        format!("❌ Not installed (run: znotify install-plugin)")
    });

    // Check Claude hooks
    let claude_settings = get_claude_settings_path()?;
    let hooks_installed = if claude_settings.exists() {
        let content = fs::read_to_string(&claude_settings).ok();
        content.and_then(|c| serde_json::from_str::<Value>(&c).ok())
            .and_then(|s| s.get("hooks").cloned())
            .and_then(|h| {
                let has_notification = h.get("Notification").is_some();
                let has_stop = h.get("Stop").is_some();
                let has_posttooluse = h.get("PostToolUse").is_some();
                Some(has_notification || has_stop || has_posttooluse)
            })
            .unwrap_or(false)
    } else {
        false
    };

    println!("Claude hooks: {}", if hooks_installed {
        format!("✅ Installed at {}", claude_settings.display())
    } else {
        format!("❌ Not installed (run: znotify claude install-hooks)")
    });

    // Check if in Zellij session
    let in_zellij = env::var("ZELLIJ").is_ok();
    println!("Zellij session: {}", if in_zellij {
        "✅ Running in Zellij"
    } else {
        "❌ Not in Zellij session"
    });

    // Show available notifications
    println!("\nAvailable notifications:");
    for (name, emoji) in NOTIFY_CONFIG {
        println!("  {} {}", emoji, name);
    }

    Ok(())
}

fn config() -> Result<()> {
    println!("Add this to your Zellij config (~/.config/zellij/config.kdl):\n");
    println!("{}", ZELLIJ_CONFIG_TEMPLATE);
    Ok(())
}

fn get_claude_settings_path() -> Result<PathBuf> {
    let home = env::var("HOME")
        .context("HOME environment variable not set")?;
    Ok(PathBuf::from(home).join(".claude").join("settings.json"))
}

fn get_plugin_path() -> Result<PathBuf> {
    let home = env::var("HOME")
        .context("HOME environment variable not set")?;
    Ok(PathBuf::from(home)
        .join(".config")
        .join("zellij")
        .join("plugins")
        .join("zellij-notify.wasm"))
}
