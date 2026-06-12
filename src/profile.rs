use std::env;
use std::ffi::OsString;
use std::path::{Path, PathBuf};

use crate::error::AppError;

const DEFAULT_PROFILE_SELECTOR: &str = "default";
const ACTIVE_PROFILE_SELECTOR_ENV: &str = "ROBO_NIX_PROFILE_SELECTOR";

#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) struct RuntimeProfile {
    requested: Option<String>,
}

impl RuntimeProfile {
    pub(crate) fn default() -> Self {
        Self { requested: None }
    }

    pub(crate) fn named(name: String) -> Result<Self, AppError> {
        validate_profile_name(&name)?;
        Ok(Self {
            requested: Some(name),
        })
    }

    pub(crate) fn from_active_env() -> Self {
        env::var(ACTIVE_PROFILE_SELECTOR_ENV)
            .ok()
            .filter(|name| validate_profile_name(name).is_ok())
            .filter(|name| name != DEFAULT_PROFILE_SELECTOR)
            .map(|requested| Self {
                requested: Some(requested),
            })
            .unwrap_or_else(Self::default)
    }

    pub(crate) fn requested(&self) -> Option<&str> {
        self.requested.as_deref()
    }

    pub(crate) fn selector(&self) -> &str {
        self.requested
            .as_deref()
            .unwrap_or(DEFAULT_PROFILE_SELECTOR)
    }

    pub(crate) fn active_selector_env_name() -> &'static str {
        ACTIVE_PROFILE_SELECTOR_ENV
    }

    pub(crate) fn state_dir(&self, workspace: &Path) -> PathBuf {
        workspace
            .join(".robo-nix")
            .join("profiles")
            .join(self.selector())
    }
}

pub(crate) fn parse_profile_option(
    args: Vec<OsString>,
) -> Result<(RuntimeProfile, Vec<OsString>), AppError> {
    let mut profile = RuntimeProfile::default();
    let mut remaining = Vec::new();
    let mut iter = args.into_iter();
    while let Some(arg) = iter.next() {
        if arg == "--profile" || arg == "-p" {
            let Some(name) = iter.next() else {
                return Err(AppError::user("missing value for --profile"));
            };
            let name = name
                .to_str()
                .ok_or_else(|| AppError::user("runtime profile must be valid UTF-8"))?;
            profile = RuntimeProfile::named(name.to_string())?;
        } else if let Some(value) = arg
            .to_str()
            .and_then(|text| text.strip_prefix("--profile="))
        {
            profile = RuntimeProfile::named(value.to_string())?;
        } else {
            remaining.push(arg);
            remaining.extend(iter);
            break;
        }
    }
    Ok((profile, remaining))
}

fn validate_profile_name(name: &str) -> Result<(), AppError> {
    if is_valid_profile_name(name) {
        Ok(())
    } else {
        Err(AppError::user(format!("invalid runtime profile `{name}`")).with_hint(
            "profile names must start with a letter or digit and contain only letters, digits, '.', '_', or '-'.",
        ))
    }
}

fn is_valid_profile_name(name: &str) -> bool {
    let mut chars = name.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    first.is_ascii_alphanumeric()
        && chars.all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '.' | '_' | '-'))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_profile_option() {
        let (profile, args) = parse_profile_option(vec![
            OsString::from("--profile"),
            OsString::from("driver"),
            OsString::from("--"),
            OsString::from("python"),
        ])
        .unwrap();

        assert_eq!(profile.requested(), Some("driver"));
        assert_eq!(args, vec![OsString::from("--"), OsString::from("python")]);
    }

    #[test]
    fn rejects_unsafe_profile_names() {
        assert!(RuntimeProfile::named("../driver".to_string()).is_err());
        assert!(RuntimeProfile::named(".driver".to_string()).is_err());
        assert!(RuntimeProfile::named("driver".to_string()).is_ok());
    }
}
