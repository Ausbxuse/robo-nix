use std::env;
use std::ffi::OsString;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use crate::bootstrap::looks_like_robo_flake;
use crate::error::AppError;
use crate::nix_env::{filter_nix_output_for_user, nix_command};
use crate::profile::RuntimeProfile;
use crate::shell_refresh::request_manual_runtime_refresh;
use crate::ui::{output_with_spinner, row, section, Config};

const ROBO_NIX_INPUT: &str = "robo-nix";

pub(crate) fn run(args: Vec<OsString>, config: Config) -> Result<ExitCode, AppError> {
    if !args.is_empty() {
        return Err(AppError::user(
            "update does not accept arguments; run `robo update`",
        ));
    }

    let workspace = update_workspace_root()?;
    validate_robo_flake(&workspace)?;
    update_robo_nix_input(config, &workspace)?;
    clear_runtime_profile_state(&workspace)?;
    section(config, "update");
    row(config, "✓", "updated", "robo-nix flake input");
    row(config, "✓", "cleared", "runtime cache state");

    if env::var_os("ROBO_NIX_ACTIVE").is_some() {
        let profile = RuntimeProfile::from_active_env();
        request_manual_runtime_refresh(&workspace, &profile).map_err(|err| {
            AppError::project(format!("failed to request active shell refresh: {err}"))
                .with_hint("the lock was updated; run `robo refresh` or start a new `robo shell`.")
        })?;
        row(config, "✓", "requested", "active shell refresh");
    } else {
        row(config, "✓", "next", "robo command will use updated lock");
    }

    Ok(ExitCode::SUCCESS)
}

fn update_workspace_root() -> Result<PathBuf, AppError> {
    if env::var_os("ROBO_NIX_ACTIVE").is_some() {
        if let Some(workspace) = env::var_os("WORKSPACE_ROOT") {
            return Ok(PathBuf::from(workspace));
        }
    }
    env::current_dir()
        .map_err(|err| AppError::project(format!("failed to determine workspace root: {err}")))
}

fn validate_robo_flake(workspace: &Path) -> Result<(), AppError> {
    let flake_path = workspace.join("flake.nix");
    let flake = fs::read_to_string(&flake_path).map_err(|err| {
        if err.kind() == io::ErrorKind::NotFound {
            AppError::project("missing flake.nix").with_hint(
                "run `robo shell` once to create robo runtime files before `robo update`.",
            )
        } else {
            AppError::project(format!("failed to read {}: {err}", flake_path.display()))
        }
    })?;

    if !looks_like_robo_flake(&flake) {
        return Err(
            AppError::project("this repository does not use a robo-nix flake").with_hint(
                "`robo update` only updates the `robo-nix` input in robo-owned project flakes.",
            ),
        );
    }

    if !flake.contains(ROBO_NIX_INPUT) {
        return Err(
            AppError::project("flake.nix does not define a `robo-nix` input").with_hint(
                "review flake.nix; generated robo project flakes define inputs.robo-nix.",
            ),
        );
    }

    Ok(())
}

fn update_robo_nix_input(config: Config, workspace: &Path) -> Result<(), AppError> {
    let mut command = nix_command();
    command
        .current_dir(workspace)
        .arg("flake")
        .arg("update")
        .arg(ROBO_NIX_INPUT);
    let output = output_with_spinner(config, &mut command, "updating robo-nix flake input")
        .map_err(|err| {
            AppError::project(format!("failed to start nix: {err}"))
                .with_hint("install Nix with flakes enabled, then rerun `robo update`.")
        })?;

    if output.status.success() {
        return Ok(());
    }

    crate::write_command_output(&filter_nix_output_for_user(&output))?;
    Err(AppError::project(format!(
        "nix flake update {ROBO_NIX_INPUT} exited with {}",
        output.status
    ))
    .with_hint("review the Nix output above; the existing flake.lock was left as Nix reported."))
}

fn clear_runtime_profile_state(workspace: &Path) -> Result<(), AppError> {
    let profiles_dir = workspace.join(".robo-nix").join("profiles");
    remove_path_if_exists(&profiles_dir).map_err(|err| {
        AppError::project(format!("failed to clear {}: {err}", profiles_dir.display()))
            .with_hint("make sure no other robo process is preparing this project.")
    })
}

fn remove_path_if_exists(path: &Path) -> io::Result<()> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(err) => return Err(err),
    };

    if metadata.file_type().is_dir() {
        fs::remove_dir_all(path)
    } else {
        fs::remove_file(path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_robo_flake_rejects_non_robo_flake() {
        let root = temp_project("non-robo-flake");
        fs::write(root.join("flake.nix"), "{ outputs = _: {}; }\n").unwrap();

        let error = validate_robo_flake(&root).unwrap_err();

        assert!(error.message().contains("does not use a robo-nix flake"));
        cleanup(root);
    }

    #[test]
    fn clear_runtime_profile_state_removes_profiles_only() {
        let root = temp_project("clear-profiles");
        let profile_cache = root.join(".robo-nix").join("profiles").join("default");
        let venv = root.join(".robo-nix").join("venvs").join("default");
        fs::create_dir_all(&profile_cache).unwrap();
        fs::create_dir_all(&venv).unwrap();
        fs::write(profile_cache.join("runtime-env-cache-v1.env0"), "cache").unwrap();
        fs::write(venv.join("pyvenv.cfg"), "venv").unwrap();

        clear_runtime_profile_state(&root).unwrap();

        assert!(!root.join(".robo-nix").join("profiles").exists());
        assert!(venv.exists());
        cleanup(root);
    }

    fn temp_project(label: &str) -> PathBuf {
        let root = env::temp_dir().join(format!("robo-update-{label}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        root
    }

    fn cleanup(root: PathBuf) {
        let _ = fs::remove_dir_all(root);
    }
}
