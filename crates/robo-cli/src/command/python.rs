use std::fs;
use std::process::ExitCode;

use crate::{error, hint, Config};

pub(super) fn ensure_python_version_files(config: Config) -> Result<(), ExitCode> {
    let Ok(pyproject) = fs::read_to_string("pyproject.toml") else {
        return Ok(());
    };
    let Some(required) = exact_python_requirement(&pyproject) else {
        return Ok(());
    };
    let Ok(project_python) = fs::read_to_string(".python-version") else {
        error(
            config,
            &format!("pyproject.toml requires Python {required}, but .python-version is missing."),
        );
        hint(
            config,
            &format!("write `{required}` to .python-version, then rerun this command."),
        );
        return Err(ExitCode::from(1));
    };
    let project_python = project_python.trim();

    if project_python == required {
        if let Ok(robo_nix) = fs::read_to_string("robo.nix") {
            if let Some(robo_python) = robo_python_version(&robo_nix) {
                if robo_python != required {
                    error(
                        config,
                        &format!("robo.nix declares Python {robo_python}, but pyproject.toml requires Python {required}."),
                    );
                    hint(
                        config,
                        &format!("set `pythonVersion = \"{required}\";` in robo.nix."),
                    );
                    return Err(ExitCode::from(1));
                }
            }
        }
        return Ok(());
    }

    error(
        config,
        &format!(".python-version is {project_python}, but pyproject.toml requires Python {required}."),
    );
    hint(
        config,
        &format!("write `{required}` to .python-version, then rerun this command."),
    );
    hint(
        config,
        "if robo.nix has a different pythonVersion, update that to match too.",
    );
    Err(ExitCode::from(1))
}

fn robo_python_version(text: &str) -> Option<&str> {
    for line in text.lines() {
        let line = line.trim();
        let Some(value) = line.strip_prefix("pythonVersion") else {
            continue;
        };
        return quoted_value(value);
    }
    None
}

pub(crate) fn exact_python_requirement(text: &str) -> Option<&str> {
    for line in text.lines() {
        let line = line.trim();
        let Some(value) = line.strip_prefix("requires-python") else {
            continue;
        };
        let Some(raw) = quoted_value(value) else {
            continue;
        };
        let raw = raw.trim();
        let Some(rest) = raw.strip_prefix("===").or_else(|| raw.strip_prefix("==")) else {
            continue;
        };
        let rest = rest.trim_start();
        let end = rest
            .find(|ch: char| !(ch.is_ascii_digit() || ch == '.' || ch == '*'))
            .unwrap_or(rest.len());
        let token = &rest[..end];
        let version = token.strip_suffix(".*").unwrap_or(token);
        let parts = version.split('.').collect::<Vec<_>>();
        if matches!(parts.len(), 2 | 3)
            && parts
                .iter()
                .all(|part| !part.is_empty() && part.chars().all(|ch| ch.is_ascii_digit()))
        {
            return Some(version);
        }
    }
    None
}

pub(crate) fn quoted_value(text: &str) -> Option<&str> {
    let (_, value) = text.split_once('=')?;
    let value = value.trim();
    let quote = value.chars().next()?;
    if quote != '"' && quote != '\'' {
        return None;
    }
    let body = &value[quote.len_utf8()..];
    let end = body.find(quote)?;
    Some(&body[..end])
}
