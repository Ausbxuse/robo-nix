use std::env;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::bootstrap::read_python_version;
use crate::ui::{error, hint, Config};

#[derive(Debug)]
pub(crate) struct AppError {
    message: String,
    hint: Option<String>,
    write_debug_log: bool,
}

impl AppError {
    pub(crate) fn user(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            hint: None,
            write_debug_log: false,
        }
    }

    pub(crate) fn project(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            hint: None,
            write_debug_log: true,
        }
    }

    pub(crate) fn with_hint(mut self, hint: impl Into<String>) -> Self {
        self.hint = Some(hint.into());
        self
    }

    pub(crate) fn should_write_debug_log(&self) -> bool {
        self.write_debug_log
    }

    #[cfg(test)]
    pub(crate) fn message(&self) -> &str {
        &self.message
    }
}

pub(crate) fn print_error(config: Config, error_value: &AppError) {
    error(config, &error_value.message);
    if let Some(message) = &error_value.hint {
        hint(config, message);
    }
}

pub(crate) fn write_debug_log(error: &AppError) -> io::Result<PathBuf> {
    // DEBUG: keep this pasteable; it is for issue reports, not user policy fixes.
    let dir = PathBuf::from(".robo-nix");
    fs::create_dir_all(&dir)?;
    let path = dir.join("last-error.log");
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs().to_string())
        .unwrap_or_else(|_| "before-unix-epoch".to_string());

    let mut text = String::new();
    text.push_str("robo-nix debug log\n");
    text.push_str(&format!("timestamp_unix = {timestamp}\n"));
    text.push_str(&format!("cwd = {}\n", env::current_dir()?.display()));
    text.push_str(&format!(
        "argv = {}\n",
        env::args().collect::<Vec<_>>().join(" ")
    ));
    text.push_str(&format!("error = {}\n", error.message));
    if let Some(hint) = &error.hint {
        text.push_str(&format!("hint = {hint}\n"));
    }
    text.push_str("\nproject files:\n");
    for file in [
        ".python-version",
        "pyproject.toml",
        "flake.nix",
        "flake.lock",
        "robo.nix",
    ] {
        let path = Path::new(file);
        text.push_str(&format!(
            "- {file}: {}\n",
            if path.exists() { "present" } else { "missing" }
        ));
    }
    if let Ok(version) = read_python_version(Path::new(".")) {
        text.push_str(&format!("\npython_version = {version}\n"));
    }

    fs::write(&path, text)?;
    Ok(path)
}
