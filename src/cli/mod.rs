use std::fs;
use std::path::PathBuf;
use anyhow::Context;

const GOGGLES_MARKER: &str = "claude-goggles/goggles.sock";

const HOOK_COMMAND: &str = "cat | nc -U ~/.claude-goggles/goggles.sock 2>/dev/null || true";

const HOOK_TYPES: &[&str] = &[
    "PreToolUse",
    "PostToolUse",
    "SubagentStart",
    "SubagentStop",
    "Stop",
];

fn home_dir() -> anyhow::Result<PathBuf> {
    dirs::home_dir().context("could not determine home directory")
}

fn settings_path() -> anyhow::Result<PathBuf> {
    Ok(home_dir()?.join(".claude").join("settings.json"))
}

pub(crate) fn socket_dir() -> anyhow::Result<PathBuf> {
    Ok(home_dir()?.join(".claude-goggles"))
}

pub(crate) fn init() -> anyhow::Result<()> {
    // Ensure socket dir exists
    let sock_dir = socket_dir()?;
    fs::create_dir_all(&sock_dir)?;

    // Read or create settings
    let path = settings_path()?;
    let content = if path.exists() {
        fs::read_to_string(&path)?
    } else {
        "{}".to_string()
    };

    let updated = merge_hooks(&content)?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&path, &updated)?;
    println!("Hooks installed into {}", path.display());
    println!("Socket dir: {}", sock_dir.display());
    Ok(())
}

pub(crate) fn clean() -> anyhow::Result<()> {
    let path = settings_path()?;
    if path.exists() {
        let content = fs::read_to_string(&path)?;
        let updated = remove_hooks(&content)?;
        fs::write(&path, &updated)?;
        println!("Hooks removed from {}", path.display());
    }

    let sock = socket_dir()?.join("goggles.sock");
    if sock.exists() {
        fs::remove_file(&sock)?;
        println!("Socket removed");
    }
    Ok(())
}

fn merge_hooks(settings_json: &str) -> anyhow::Result<String> {
    let mut v: serde_json::Value = serde_json::from_str(settings_json)?;

    let hooks = v
        .as_object_mut()
        .context("settings.json root is not an object")?
        .entry("hooks")
        .or_insert_with(|| serde_json::json!({}));

    for hook_type in HOOK_TYPES {
        let arr = hooks
            .as_object_mut()
            .context("hooks is not an object")?
            .entry(*hook_type)
            .or_insert_with(|| serde_json::json!([]));

        let entries = arr.as_array_mut().context("hook type entry is not an array")?;

        // Check if already installed
        let already = entries.iter().any(|e| {
            e.get("hooks")
                .and_then(|h| h.as_array())
                .is_some_and(|arr| {
                    arr.iter().any(|hook| {
                        hook.get("command")
                            .and_then(|c| c.as_str())
                            .is_some_and(|s| s.contains(GOGGLES_MARKER))
                    })
                })
        });

        if !already {
            entries.push(serde_json::json!({
                "matcher": "",
                "hooks": [{ "type": "command", "command": HOOK_COMMAND }]
            }));
        }
    }

    Ok(serde_json::to_string_pretty(&v)?)
}

fn remove_hooks(settings_json: &str) -> anyhow::Result<String> {
    let mut v: serde_json::Value = serde_json::from_str(settings_json)?;

    if let Some(hooks) = v.get_mut("hooks").and_then(|h| h.as_object_mut()) {
        for hook_type in HOOK_TYPES {
            if let Some(arr) = hooks.get_mut(*hook_type).and_then(|a| a.as_array_mut()) {
                arr.retain(|e| {
                    !e.get("hooks")
                        .and_then(|h| h.as_array())
                        .is_some_and(|hooks_arr| {
                            hooks_arr.iter().any(|hook| {
                                hook.get("command")
                                    .and_then(|c| c.as_str())
                                    .is_some_and(|s| s.contains(GOGGLES_MARKER))
                            })
                        })
                });
            }
        }
    }

    Ok(serde_json::to_string_pretty(&v)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_merge_hooks_into_empty_settings() {
        let settings = r#"{}"#;
        let result = merge_hooks(settings).unwrap();
        let v: serde_json::Value = serde_json::from_str(&result).unwrap();
        let hooks = v.get("hooks").unwrap();
        let pre = hooks.get("PreToolUse").unwrap().as_array().unwrap();
        assert_eq!(pre.len(), 1);
        assert_eq!(pre[0]["matcher"].as_str().unwrap(), "");
        assert!(pre[0]["hooks"].is_array());
        assert_eq!(pre[0]["hooks"][0]["type"].as_str().unwrap(), "command");
        assert!(pre[0]["hooks"][0]["command"].as_str().unwrap().contains(GOGGLES_MARKER));
        assert!(hooks.get("SubagentStart").unwrap().as_array().unwrap().len() == 1);
    }

    #[test]
    fn test_merge_hooks_preserves_existing() {
        let settings = r#"{
            "hooks": {
                "PreToolUse": [{ "matcher": "Bash", "hooks": [{ "type": "command", "command": "echo existing" }] }]
            }
        }"#;
        let result = merge_hooks(settings).unwrap();
        let v: serde_json::Value = serde_json::from_str(&result).unwrap();
        let pre = v["hooks"]["PreToolUse"].as_array().unwrap();
        assert_eq!(pre.len(), 2); // existing + goggles
        assert_eq!(pre[0]["hooks"][0]["command"].as_str().unwrap(), "echo existing");
    }

    #[test]
    fn test_merge_hooks_idempotent() {
        let settings = r#"{}"#;
        let first = merge_hooks(settings).unwrap();
        let second = merge_hooks(&first).unwrap();
        let v: serde_json::Value = serde_json::from_str(&second).unwrap();
        // Should not duplicate
        assert_eq!(v["hooks"]["PreToolUse"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn test_remove_hooks() {
        let settings = r#"{
            "hooks": {
                "PreToolUse": [
                    { "matcher": "Bash", "hooks": [{ "type": "command", "command": "echo existing" }] },
                    { "matcher": "", "hooks": [{ "type": "command", "command": "cat | nc -U ~/.claude-goggles/goggles.sock 2>/dev/null || true" }] }
                ]
            }
        }"#;
        let result = remove_hooks(settings).unwrap();
        let v: serde_json::Value = serde_json::from_str(&result).unwrap();
        let pre = v["hooks"]["PreToolUse"].as_array().unwrap();
        assert_eq!(pre.len(), 1);
        assert_eq!(pre[0]["hooks"][0]["command"].as_str().unwrap(), "echo existing");
    }
}
