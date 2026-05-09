use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::io;
use std::path::Path;

use crate::error::AppError;
use crate::inference::{
    infer_initial_runtime, PyprojectStatus, RuntimeInference, KNOWN_COMPONENTS,
};
use crate::nix_env::with_project_lock;
use crate::ui::{attention, detail, row, section, success, Config};

const PROJECT_FLAKE_TEMPLATE: &str = include_str!("templates/project/flake.nix");
const PROJECT_ROBO_TEMPLATE: &str = include_str!("templates/project/robo.nix");

pub(crate) fn prepare_project(root: &Path) -> Result<BootstrapReport, AppError> {
    let python_version = read_python_version(root)?;
    fs::create_dir_all(root.join(".robo-nix"))
        .map_err(|err| AppError::project(format!("failed to create .robo-nix/: {err}")))?;

    // NOTE: shell bootstraps missing files only. Existing robo.nix is user-owned.
    with_project_lock(root, "bootstrap", || {
        prepare_project_locked(root, python_version)
    })
}

fn prepare_project_locked(
    root: &Path,
    python_version: String,
) -> Result<BootstrapReport, AppError> {
    let mut report = BootstrapReport::default();
    let flake_path = root.join("flake.nix");
    if flake_path.exists() {
        let flake = fs::read_to_string(&flake_path)
            .map_err(|err| AppError::project(format!("failed to read flake.nix: {err}")))?;
        if !looks_like_robo_flake(&flake) {
            return Err(
                AppError::project("this repository already has a non-robo flake.nix")
                    .with_hint("robo shell will not overwrite an existing non-robo flake."),
            );
        }
    } else {
        fs::write(&flake_path, render_flake_nix()?)
            .map_err(|err| AppError::project(format!("failed to write flake.nix: {err}")))?;
        report.wrote_flake = true;
    }

    let robo_path = root.join("robo.nix");
    if !robo_path.exists() {
        let inference = infer_initial_runtime(root)?;
        let robo_nix = render_robo_nix(&inference)?;
        fs::write(&robo_path, robo_nix)
            .map_err(|err| AppError::project(format!("failed to write robo.nix: {err}")))?;
        report.wrote_robo_nix = true;
        report.inference = Some(inference);
    }

    report.python_version = python_version;
    Ok(report)
}

pub(crate) fn read_python_version(root: &Path) -> Result<String, AppError> {
    let path = root.join(".python-version");
    let raw = fs::read_to_string(&path).map_err(|err| {
        if err.kind() == io::ErrorKind::NotFound {
            AppError::project("missing .python-version")
                .with_hint("choose the project Python version first, for example with `uv python pin <version>`.")
        } else {
            AppError::project(format!("failed to read .python-version: {err}"))
        }
    })?;
    let version = raw.lines().next().unwrap_or("").trim();
    if version.is_empty() {
        return Err(AppError::project(".python-version is empty")
            .with_hint("write the project Python version, for example `3.11` or `3.12`."));
    }
    Ok(version.to_string())
}

pub(crate) fn print_bootstrap_report(config: Config, report: &BootstrapReport) {
    if report.wrote_flake || report.wrote_robo_nix {
        section(config, "generated");
        if report.wrote_flake {
            row(config, "✓", "wrote", "./flake.nix");
        }
        if report.wrote_robo_nix {
            row(config, "✓", "wrote", "./robo.nix");
        }
    }

    if let Some(inference) = &report.inference {
        print_inference_report(config, inference);
    }
}

fn print_inference_report(config: Config, inference: &RuntimeInference) {
    match inference.pyproject_status {
        PyprojectStatus::Missing => {
            section(config, "attention");
            attention(
                config,
                "pyproject.toml not found; generated base runtime only",
            );
        }
        PyprojectStatus::Invalid => {
            section(config, "attention");
            attention(
                config,
                "pyproject.toml is invalid TOML; generated base runtime only",
            );
        }
        PyprojectStatus::Read => {
            if inference.matches.is_empty() {
                return;
            }
            section(config, "inferred");
            for matched in &inference.matches {
                success(
                    config,
                    &matched.component,
                    &format!("pyproject.toml dependency `{}`", matched.package),
                );
                detail(
                    config,
                    &format!(
                        "capability `{}` from {}; sources: {}",
                        matched.capability,
                        matched.provenance,
                        matched.sources.join(", ")
                    ),
                );
                detail(config, &matched.note);
            }
        }
    }
}

