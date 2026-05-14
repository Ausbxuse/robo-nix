use std::env;
use std::ffi::OsString;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use crate::error::AppError;
use crate::shell_refresh::request_manual_runtime_refresh;
use crate::ui::{status, Config};

pub(crate) fn run(args: Vec<OsString>, config: Config) -> Result<ExitCode, AppError> {
    if !args.is_empty() {
        return Err(AppError::user(
            "refresh does not accept arguments; run `robo refresh`",
        ));
    }

    let workspace = refresh_workspace_root()?;
    clear_robo_state(&workspace)?;
    status(config, "cleared .robo-nix runtime state");

    if env::var_os("ROBO_NIX_ACTIVE").is_some() {
        request_manual_runtime_refresh(&workspace).map_err(|err| {
            AppError::project(format!("failed to request active shell refresh: {err}")).with_hint(
                "the runtime state was cleared; run `robo refresh` again or start a new `robo shell`.",
            )
        })?;
        status(config, "active shell refresh requested");
    } else {
        status(config, "next robo command will rebuild the runtime cache");
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

fn clear_robo_state(workspace: &Path) -> Result<(), AppError> {
    let state_dir = workspace.join(".robo-nix");
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clear_robo_state_removes_runtime_state_directory() {
        let root = temp_project("refresh-clear-state");
        fs::create_dir_all(root.join(".robo-nix/nested")).unwrap();
        fs::write(root.join(".robo-nix/nested/cache"), "cached").unwrap();

        clear_robo_state(&root).unwrap();

        assert!(!root.join(".robo-nix").exists());
        cleanup(root);
    }

    #[test]
    fn clear_robo_state_allows_absent_state_directory() {
        let root = temp_project("refresh-clear-absent");

        clear_robo_state(&root).unwrap();

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
