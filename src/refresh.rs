use std::env;
use std::ffi::OsString;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use crate::error::AppError;
use crate::profile::{parse_profile_option, RuntimeProfile};
use crate::shell_refresh::request_manual_runtime_refresh;
use crate::ui::{row, section, Config};

pub(crate) fn run(args: Vec<OsString>, config: Config) -> Result<ExitCode, AppError> {
    let (mut profile, args) = parse_profile_option(args)?;
    if !args.is_empty() {
        return Err(AppError::user(
            "refresh does not accept arguments; run `robo refresh [--profile <name>]`",
        ));
    }

    let workspace = refresh_workspace_root()?;
    if env::var_os("ROBO_NIX_ACTIVE").is_some() && profile.requested().is_none() {
        profile = RuntimeProfile::from_active_env();
    }
    clear_robo_state(&workspace, &profile)?;
    section(config, "refresh");
    row(
        config,
        "✓",
        "cleared",
        &format!("{} runtime state", profile_status_name(&profile)),
    );

    if env::var_os("ROBO_NIX_ACTIVE").is_some() {
        request_manual_runtime_refresh(&workspace, &profile).map_err(|err| {
            AppError::project(format!("failed to request active shell refresh: {err}")).with_hint(
                "the runtime state was cleared; run `robo refresh` again or start a new `robo shell`.",
            )
        })?;
        row(config, "✓", "requested", "active shell refresh");
    } else {
        row(
            config,
            "✓",
            "next",
            "robo command will rebuild runtime cache",
        );
    }

    Ok(ExitCode::SUCCESS)
}

fn refresh_workspace_root() -> Result<PathBuf, AppError> {
    if env::var_os("ROBO_NIX_ACTIVE").is_some() {
        if let Some(workspace) = env::var_os("WORKSPACE_ROOT") {
            return Ok(PathBuf::from(workspace));
        }
    }
    env::current_dir()
        .map_err(|err| AppError::project(format!("failed to determine workspace root: {err}")))
}

fn clear_robo_state(workspace: &Path, profile: &RuntimeProfile) -> Result<(), AppError> {
    let state_dir = profile.state_dir(workspace);
    let metadata = match fs::symlink_metadata(&state_dir) {
        Ok(metadata) => metadata,
        Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(err) => {
            return Err(AppError::project(format!(
                "failed to inspect {}: {err}",
                state_dir.display()
            )));
        }
    };

    let result = if metadata.file_type().is_dir() {
        fs::remove_dir_all(&state_dir)
    } else {
        fs::remove_file(&state_dir)
    };

    result.map_err(|err| {
        AppError::project(format!("failed to clear {}: {err}", state_dir.display()))
            .with_hint("make sure no other robo process is preparing this project.")
    })
}

fn profile_status_name(profile: &RuntimeProfile) -> String {
    match profile.requested() {
        Some(name) => format!("profile `{name}`"),
        None => "default profile".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clear_robo_state_removes_runtime_state_directory() {
        let root = temp_project("refresh-clear-state");
        let state_dir = RuntimeProfile::default().state_dir(&root);
        fs::create_dir_all(state_dir.join("nested")).unwrap();
        fs::write(state_dir.join("nested/cache"), "cached").unwrap();

        clear_robo_state(&root, &RuntimeProfile::default()).unwrap();

        assert!(!RuntimeProfile::default().state_dir(&root).exists());
        cleanup(root);
    }

    #[test]
    fn clear_robo_state_allows_absent_state_directory() {
        let root = temp_project("refresh-clear-absent");

        clear_robo_state(&root, &RuntimeProfile::default()).unwrap();

        cleanup(root);
    }

    fn temp_project(label: &str) -> PathBuf {
        let root = env::temp_dir().join(format!("robo-{label}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        root
    }

    fn cleanup(root: PathBuf) {
        let _ = fs::remove_dir_all(root);
    }
}