fn looks_like_robo_flake(flake: &str) -> bool {
    flake.contains("mkProjectFlakeFromManifest")
        || (flake.contains("nixpkgs-python")
            && flake.contains("import ./robo.nix")
            && flake.contains(".python-version"))
}

fn render_robo_nix(inference: &RuntimeInference) -> Result<String, AppError> {
    render_template(
        PROJECT_ROBO_TEMPLATE,
        &[("components", render_component_lines(inference))],
    )
}

fn render_flake_nix() -> Result<String, AppError> {
    render_template(
        PROJECT_FLAKE_TEMPLATE,
        &[("robo_nix_url", escape_nix_string(&robo_nix_source_url()))],
    )
}

fn robo_nix_source_url() -> String {
    env::var("ROBO_NIX_DEFAULT_SOURCE_URL")
        .unwrap_or_else(|_| "github:ausbxuse/robo-nix/rewrite".to_string())
}

fn escape_nix_string(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace("${", "\\${")
}

fn render_component_lines(inference: &RuntimeInference) -> String {
    let mut packages_by_component: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for matched in &inference.matches {
        packages_by_component
            .entry(&matched.component)
            .or_default()
            .push(&matched.package);
    }

    let mut lines = Vec::new();
    for component in KNOWN_COMPONENTS {
        if !inference.components.contains(*component) {
            continue;
        }
        if let Some(packages) = packages_by_component.get(component) {
            lines.push(format!(
                "    \"{component}\" # inferred from pyproject.toml: {}",
                packages.join(", ")
            ));
        } else {
            lines.push(format!("    \"{component}\""));
        }
    }
    lines.join("\n")
}

fn render_template(template: &str, values: &[(&str, String)]) -> Result<String, AppError> {
    let mut rendered = template.to_string();
    for (key, value) in values {
        rendered = rendered.replace(&format!("{{{{{key}}}}}"), value);
    }
    if rendered.contains("{{") || rendered.contains("}}") {
        return Err(AppError::project(
            "template rendering left an unresolved placeholder",
        ));
    }
    Ok(rendered)
}

#[derive(Debug, Default)]
pub(crate) struct BootstrapReport {
    python_version: String,
    wrote_flake: bool,
    wrote_robo_nix: bool,
    inference: Option<RuntimeInference>,
}

impl BootstrapReport {
    pub(crate) fn python_version(&self) -> &str {
        &self.python_version
    }

    pub(crate) fn inference(&self) -> Option<&RuntimeInference> {
        self.inference.as_ref()
    }

    pub(crate) fn wrote_files(&self) -> Vec<&'static str> {
        let mut files = Vec::new();
        if self.wrote_flake {
            files.push("flake.nix");
        }
        if self.wrote_robo_nix {
            files.push("robo.nix");
        }
        files
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn bootstrap_requires_python_version() {
        let root = temp_project("requires-python");

        let error = prepare_project(&root).unwrap_err();
        assert!(error.message().contains(".python-version"));
        assert!(!root.join("flake.nix").exists());

        cleanup(root);
    }

    #[test]
    fn bootstrap_writes_runtime_files_without_pyproject() {
        let root = temp_project("base");
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join(".python-version"), "3.12\n").unwrap();

        let report = prepare_project(&root).unwrap();

        assert_eq!(report.python_version, "3.12");
        assert!(report.wrote_flake);
        assert!(report.wrote_robo_nix);
        assert!(root.join(".robo-nix").is_dir());
        assert!(!root.join("pyproject.toml").exists());
        assert!(fs::read_to_string(root.join("flake.nix"))
            .unwrap()
            .contains("robo-nix.lib.mkProjectFlakeFromManifest ./robo.nix"));
        assert!(fs::read_to_string(root.join("robo.nix"))
            .unwrap()
            .contains("\"python-uv\""));

