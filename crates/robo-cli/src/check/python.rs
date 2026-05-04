use std::fs;
use std::path::Path;

use crate::runtime::ProjectRuntime;
use crate::{Config, exact_python_requirement};

use super::output::{check_error, check_hint, check_ok, check_warn};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum PythonEnvironmentOrigin {
    Missing,
    NixBacked(String),
    HostBacked(String),
}

pub(super) fn python_environment_origin() -> PythonEnvironmentOrigin {
    if !Path::new(".venv").is_dir() {
        return PythonEnvironmentOrigin::Missing;
    }

    let origin = fs::read_to_string(".venv/pyvenv.cfg")
        .ok()
        .and_then(|config| {
            config.lines().find_map(|line| {
                let (name, value) = line.split_once('=')?;
                (name.trim() == "home").then(|| value.trim().to_string())
            })
        })
        .or_else(|| {
            fs::canonicalize(".venv/bin/python")
                .ok()
                .map(|path| path.display().to_string())
        });

    match origin {
        Some(path) if path.starts_with("/nix/store/") => PythonEnvironmentOrigin::NixBacked(path),
        Some(path) => PythonEnvironmentOrigin::HostBacked(path),
        None => PythonEnvironmentOrigin::HostBacked(".venv/bin/python".to_string()),
    }
}

pub(super) fn check_python_files(
    config: Config,
    runtime: &ProjectRuntime,
    pyproject: Option<&str>,
    issues: &mut usize,
    warnings: &mut usize,
) {
    match fs::read_to_string(".python-version") {
        Ok(version) => {
            let version = version.trim();
            if version == runtime.python_version {
                check_ok(
                    config,
                    &format!(".python-version matches {}", runtime.python_version),
                );
            } else {
                check_warn(
                    config,
                    warnings,
                    &format!(
                        ".python-version is {version} but robo.nix declares {}",
                        runtime.python_version
                    ),
                );
                check_hint(
                    config,
                    "update .python-version or pythonVersion in robo.nix",
                );
            }
        }
        Err(_) => {
            check_warn(config, warnings, ".python-version is missing");
            check_hint(
                config,
                &format!(
                    "create .python-version with {} so uv uses the intended interpreter",
                    runtime.python_version
                ),
            );
        }
    }

    if let Some(pyproject) = pyproject {
        if let Some(required) = exact_python_requirement(pyproject) {
            if required == runtime.python_version {
                check_ok(
                    config,
                    &format!("pyproject.toml requires Python {required}"),
                );
            } else {
                check_error(
                    config,
                    issues,
                    &format!(
                        "pyproject.toml requires Python {required} but robo.nix declares {}",
                        runtime.python_version
                    ),
                );
                check_hint(
                    config,
                    &format!(
                        "set `pythonVersion = \"{required}\";` in robo.nix and write `{required}` to .python-version"
                    ),
                );
            }
        }
    } else {
        check_warn(config, warnings, "pyproject.toml is missing");
        check_hint(config, "run `robo init .` or create pyproject.toml for uv");
    }
}

pub(super) fn check_python_environment(config: Config, issues: &mut usize, warnings: &mut usize) {
    match python_environment_origin() {
        PythonEnvironmentOrigin::Missing => {
            check_warn(config, warnings, "Python virtualenv is missing");
            check_hint(config, "run `robo shell`, then `uv sync`");
        }
        PythonEnvironmentOrigin::NixBacked(origin) => {
            check_ok(
                config,
                &format!("Python virtualenv uses robo-nix Python ({origin})"),
            );
        }
        PythonEnvironmentOrigin::HostBacked(origin) => {
            check_error(
                config,
                issues,
                "Python virtualenv was created outside robo-nix",
            );
            check_hint(config, &format!("found interpreter origin: {origin}"));
            check_hint(
                config,
                "run `robo shell -c 'uv venv --python \"$ROBO_NIX_PYTHON\" --clear && uv sync'`",
            );
            check_hint(
                config,
                "Nix runtime libraries require an ABI-aligned Python process on older distros",
            );
        }
    }
}
