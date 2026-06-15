use std::env;
use std::ffi::OsString;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use serde_json::Value;

use crate::bootstrap::looks_like_robo_flake;
use crate::error::AppError;
use crate::nix_env::{filter_nix_output_for_user, nix_command};
use crate::profile::RuntimeProfile;
use crate::shell_refresh::request_manual_runtime_refresh;
use crate::ui::{row, section, Config, ProgressTreeSession};

const ROBO_NIX_INPUT: &str = "robo-nix";

pub(crate) fn run(args: Vec<OsString>, config: Config) -> Result<ExitCode, AppError> {
    if !args.is_empty() {
        return Err(AppError::user(
            "update does not accept arguments; run `robo update`",
        ));
    }

    let workspace = update_workspace_root()?;
    validate_robo_flake(&workspace)?;
    let mut progress = ProgressTreeSession::new(
        config,
        "robo update",
        "updating robo-nix flake input",
        vec![],
    );
    update_robo_nix_input(&mut progress, &workspace)?;
    let installable = locked_robo_nix_installable(&workspace).map_err(|err| {
        progress.finish_clear();
        err
    })?;
    progress.start_active_child("installing robo CLI binary");
    reinstall_robo_binary(&mut progress, &workspace, &installable)?;
    progress.start_active_child("clearing runtime cache state");
    clear_runtime_profile_state(&workspace).map_err(|err| {
        progress.finish_clear();
        err
    })?;
    progress.finish_active_child(Some("cleared"));
    progress.finish_success("robo updated");

    section(config, "update");
    row(config, "✓", "updated", "robo-nix flake input");
    row(config, "✓", "installed", "robo CLI binary");
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
                "`robo update` updates the `robo-nix` input and CLI binary for robo-owned project flakes.",
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

fn update_robo_nix_input(
    progress: &mut ProgressTreeSession,
    workspace: &Path,
) -> Result<(), AppError> {
    let mut command = nix_command();
    command
        .current_dir(workspace)
        .arg("flake")
        .arg("update")
        .arg(ROBO_NIX_INPUT);
    let output = progress.run_command(&mut command).map_err(|err| {
        progress.finish_clear();
        AppError::project(format!("failed to start nix: {err}"))
            .with_hint("install Nix with flakes enabled, then rerun `robo update`.")
    })?;

    if output.status.success() {
        progress.finish_active_child(None);
        return Ok(());
    }

    progress.finish_clear();
    crate::write_command_output(&filter_nix_output_for_user(&output))?;
    Err(AppError::project(format!(
        "nix flake update {ROBO_NIX_INPUT} exited with {}",
        output.status
    ))
    .with_hint("review the Nix output above; the existing flake.lock was left as Nix reported."))
}

fn locked_robo_nix_installable(workspace: &Path) -> Result<String, AppError> {
    let lock_path = workspace.join("flake.lock");
    let lock = fs::read_to_string(&lock_path).map_err(|err| {
        AppError::project(format!("failed to read {}: {err}", lock_path.display()))
            .with_hint("rerun `robo update`; Nix should create or update flake.lock first.")
    })?;
    let lock: Value = serde_json::from_str(&lock).map_err(|err| {
        AppError::project(format!("failed to parse {}: {err}", lock_path.display()))
    })?;
    let locked = lock
        .get("nodes")
        .and_then(|nodes| nodes.get(ROBO_NIX_INPUT))
        .and_then(|node| node.get("locked"))
        .ok_or_else(|| {
            AppError::project("flake.lock does not contain a locked `robo-nix` input").with_hint(
                "rerun `robo update`; generated robo project flakes define inputs.robo-nix.",
            )
        })?;

    let source = locked_robo_nix_source(locked)?;
    Ok(format!("{source}#robo"))
}

fn locked_robo_nix_source(locked: &Value) -> Result<String, AppError> {
    match json_str(locked, "type") {
        Some("github") => {
            let owner = required_locked_field(locked, "owner")?;
            let repo = required_locked_field(locked, "repo")?;
            let rev = required_locked_field(locked, "rev")?;
            Ok(format!("github:{owner}/{repo}/{rev}"))
        }
        Some("path") => {
            let path = required_locked_field(locked, "path")?;
            Ok(format!("path:{path}"))
        }
        Some(kind) => Err(AppError::project(format!(
            "cannot reinstall robo from locked robo-nix input type `{kind}`"
        ))
        .with_hint(
            "install robo manually with `nix profile install <robo-nix-flake>#robo`, then rerun `robo update`.",
        )),
        None => Err(AppError::project(
            "locked robo-nix input is missing its source type",
        )),
    }
}

fn required_locked_field<'a>(locked: &'a Value, name: &str) -> Result<&'a str, AppError> {
    json_str(locked, name).ok_or_else(|| {
        AppError::project(format!("locked robo-nix input is missing `{name}`"))
            .with_hint("rerun `robo update`; Nix should write a complete flake.lock entry.")
    })
}

fn json_str<'a>(value: &'a Value, name: &str) -> Option<&'a str> {
    value.get(name).and_then(Value::as_str)
}

fn reinstall_robo_binary(
    progress: &mut ProgressTreeSession,
    workspace: &Path,
    installable: &str,
) -> Result<(), AppError> {
    let mut remove = nix_command();
    let _ = remove
        .current_dir(workspace)
        .arg("profile")
        .arg("remove")
        .arg("robo")
        .output();

    let mut install = nix_command();
    install
        .current_dir(workspace)
        .arg("--accept-flake-config")
        .arg("profile")
        .arg("install")
        .arg(installable);
    let output = progress.run_command(&mut install).map_err(|err| {
        progress.finish_clear();
        AppError::project(format!("failed to start nix: {err}"))
            .with_hint("install Nix with flakes enabled, then rerun `robo update`.")
    })?;

    if output.status.success() {
        progress.finish_active_child(None);
        return Ok(());
    }

    progress.finish_clear();
    crate::write_command_output(&filter_nix_output_for_user(&output))?;
    Err(AppError::project(format!(
        "nix profile install {installable} exited with {}",
        output.status
    ))
    .with_hint(
        "the project lock was updated, but the robo CLI binary was not reinstalled; rerun `robo update` after fixing the Nix error.",
    ))
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

    #[test]
    fn locked_installable_uses_github_revision() {
        let root = temp_project("github-installable");
        fs::write(
            root.join("flake.lock"),
            r#"{
  "nodes": {
    "robo-nix": {
      "locked": {
        "type": "github",
        "owner": "ausbxuse",
        "repo": "robo-nix",
        "rev": "abc123"
      }
    }
  }
}
"#,
        )
        .unwrap();

        assert_eq!(
            locked_robo_nix_installable(&root).unwrap(),
            "github:ausbxuse/robo-nix/abc123#robo"
        );
        cleanup(root);
    }

    #[test]
    fn locked_installable_supports_path_inputs() {
        let root = temp_project("path-installable");
        fs::write(
            root.join("flake.lock"),
            r#"{
  "nodes": {
    "robo-nix": {
      "locked": {
        "type": "path",
        "path": "/workspace/robo-nix"
      }
    }
  }
}
"#,
        )
        .unwrap();

        assert_eq!(
            locked_robo_nix_installable(&root).unwrap(),
            "path:/workspace/robo-nix#robo"
        );
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