        cleanup(root);
    }

    #[test]
    fn first_bootstrap_infers_initial_components_from_pyproject() {
        let root = temp_project("inference");
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join(".python-version"), "3.11\n").unwrap();
        fs::write(
            root.join("pyproject.toml"),
            r#"[project]
dependencies = [
  "torch>=2",
  "mujoco",
]
"#,
        )
        .unwrap();

        prepare_project(&root).unwrap();
        let robo_nix = fs::read_to_string(root.join("robo.nix")).unwrap();

        assert!(robo_nix.contains("\"python-uv\""));
        assert!(robo_nix.contains("\"native-build\" # inferred from pyproject.toml:"));
        assert!(robo_nix.contains("mujoco"));
        assert!(robo_nix.contains("torch"));
        assert!(robo_nix.contains("\"desktop-gl\" # inferred from pyproject.toml: mujoco"));

        cleanup(root);
    }

    #[test]
    fn first_bootstrap_infers_components_from_optional_dependencies_and_groups() {
        let root = temp_project("group-inference");
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join(".python-version"), "3.11\n").unwrap();
        fs::write(
            root.join("pyproject.toml"),
            r#"[project]
dependencies = []

[project.optional-dependencies]
sim = [
  "dm-control",
]

[dependency-groups]
gpu = [
  "flash-attn",
]
"#,
        )
        .unwrap();

        prepare_project(&root).unwrap();
        let robo_nix = fs::read_to_string(root.join("robo.nix")).unwrap();

        assert!(robo_nix.contains("\"native-build\" # inferred from pyproject.toml:"));
        assert!(robo_nix.contains("\"desktop-gl\" # inferred from pyproject.toml: dm-control"));
        assert!(robo_nix.contains("\"cuda-toolkit\" # inferred from pyproject.toml: flash-attn"));

        cleanup(root);
    }

    #[test]
    fn first_bootstrap_infers_cuda_toolkit_for_cuda_python_packages() {
        let root = temp_project("cuda-inference");
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join(".python-version"), "3.11\n").unwrap();
        fs::write(
            root.join("pyproject.toml"),
            r#"[project]
dependencies = [
  "cuda-python",
  "cupy-cuda12x",
]
"#,
        )
        .unwrap();

        prepare_project(&root).unwrap();
        let robo_nix = fs::read_to_string(root.join("robo.nix")).unwrap();

        assert!(robo_nix.contains("\"cuda-toolkit\" # inferred from pyproject.toml:"));
        assert!(robo_nix.contains("cuda-python"));
        assert!(robo_nix.contains("cupy-cuda12x"));

        cleanup(root);
    }

    #[test]
    fn first_bootstrap_infers_linux_headers_for_evdev() {
        let root = temp_project("evdev-inference");
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join(".python-version"), "3.11\n").unwrap();
        fs::write(
            root.join("pyproject.toml"),
            r#"[project]
dependencies = [
  "evdev<1.9.3; sys_platform == 'linux'",
]
"#,
        )
        .unwrap();

        prepare_project(&root).unwrap();
        let robo_nix = fs::read_to_string(root.join("robo.nix")).unwrap();

        assert!(robo_nix.contains("\"native-build\" # inferred from pyproject.toml: evdev"));
        assert!(robo_nix.contains("\"linux-headers\" # inferred from pyproject.toml: evdev"));

        cleanup(root);
    }

    #[test]
    fn existing_robo_nix_is_canonical() {
        let root = temp_project("existing-robo");
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join(".python-version"), "3.11\n").unwrap();
        fs::write(
            root.join("robo.nix"),
            "{ components = [ \"python-uv\" ]; }\n",
        )
        .unwrap();
        fs::write(
            root.join("pyproject.toml"),
            r#"[project]
dependencies = ["opencv-python"]
"#,
        )
        .unwrap();

        prepare_project(&root).unwrap();

        assert_eq!(
            fs::read_to_string(root.join("robo.nix")).unwrap(),
            "{ components = [ \"python-uv\" ]; }\n"
        );

        cleanup(root);
    }

    #[test]
    fn non_robo_flake_is_refused() {
        let root = temp_project("non-robo-flake");
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join(".python-version"), "3.11\n").unwrap();
        fs::write(root.join("flake.nix"), "{ outputs = _: {}; }\n").unwrap();

        let error = prepare_project(&root).unwrap_err();

        assert!(error.message().contains("non-robo flake"));
        assert!(!root.join("robo.nix").exists());

        cleanup(root);
    }

    fn temp_project(label: &str) -> PathBuf {
        let root = env::temp_dir().join(format!("robo-minimal-{label}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        root
    }

    fn cleanup(root: PathBuf) {
        let _ = fs::remove_dir_all(root);
    }
}
