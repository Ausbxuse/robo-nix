use std::collections::BTreeSet;
use std::path::Path;
use std::process::ExitCode;

use crate::runtime::ProjectRuntime;
use crate::{Config, LabelKind, UiProgress, combined_output, run_bootstrap_with_progress};

use super::egl;
use super::output::{check_error, check_hint, check_line, check_ok, check_warn};
use super::runtime_command;

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct NixDryRunItem {
    drv_path: String,
    outputs: std::collections::BTreeMap<String, String>,
}

pub(super) fn run_deep_checks(
    config: Config,
    runtime: &ProjectRuntime,
    pyproject_dependencies: &BTreeSet<String>,
    issues: &mut usize,
    warnings: &mut usize,
) -> Result<(), ExitCode> {
    let mut progress = UiProgress::new(config, 6, "running deep checks");
    check_runtime_preview(config, warnings, Some(&mut progress));
    if let Err(code) = run_bootstrap_with_progress(config, &mut progress) {
        progress.finish();
        return Err(code);
    }
    check_runtime_tools(config, issues, &mut progress);
    check_runtime_egl_glvnd_surface(config, runtime, issues, warnings, &mut progress);
    check_runtime_cuda_build_surface(config, runtime, issues, &mut progress);
    check_runtime_probes(config, pyproject_dependencies, warnings, &mut progress);
    progress.finish();
    Ok(())
}

fn check_runtime_preview(config: Config, warnings: &mut usize, progress: Option<&mut UiProgress>) {
    let mut command = crate::nix_command(config);
    command.arg("build");
    crate::add_runtime_source_override(&mut command);
    command.args([".#default", "--dry-run", "--no-link", "--json"]);

    let mut progress = progress;
    let output = match progress.as_deref_mut() {
        Some(progress) => progress.output(&mut command, "checking runtime download plan"),
        None => crate::output_with_spinner(config, &mut command, "checking runtime download plan"),
    };

    let print_output =
        |warnings: &mut usize, output: Result<std::process::Output, std::io::Error>| match output {
            Ok(output) if output.status.success() => {
                let text = combined_output(&output);
                let summary = summarize_nix_dry_run_json(&output.stdout);
                if summary.is_empty() {
                    check_line(
                        config,
                        "preview:",
                        LabelKind::Status,
                        "runtime is already available or Nix reported no downloads",
                    );
                } else {
                    for line in summary {
                        check_line(config, "preview:", LabelKind::Status, &line);
                    }
                }
                if config.debug {
                    check_hint(config, &text);
                }
            }
            Ok(output) => {
                check_warn(config, warnings, "could not preview runtime downloads");
                check_hint(config, &combined_output(&output));
            }
            Err(err) => {
                check_warn(
                    config,
                    warnings,
                    &format!("failed to start Nix preview: {err}"),
                );
            }
        };

    match progress {
        Some(progress) => progress.suspend(|| print_output(warnings, output)),
        None => print_output(warnings, output),
    }
}

fn summarize_nix_dry_run_json(stdout: &[u8]) -> Vec<String> {
    let Ok(items) = serde_json::from_slice::<Vec<NixDryRunItem>>(stdout) else {
        return Vec::new();
    };
    if items.is_empty() {
        return Vec::new();
    }
    let output_count = items.iter().map(|item| item.outputs.len()).sum::<usize>();
    let mut summary = vec![format!(
        "planned builds: {} derivation{} producing {} output{}",
        items.len(),
        if items.len() == 1 { "" } else { "s" },
        output_count,
        if output_count == 1 { "" } else { "s" }
    )];
    if items.len() == 1 {
        summary.push(format!("derivation: {}", items[0].drv_path));
    }
    summary
}

fn check_runtime_tools(config: Config, issues: &mut usize, progress: &mut UiProgress) {
    let mut command = runtime_command(config, "uv", ["--version"], []);
    let output = progress.output(&mut command, "checking runtime tools");
    progress.suspend(|| match output {
        Ok(output) if output.status.success() => check_ok(config, "uv is available"),
        Ok(output) => {
            check_error(config, issues, "uv is not available in the runtime shell");
            check_hint(config, &combined_output(&output));
        }
        Err(err) => {
            check_error(
                config,
                issues,
                &format!("failed to probe uv in runtime shell: {err}"),
            );
        }
    });
}

