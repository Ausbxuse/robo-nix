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
use crate::project_lock::with_project_lock;
use crate::shell_refresh::request_manual_runtime_refresh;
use crate::ui::{attention, detail, row, section, Config, ProgressTreeSession};

const ROBO_NIX_INPUT: &str = "robo-nix";
const OFFICIAL_ROBO_NIX_OWNER: &str = "ausbxuse";
const AUTOMATIC_UPDATE_MARKER: &str = "tooling-update-attempt-v1";
const BUILD_REVISION: Option<&str> = option_env!("ROBO_NIX_BUILD_REVISION");
const BUILD_LAST_MODIFIED: Option<&str> = option_env!("ROBO_NIX_BUILD_LAST_MODIFIED");

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum UpdateWorkspaceKind {
    Project,
    SourceCheckout,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct BuildProvenance {
    revision: String,
    last_modified: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct LockedGithubInput {
    owner: String,
    repo: String,
    revision: String,
    last_modified: u64,
}

#[derive(Debug, Default, Eq, PartialEq)]
pub(crate) struct RuntimeLockUpdateReport {
    pub(crate) decision: Option<String>,
    pub(crate) warning: Option<String>,
}

pub(crate) fn run(args: Vec<OsString>, config: Config) -> Result<ExitCode, AppError> {
    if !args.is_empty() {
        return Err(AppError::user(
            "update does not accept arguments; run `robo update`",
        ));
    }

    let workspace = update_workspace_root()?;
    let workspace_kind = classify_update_workspace(&workspace)?;
    match workspace_kind {
        UpdateWorkspaceKind::Project => update_project(&workspace, config)?,
        UpdateWorkspaceKind::SourceCheckout => update_source_checkout(&workspace, config)?,
    }

    Ok(ExitCode::SUCCESS)
}

fn update_project(workspace: &Path, config: Config) -> Result<(), AppError> {
    let mut progress = ProgressTreeSession::new(
        config,
        "robo update",
        "updating robo-nix flake input",
        vec![],
    );
    let result = with_project_lock(workspace, "tooling-update", || {
        update_robo_nix_input(&mut progress, workspace)?;
        let installable = locked_robo_nix_installable(workspace)?;
        progress.start_active_child("installing robo CLI binary");
        reinstall_robo_binary(&mut progress, workspace, &installable)?;
        progress.start_active_child("clearing runtime cache state");
        clear_runtime_profile_state(workspace)?;
        progress.finish_active_child(Some("cleared"));
        Ok(())
    });
    if let Err(err) = result {
        progress.finish_clear();
        return Err(err);
    }
    progress.finish_success("robo updated");

    section(config, "update");
    row(config, "✓", "updated", "robo-nix flake input");
    row(config, "✓", "installed", "robo CLI binary");
    row(config, "✓", "cleared", "runtime cache state");

    if env::var_os("ROBO_NIX_ACTIVE").is_some() {
        let profile = RuntimeProfile::from_active_env();
        request_manual_runtime_refresh(workspace, &profile).map_err(|err| {
            AppError::project(format!("failed to request active shell refresh: {err}"))
                .with_hint("the lock was updated; run `robo refresh` or start a new `robo shell`.")
        })?;
        row(config, "✓", "requested", "active shell refresh");
    } else {
        row(config, "✓", "next", "robo command will use updated lock");
    }

    Ok(())
}

fn update_source_checkout(workspace: &Path, config: Config) -> Result<(), AppError> {
    let mut progress = ProgressTreeSession::new(
        config,
        "robo update",
        "installing robo CLI binary from local checkout",
        vec![],
    );
    reinstall_robo_binary(&mut progress, workspace, ".#robo")?;
    progress.finish_success("robo updated");

    section(config, "update");
    row(config, "✓", "installed", "robo CLI binary from .#robo");
    row(
        config,
        "✓",
        "kept",
        "source flake inputs and checkout unchanged",
    );
    Ok(())
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

fn classify_update_workspace(workspace: &Path) -> Result<UpdateWorkspaceKind, AppError> {
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

    if is_robo_nix_source_checkout(workspace, &flake) {
        return Ok(UpdateWorkspaceKind::SourceCheckout);
    }

    if !looks_like_robo_flake(&flake) {
        return Err(
            AppError::project("this repository does not use a robo-nix flake").with_hint(
                "run `robo update` from a robo project or the robo-nix source checkout.",
            ),
        );
    }

    if flake.contains(ROBO_NIX_INPUT) || workspace_lock_has_robo_nix_input(workspace) {
        return Ok(UpdateWorkspaceKind::Project);
    }

    Err(
        AppError::project("flake.nix does not define a `robo-nix` input")
            .with_hint("review flake.nix; generated robo project flakes define inputs.robo-nix."),
    )
}

fn is_robo_nix_source_checkout(workspace: &Path, flake: &str) -> bool {
    flake.contains("description = \"robo-nix\"")
        && workspace.join("Cargo.toml").is_file()
        && workspace.join("src/nix/project-flake.nix").is_file()
        && workspace.join("src/templates/project/flake.nix").is_file()
}

fn workspace_lock_has_robo_nix_input(workspace: &Path) -> bool {
    let Ok(lock) = fs::read_to_string(workspace.join("flake.lock")) else {
        return false;
    };
    let Ok(lock) = serde_json::from_str::<Value>(&lock) else {
        return false;
    };
    locked_robo_nix_node(&lock).is_some()
}

fn running_build_provenance() -> Option<BuildProvenance> {
    BuildProvenance::from_fields(BUILD_REVISION?, BUILD_LAST_MODIFIED?)
}

impl BuildProvenance {
    fn from_fields(revision: &str, last_modified: &str) -> Option<Self> {
        let revision = revision.trim();
        if revision.len() != 40 || !revision.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return None;
        }
        let last_modified = last_modified.trim().parse().ok()?;
        if last_modified == 0 {
            return None;
        }
        Some(Self {
            revision: revision.to_ascii_lowercase(),
            last_modified,
        })
    }
}

impl LockedGithubInput {
    fn from_locked(locked: &Value) -> Option<Self> {
        if json_str(locked, "type") != Some("github") {
            return None;
        }
        Some(Self {
            owner: json_str(locked, "owner")?.to_string(),
            repo: json_str(locked, "repo")?.to_string(),
            revision: json_str(locked, "rev")?.to_ascii_lowercase(),
            last_modified: locked.get("lastModified")?.as_u64()?,
        })
    }

    fn is_older_than(&self, build: &BuildProvenance) -> bool {
        self.owner == OFFICIAL_ROBO_NIX_OWNER
            && self.repo == ROBO_NIX_INPUT
            && self.revision != build.revision
            && self.last_modified < build.last_modified
    }
}

pub(crate) fn reconcile_project_lock_for_running_cli(
    workspace: &Path,
    phase: &str,
    config: Config,
) -> RuntimeLockUpdateReport {
    let Some(build) = running_build_provenance() else {
        return RuntimeLockUpdateReport::default();
    };
    if automatic_update_candidate(workspace, &build).is_none() {
        return RuntimeLockUpdateReport::default();
    }

    match with_project_lock(workspace, "tooling-update", || {
        Ok(reconcile_project_lock_locked(
            workspace, phase, config, &build,
        ))
    }) {
        Ok(report) => report,
        Err(err) => automatic_update_deferred(
            config,
            format!(
                "could not acquire the tooling update lock: {}",
                err.message()
            ),
        ),
    }
}

fn reconcile_project_lock_locked(
    workspace: &Path,
    phase: &str,
    config: Config,
    build: &BuildProvenance,
) -> RuntimeLockUpdateReport {
    let Some(locked) = automatic_update_candidate(workspace, build) else {
        return RuntimeLockUpdateReport::default();
    };
    let attempt = automatic_update_attempt(build, &locked);
    if fs::read_to_string(automatic_update_marker_path(workspace))
        .is_ok_and(|recorded| recorded == attempt)
    {
        return RuntimeLockUpdateReport {
            decision: Some("robo_nix_lock=automatic-update-already-attempted".to_string()),
            warning: None,
        };
    }

    if let Err(err) = write_automatic_update_marker(workspace, &attempt) {
        return automatic_update_deferred(
            config,
            format!("could not record the one-time update attempt: {err}"),
        );
    }

    let root = format!("robo {phase}");
    let mut progress = ProgressTreeSession::new(
        config,
        &root,
        "updating project robo-nix lock for this CLI",
        vec![],
    );
    let mut command = nix_command();
    command
        .current_dir(workspace)
        .arg("flake")
        .arg("update")
        .arg(ROBO_NIX_INPUT);
    let output = match progress.run_command(&mut command) {
        Ok(output) => output,
        Err(err) => {
            progress.finish_clear();
            return automatic_update_deferred(
                config,
                format!("failed to start nix for the automatic lock update: {err}"),
            );
        }
    };

    if !output.status.success() {
        progress.finish_clear();
        return automatic_update_deferred(config, automatic_nix_failure(&output));
    }

    let current = locked_github_input(workspace);
    if let Some(current) = &current {
        let _ = write_automatic_update_marker(workspace, &automatic_update_attempt(build, current));
    }
    if let Some(current) = current.filter(|current| current.revision != locked.revision) {
        progress.finish_active_child(None);
        progress.finish_success("project lock updated");
        return RuntimeLockUpdateReport {
            decision: Some(format!(
                "robo_nix_lock=automatic-update from={} to={}",
                short_revision(&locked.revision),
                short_revision(&current.revision)
            )),
            warning: None,
        };
    }

    progress.finish_active_child(Some("unchanged"));
    progress.finish_success("project lock checked");
    automatic_update_deferred(
        config,
        "the robo-nix source declared by flake.nix remained at its existing revision".to_string(),
    )
}

fn automatic_update_candidate(
    workspace: &Path,
    build: &BuildProvenance,
) -> Option<LockedGithubInput> {
    locked_github_input(workspace).filter(|locked| locked.is_older_than(build))
}

fn locked_github_input(workspace: &Path) -> Option<LockedGithubInput> {
    let lock = fs::read_to_string(workspace.join("flake.lock")).ok()?;
    let lock = serde_json::from_str::<Value>(&lock).ok()?;
    let locked = locked_robo_nix_node(&lock)?.get("locked")?;
    LockedGithubInput::from_locked(locked)
}

fn automatic_update_attempt(build: &BuildProvenance, locked: &LockedGithubInput) -> String {
    format!(
        "robo-nix-tooling-update-attempt-v1\nbuild-revision={}\nlocked-source={}/{}\nlocked-revision={}\n",
        build.revision, locked.owner, locked.repo, locked.revision
    )
}

fn automatic_update_marker_path(workspace: &Path) -> PathBuf {
    workspace.join(".robo-nix").join(AUTOMATIC_UPDATE_MARKER)
}

fn write_automatic_update_marker(workspace: &Path, contents: &str) -> io::Result<()> {
    let state_dir = workspace.join(".robo-nix");
    fs::create_dir_all(&state_dir)?;
    let path = automatic_update_marker_path(workspace);
    let temporary = state_dir.join(format!(
        ".{AUTOMATIC_UPDATE_MARKER}.{}.tmp",
        std::process::id()
    ));
    fs::write(&temporary, contents)?;
    if let Err(err) = fs::rename(&temporary, &path) {
        let _ = fs::remove_file(&temporary);
        return Err(err);
    }
    Ok(())
}

fn automatic_nix_failure(output: &std::process::Output) -> String {
    let filtered = filter_nix_output_for_user(output);
    let stderr = String::from_utf8_lossy(&filtered.stderr);
    let line = stderr
        .lines()
        .rev()
        .find(|line| !line.trim().is_empty())
        .map(str::trim)
        .unwrap_or("Nix did not provide an error message");
    format!(
        "nix flake update {ROBO_NIX_INPUT} exited with {}: {}",
        output.status,
        truncate_detail(line, 300)
    )
}

fn truncate_detail(text: &str, limit: usize) -> String {
    let mut chars = text.chars();
    let prefix = chars.by_ref().take(limit).collect::<String>();
    if chars.next().is_some() {
        format!("{prefix}…")
    } else {
        prefix
    }
}

fn automatic_update_deferred(config: Config, reason: String) -> RuntimeLockUpdateReport {
    section(config, "attention");
    attention(
        config,
        "project robo-nix lock was not updated automatically",
    );
    detail(config, &reason);
    detail(
        config,
        "continuing with the existing lock; run `robo update` to retry explicitly",
    );
    RuntimeLockUpdateReport {
        decision: Some("robo_nix_lock=automatic-update-deferred".to_string()),
        warning: Some(format!("automatic robo-nix lock update deferred: {reason}")),
    }
}

fn short_revision(revision: &str) -> String {
    revision.chars().take(12).collect()
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
    let locked = locked_robo_nix_node(&lock)
        .and_then(|node| node.get("locked"))
        .ok_or_else(|| {
            AppError::project("flake.lock does not contain a locked `robo-nix` input").with_hint(
                "rerun `robo update`; generated robo project flakes define inputs.robo-nix.",
            )
        })?;

    let source = locked_robo_nix_source(locked)?;
    Ok(format!("{source}#robo"))
}

fn locked_robo_nix_node(lock: &Value) -> Option<&Value> {
    let nodes = lock.get("nodes")?;
    let root_name = lock.get("root").and_then(Value::as_str).unwrap_or("root");
    let referenced_name = nodes
        .get(root_name)
        .and_then(|root| root.get("inputs"))
        .and_then(|inputs| inputs.get(ROBO_NIX_INPUT))
        .and_then(Value::as_str);

    match referenced_name {
        Some(name) => nodes.get(name),
        None => nodes.get(ROBO_NIX_INPUT),
    }
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
        .arg("install");
    if installable == ".#robo" {
        install.arg("--no-update-lock-file");
    }
    install.arg(installable);
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
        "the robo CLI binary was not reinstalled; fix the Nix error, then rerun `robo update`.",
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
    fn update_workspace_rejects_non_robo_flake() {
        let root = temp_project("non-robo-flake");
        fs::write(root.join("flake.nix"), "{ outputs = _: {}; }\n").unwrap();

        let error = classify_update_workspace(&root).unwrap_err();

        assert!(error.message().contains("does not use a robo-nix flake"));
        cleanup(root);
    }

    #[test]
    fn update_workspace_recognizes_generated_project() {
        let root = temp_project("generated-project");
        fs::write(
            root.join("flake.nix"),
            r#"{
  inputs.robo-nix.url = "github:ausbxuse/robo-nix/master";
  outputs = {robo-nix, ...}:
    robo-nix.lib.mkProjectFlakeFromManifest ./robo.nix;
}
"#,
        )
        .unwrap();

        assert_eq!(
            classify_update_workspace(&root).unwrap(),
            UpdateWorkspaceKind::Project
        );
        cleanup(root);
    }

    #[test]
    fn update_workspace_recognizes_source_checkout() {
        let root = temp_project("source-checkout");
        fs::create_dir_all(root.join("src/nix")).unwrap();
        fs::create_dir_all(root.join("src/templates/project")).unwrap();
        fs::write(root.join("Cargo.toml"), "[package]\nname = \"robo\"\n").unwrap();
        fs::write(root.join("src/nix/project-flake.nix"), "{}\n").unwrap();
        fs::write(root.join("src/templates/project/flake.nix"), "{}\n").unwrap();
        fs::write(
            root.join("flake.nix"),
            "{ description = \"robo-nix\"; outputs = _: {}; }\n",
        )
        .unwrap();

        assert_eq!(
            classify_update_workspace(&root).unwrap(),
            UpdateWorkspaceKind::SourceCheckout
        );
        cleanup(root);
    }

    #[test]
    fn build_provenance_requires_exact_revision_and_timestamp() {
        let revision = "0123456789abcdef0123456789abcdef01234567";
        assert_eq!(
            BuildProvenance::from_fields(revision, "123"),
            Some(BuildProvenance {
                revision: revision.to_string(),
                last_modified: 123,
            })
        );
        assert!(BuildProvenance::from_fields("abc123", "123").is_none());
        assert!(BuildProvenance::from_fields(revision, "0").is_none());
        assert!(BuildProvenance::from_fields(revision, "not-a-time").is_none());
    }

    #[test]
    fn automatic_update_requires_newer_official_build() {
        let build = BuildProvenance {
            revision: "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".to_string(),
            last_modified: 200,
        };
        let mut locked = LockedGithubInput {
            owner: "ausbxuse".to_string(),
            repo: "robo-nix".to_string(),
            revision: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string(),
            last_modified: 100,
        };

        assert!(locked.is_older_than(&build));
        locked.last_modified = 300;
        assert!(!locked.is_older_than(&build));
        locked.last_modified = 100;
        locked.owner = "example".to_string();
        assert!(!locked.is_older_than(&build));
    }

    #[test]
    fn automatic_update_marker_records_build_and_lock_pair() {
        let root = temp_project("automatic-marker");
        let build = BuildProvenance {
            revision: "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".to_string(),
            last_modified: 200,
        };
        let locked = LockedGithubInput {
            owner: "ausbxuse".to_string(),
            repo: "robo-nix".to_string(),
            revision: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string(),
            last_modified: 100,
        };
        let attempt = automatic_update_attempt(&build, &locked);

        write_automatic_update_marker(&root, &attempt).unwrap();

        assert_eq!(
            fs::read_to_string(automatic_update_marker_path(&root)).unwrap(),
            attempt
        );
        cleanup(root);
    }

    #[test]
    fn clear_runtime_profile_state_removes_profiles_only() {
        let root = temp_project("clear-profiles");
        let profile_cache = root.join(".robo-nix").join("profiles").join("default");
        let venv = root.join(".robo-nix").join("venvs").join("default");
        fs::create_dir_all(profile_cache.join("runtime-gc-roots-v1/generation")).unwrap();
        fs::create_dir_all(&venv).unwrap();
        fs::write(profile_cache.join("runtime-env-cache-v2.env0"), "cache").unwrap();
        fs::write(
            profile_cache.join("runtime-gc-roots-v1/generation/root"),
            "root",
        )
        .unwrap();
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

    #[test]
    fn locked_installable_resolves_root_input_node_name() {
        let root = temp_project("aliased-lock-node");
        fs::write(
            root.join("flake.lock"),
            r#"{
  "nodes": {
    "root": { "inputs": { "robo-nix": "robo-nix_2" } },
    "robo-nix_2": {
      "locked": {
        "type": "github",
        "owner": "ausbxuse",
        "repo": "robo-nix",
        "rev": "def456"
      }
    }
  },
  "root": "root"
}
"#,
        )
        .unwrap();

        assert_eq!(
            locked_robo_nix_installable(&root).unwrap(),
            "github:ausbxuse/robo-nix/def456#robo"
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
