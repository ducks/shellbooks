use anyhow::{Context, Result};
use serde::Deserialize;
use std::path::PathBuf;

#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    pub library_paths: Vec<PathBuf>,
    #[serde(default = "default_speed")]
    pub default_speed: f32,
    #[serde(default)]
    pub sleep_timer_minutes: u32,
    #[serde(default = "default_theme")]
    pub theme: String,
    #[serde(default = "default_finish_threshold")]
    pub finish_threshold_seconds: u64,
}

fn default_speed() -> f32 { 1.0 }
fn default_theme() -> String { "gruvbox".into() }
fn default_finish_threshold() -> u64 { 30 }

impl Default for Config {
    fn default() -> Self {
        Self {
            library_paths: vec![],
            default_speed: 1.0,
            sleep_timer_minutes: 0,
            theme: "gruvbox".into(),
            finish_threshold_seconds: 30,
        }
    }
}

/// Load config from ~/.config/shellbooks/config.toml. Missing file is OK —
/// returns Config::default() so the app can show an empty-state message
/// instead of refusing to launch.
pub fn load() -> Result<Config> {
    let path = config_path().context("could not resolve config dir")?;
    if !path.exists() {
        return Ok(Config::default());
    }
    let body = std::fs::read_to_string(&path)
        .with_context(|| format!("reading {}", path.display()))?;
    let cfg: Config = toml::from_str(&body)
        .with_context(|| format!("parsing {}", path.display()))?;
    Ok(cfg)
}

fn config_path() -> Option<PathBuf> {
    Some(dirs::config_dir()?.join("shellbooks").join("config.toml"))
}

/// Where library.json lives. ~/.local/share/shellbooks/library.json on Linux.
pub fn library_db_path() -> Option<PathBuf> {
    Some(dirs::data_dir()?.join("shellbooks").join("library.json"))
}

/// Expand a leading `~` against $HOME. Used so users can write `~/Audiobooks`
/// in their config without the rest of the app seeing a literal tilde.
pub fn expand_tilde(p: &std::path::Path) -> PathBuf {
    let s = p.to_string_lossy();
    if let Some(rest) = s.strip_prefix("~/") {
        if let Some(home) = dirs::home_dir() {
            return home.join(rest);
        }
    } else if s == "~" {
        if let Some(home) = dirs::home_dir() {
            return home;
        }
    }
    p.to_path_buf()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn expand_tilde_leaves_non_tilde_paths_alone() {
        assert_eq!(expand_tilde(Path::new("/etc/hosts")), PathBuf::from("/etc/hosts"));
        assert_eq!(expand_tilde(Path::new("relative/path")), PathBuf::from("relative/path"));
        // A tilde mid-path is not expanded; only a leading `~` or `~/`.
        assert_eq!(
            expand_tilde(Path::new("/foo/~/bar")),
            PathBuf::from("/foo/~/bar")
        );
    }

    #[test]
    fn expand_tilde_replaces_leading_tilde_slash() {
        let Some(home) = dirs::home_dir() else { return };
        assert_eq!(expand_tilde(Path::new("~/Audiobooks")), home.join("Audiobooks"));
    }

    #[test]
    fn expand_tilde_replaces_bare_tilde() {
        let Some(home) = dirs::home_dir() else { return };
        assert_eq!(expand_tilde(Path::new("~")), home);
    }

    #[test]
    fn config_defaults_are_usable() {
        let cfg = Config::default();
        assert!(cfg.library_paths.is_empty());
        assert_eq!(cfg.default_speed, 1.0);
        assert_eq!(cfg.theme, "gruvbox");
        assert_eq!(cfg.finish_threshold_seconds, 30);
    }
}