fn check_runtime_egl_glvnd_surface(
    config: Config,
    runtime: &ProjectRuntime,
    issues: &mut usize,
    warnings: &mut usize,
    progress: &mut UiProgress,
) {
    if !runtime
        .components
        .iter()
        .any(|component| component == "desktop-gl")
    {
        progress.step("skipping EGL/GLVND graphics surface");
        return;
    }

    let mut command = runtime_command(config, "bash", ["-lc", egl::PROBE_SCRIPT], []);
    let output = progress.output(&mut command, "checking EGL/GLVND graphics surface");
    progress.suspend(|| match output {
        Ok(output) if output.status.success() => {
            let text = String::from_utf8_lossy(&output.stdout);
            for finding in egl::findings(&egl::parse(&text)) {
                match finding.kind {
                    egl::FindingKind::Ok => check_ok(config, &finding.message),
                    egl::FindingKind::Warn => {
                        check_warn(config, warnings, &finding.message);
                        if let Some(hint) = finding.hint {
                            check_hint(config, hint);
                        }
                    }
                    egl::FindingKind::Error => {
                        check_error(config, issues, &finding.message);
                        if let Some(hint) = finding.hint {
                            check_hint(config, hint);
                        }
                    }
                }
            }
        }
        Ok(output) => {
            check_warn(config, warnings, "could not inspect EGL/GLVND runtime state");
            check_hint(config, &combined_output(&output));
        }
        Err(err) => {
            check_warn(
                config,
                warnings,
                &format!("failed to probe EGL/GLVND runtime state: {err}"),
            );
        }
    });
}

fn check_runtime_cuda_build_surface(
    config: Config,
    runtime: &ProjectRuntime,
    issues: &mut usize,
    progress: &mut UiProgress,
) {
    if !runtime
        .components
        .iter()
        .any(|component| component == "cuda-toolkit")
    {
        progress.step("skipping CUDA native build surface");
        return;
    }

    let script = r#"
root="${ROBO_NIX_CUDA_ROOT:-${CUDA_HOME:-${CUDA_PATH:-}}}"
if [ -z "$root" ] || [ ! -d "$root" ]; then
  printf 'CUDA_HOME/CUDA_PATH did not point at a toolkit\n'
  exit 1
fi

missing=0
for path in \
  "$root/bin/nvcc" \
  "$root/include/cuda_runtime.h" \
  "$root/include/cuda_runtime_api.h" \
  "$root/include/nv/target" \
  "$root/lib/libcudart.so"
do
  if [ ! -e "$path" ]; then
    printf 'missing %s\n' "$path"
    missing=1
  fi
done

case ":${LIBRARY_PATH:-}:" in
  *":$root/lib:"*) ;;
  *)
    printf 'LIBRARY_PATH does not include %s/lib\n' "$root"
    missing=1
    ;;
esac

if [ "$missing" -ne 0 ]; then
  exit 1
fi

printf 'root=%s\n' "$root"
"#;

    let mut command = runtime_command(config, "bash", ["-lc", script], []);
    let output = progress.output(&mut command, "checking CUDA native build surface");
    progress.suspend(|| match output {
        Ok(output) if output.status.success() => {
            let text = String::from_utf8_lossy(&output.stdout);
            let root = text
                .lines()
                .find_map(|line| line.strip_prefix("root="))
                .unwrap_or("runtime CUDA toolkit");
            check_ok(
                config,
                &format!("CUDA native build surface is available at {root}"),
            );
        }
        Ok(output) => {
            check_error(config, issues, "CUDA native build surface is incomplete");
            check_hint(
                config,
                "Nix owns nvcc, CUDA headers, CCCL headers, and the libcudart link surface for native extension builds",
            );
            check_hint(
                config,
                "uv owns PyTorch and nvidia-* CUDA runtime wheels such as cuBLAS, cuDNN, and NCCL",
            );
            check_hint(config, &combined_output(&output));
        }
        Err(err) => {
            check_error(
                config,
                issues,
                &format!("failed to probe CUDA native build surface: {err}"),
            );
        }
    });
}

fn check_runtime_probes(
    config: Config,
    pyproject_dependencies: &BTreeSet<String>,
    warnings: &mut usize,
    progress: &mut UiProgress,
) {
    if pyproject_dependencies.is_empty() {
        progress.step("skipping Python runtime probes");
        return;
    }
    progress.step("checking Python runtime probes");

    if has_dependency(pyproject_dependencies, &["pyqt6", "pyqt5", "pyside6"]) {
        if !Path::new(".venv/bin/python").exists() {
            progress.suspend(|| {
                check_warn(
                    config,
                    warnings,
                    "Python virtualenv is missing; skipped Qt binding import probe",
                );
                check_hint(
                    config,
                    "run 'robo shell', then run 'uv sync' before GUI runtime probing",
                );
            });
        } else {
            let code = "from PyQt6 import QtCore, QtGui, QtWidgets; print(QtCore.QT_VERSION_STR)";
            let mut command = runtime_command(config, ".venv/bin/python", ["-c", code], []);
            let output = progress.output_current(&mut command, "checking PyQt6 GUI import");
            progress.suspend(|| match output {
                Ok(output) if output.status.success() => {
                    check_ok(config, "PyQt6 QtCore/QtGui/QtWidgets import works")
                }
                Ok(output) => {
                    check_warn(config, warnings, "PyQt6 GUI import failed");
                    check_hint(config, &combined_output(&output));
                    check_hint(
                        config,
                        "run 'uv sync' after changing Python dependencies or add missing native runtime components",
                    );
                }
                Err(err) => check_warn(
                    config,
                    warnings,
                    &format!("failed to run PyQt6 GUI probe: {err}"),
                ),
            });
        }
    }

    if has_dependency(pyproject_dependencies, &["matplotlib"]) {
        if !Path::new(".venv/bin/python").exists() {
            progress.suspend(|| {
                check_warn(
                    config,
                    warnings,
                    "Python virtualenv is missing; skipped matplotlib backend probe",
                );
                check_hint(
                    config,
                    "run 'robo shell', then run 'uv sync' before matplotlib runtime probing",
                );
            });
        } else {
            let code = "import matplotlib.pyplot as plt; fig = plt.figure(); print(type(fig.canvas).__name__)";
            let mut command = runtime_command(
                config,
                ".venv/bin/python",
                ["-c", code],
                [("MPLBACKEND", "QtAgg")],
            );
            let output = progress.output_current(&mut command, "checking matplotlib QtAgg probe");
            progress.suspend(|| match output {
                Ok(output) if output.status.success() => check_ok(
                    config,
                    "matplotlib QtAgg backend probe works",
                ),
                Ok(output) => {
                    check_warn(config, warnings, "matplotlib QtAgg backend probe failed");
                    check_hint(config, &combined_output(&output));
                    check_hint(
                        config,
                        "install a Qt binding such as pyqt6 and include qt6,desktop-gl when using plt.show()",
                    );
                }
                Err(err) => check_warn(
                    config,
                    warnings,
                    &format!("failed to run matplotlib QtAgg probe: {err}"),
                ),
            });
        }
    }

    if has_dependency(pyproject_dependencies, &["torchcodec"]) {
        if !Path::new(".venv/bin/python").exists() {
            progress.suspend(|| {
                check_warn(
                    config,
                    warnings,
                    "Python virtualenv is missing; skipped TorchCodec import probe",
                );
                check_hint(
                    config,
                    "run 'robo shell', then run 'uv sync' before video decoder runtime probing",
                );
            });
        } else {
            let code = "import torchcodec; print('torchcodec ok')";
            let mut command = runtime_command(config, ".venv/bin/python", ["-c", code], []);
            let output = progress.output_current(&mut command, "checking TorchCodec import");
            progress.suspend(|| match output {
                Ok(output) if output.status.success() => check_ok(
                    config,
                    "TorchCodec import works with the runtime FFmpeg libraries",
                ),
                Ok(output) => {
                    check_warn(config, warnings, "TorchCodec import failed");
                    check_hint(
                        config,
                        "TorchCodec needs FFmpeg shared libraries from the media component",
                    );
                    check_hint(config, &combined_output(&output));
                }
                Err(err) => check_warn(
                    config,
                    warnings,
                    &format!("failed to run TorchCodec import probe: {err}"),
                ),
            });
        }
    }
}

fn has_dependency(dependencies: &BTreeSet<String>, names: &[&str]) -> bool {
    crate::pyproject::has_dependency_name(dependencies, names.iter().copied())
}
