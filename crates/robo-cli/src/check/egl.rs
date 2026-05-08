pub(crate) const PROBE_SCRIPT: &str = r#"
set +u
printf 'XDG_SESSION_TYPE=%s\n' "${XDG_SESSION_TYPE:-}"
printf 'WAYLAND_DISPLAY=%s\n' "${WAYLAND_DISPLAY:-}"
printf 'DISPLAY=%s\n' "${DISPLAY:-}"
printf 'LIBGL_ALWAYS_SOFTWARE=%s\n' "${LIBGL_ALWAYS_SOFTWARE:-}"
printf 'MESA_LOADER_DRIVER_OVERRIDE=%s\n' "${MESA_LOADER_DRIVER_OVERRIDE:-}"
printf 'GBM_BACKEND=%s\n' "${GBM_BACKEND:-}"
if [ -f /.dockerenv ] || [ -f /run/.containerenv ] || [ -n "${container:-}" ]; then
  printf 'CONTAINER=present:%s\n' "${container:-unknown}"
else
  printf 'CONTAINER=none:\n'
fi

libegl=""
old_ifs="$IFS"
IFS=:
for dir in ${LD_LIBRARY_PATH:-}; do
  if [ -n "$dir" ] && [ -e "$dir/libEGL.so.1" ]; then
    libegl="$dir/libEGL.so.1"
    break
  fi
done
IFS="$old_ifs"
printf 'LIBEGL=%s\n' "$libegl"

if [ -z "${__EGL_VENDOR_LIBRARY_FILENAMES:-}" ]; then
  printf 'EGL_VENDOR_FILE=unset:\n'
else
  old_ifs="$IFS"
  IFS=:
  for path in $__EGL_VENDOR_LIBRARY_FILENAMES; do
    if [ -z "$path" ]; then
      continue
    fi
    if [ -e "$path" ]; then
      printf 'EGL_VENDOR_FILE=exists:%s\n' "$path"
    else
      printf 'EGL_VENDOR_FILE=missing:%s\n' "$path"
    fi
  done
  IFS="$old_ifs"
fi

if [ -d /dev/dri ]; then
  dri_nodes="$(find /dev/dri -maxdepth 1 -type c -printf '%f ' 2>/dev/null | sed 's/[[:space:]]*$//')"
  printf 'DEV_DRI=present:%s\n' "$dri_nodes"
else
  printf 'DEV_DRI=missing:\n'
fi

if [ -d /run/opengl-driver ]; then
  printf 'RUN_OPENGL_DRIVER=present:/run/opengl-driver\n'
else
  printf 'RUN_OPENGL_DRIVER=missing:/run/opengl-driver\n'
fi

printf 'ROBO_NIX_HOST_GRAPHICS_AUTO=%s\n' "${ROBO_NIX_HOST_GRAPHICS_AUTO:-}"
printf 'ROBO_NIX_HOST_GRAPHICS_LIB_DIRS_AUTO=%s\n' "${ROBO_NIX_HOST_GRAPHICS_LIB_DIRS_AUTO:-}"

if command -v glxinfo >/dev/null 2>&1; then
  glxinfo_output="$(timeout 5 glxinfo -B 2>/dev/null || true)"
  printf '%s\n' "$glxinfo_output" | while IFS= read -r line; do
    case "$line" in
      "direct rendering:"*) printf 'GLXINFO_DIRECT=%s\n' "${line#direct rendering: }" ;;
      "OpenGL vendor string:"*) printf 'GLXINFO_VENDOR=%s\n' "${line#OpenGL vendor string: }" ;;
      "OpenGL renderer string:"*) printf 'GLXINFO_RENDERER=%s\n' "${line#OpenGL renderer string: }" ;;
      "OpenGL version string:"*) printf 'GLXINFO_VERSION=%s\n' "${line#OpenGL version string: }" ;;
    esac
  done
else
  printf 'GLXINFO=missing:\n'
fi
"#;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct VendorFile {
    path: String,
    exists: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Probe {
    session_type: Option<String>,
    wayland_display: Option<String>,
    display: Option<String>,
    libgl_always_software: Option<String>,
    mesa_loader_driver_override: Option<String>,
    gbm_backend: Option<String>,
    container: Option<String>,
    libegl: Option<String>,
    vendor_files: Vec<VendorFile>,
    vendor_unset: bool,
    dev_dri: Option<String>,
    run_opengl_driver: bool,
    host_graphics_auto: Option<String>,
    host_graphics_lib_dirs_auto: Option<String>,
    glxinfo_missing: bool,
    glxinfo_direct: Option<String>,
    glxinfo_vendor: Option<String>,
    glxinfo_renderer: Option<String>,
    glxinfo_version: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FindingKind {
    Ok,
    Warn,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Finding {
    pub(crate) kind: FindingKind,
    pub(crate) message: String,
    pub(crate) hint: Option<&'static str>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MujocoContext {
    Ready,
    SkippedMissingVenv,
    NotSelected,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SummarySection {
    pub(crate) title: &'static str,
    pub(crate) rows: Vec<SummaryRow>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SummaryRow {
    pub(crate) kind: FindingKind,
    pub(crate) name: &'static str,
    pub(crate) value: String,
}

impl Finding {
    fn ok(message: impl Into<String>) -> Self {
        Self {
            kind: FindingKind::Ok,
            message: message.into(),
            hint: None,
        }
    }

    fn warn(message: impl Into<String>, hint: impl Into<Option<&'static str>>) -> Self {
        Self {
            kind: FindingKind::Warn,
            message: message.into(),
            hint: hint.into(),
        }
    }

    fn error(message: impl Into<String>, hint: impl Into<Option<&'static str>>) -> Self {
        Self {
            kind: FindingKind::Error,
            message: message.into(),
            hint: hint.into(),
        }
    }
}

pub(crate) fn parse(text: &str) -> Probe {
    let mut probe = Probe::default();
    for line in text.lines() {
        if let Some(value) = line.strip_prefix("XDG_SESSION_TYPE=") {
            probe.session_type = non_empty(value);
        } else if let Some(value) = line.strip_prefix("WAYLAND_DISPLAY=") {
            probe.wayland_display = non_empty(value);
        } else if let Some(value) = line.strip_prefix("DISPLAY=") {
            probe.display = non_empty(value);
        } else if let Some(value) = line.strip_prefix("LIBGL_ALWAYS_SOFTWARE=") {
            probe.libgl_always_software = non_empty(value);
        } else if let Some(value) = line.strip_prefix("MESA_LOADER_DRIVER_OVERRIDE=") {
            probe.mesa_loader_driver_override = non_empty(value);
        } else if let Some(value) = line.strip_prefix("GBM_BACKEND=") {
            probe.gbm_backend = non_empty(value);
        } else if let Some(value) = line.strip_prefix("CONTAINER=present:") {
            probe.container = Some(value.to_string());
        } else if line == "CONTAINER=none:" {
            probe.container = None;
        } else if let Some(value) = line.strip_prefix("LIBEGL=") {
            probe.libegl = non_empty(value);
        } else if line == "EGL_VENDOR_FILE=unset:" {
            probe.vendor_unset = true;
        } else if let Some(path) = line.strip_prefix("EGL_VENDOR_FILE=exists:") {
            probe.vendor_files.push(VendorFile {
                path: path.to_string(),
                exists: true,
            });
        } else if let Some(path) = line.strip_prefix("EGL_VENDOR_FILE=missing:") {
            probe.vendor_files.push(VendorFile {
                path: path.to_string(),
                exists: false,
            });
        } else if let Some(value) = line.strip_prefix("DEV_DRI=present:") {
            probe.dev_dri = Some(value.to_string());
        } else if line == "DEV_DRI=missing:" {
            probe.dev_dri = None;
        } else if line == "RUN_OPENGL_DRIVER=present:/run/opengl-driver" {
            probe.run_opengl_driver = true;
        } else if let Some(value) = line.strip_prefix("ROBO_NIX_HOST_GRAPHICS_AUTO=") {
            probe.host_graphics_auto = non_empty(value);
        } else if let Some(value) = line.strip_prefix("ROBO_NIX_HOST_GRAPHICS_LIB_DIRS_AUTO=") {
            probe.host_graphics_lib_dirs_auto = non_empty(value);
        } else if line == "GLXINFO=missing:" {
            probe.glxinfo_missing = true;
        } else if let Some(value) = line.strip_prefix("GLXINFO_DIRECT=") {
            probe.glxinfo_direct = non_empty(value);
        } else if let Some(value) = line.strip_prefix("GLXINFO_VENDOR=") {
            probe.glxinfo_vendor = non_empty(value);
        } else if let Some(value) = line.strip_prefix("GLXINFO_RENDERER=") {
            probe.glxinfo_renderer = non_empty(value);
        } else if let Some(value) = line.strip_prefix("GLXINFO_VERSION=") {
            probe.glxinfo_version = non_empty(value);
        }
    }
    probe
}

fn non_empty(value: &str) -> Option<String> {
    (!value.is_empty()).then(|| value.to_string())
}

pub(crate) fn findings(probe: &Probe) -> Vec<Finding> {
    let mut findings = Vec::new();
    findings.push(display_finding(probe));
    findings.push(libegl_finding(probe));
    findings.extend(host_container_findings(probe));

    if probe.vendor_unset {
        findings.push(Finding::warn(
            "EGL vendor file selection is unset",
            "EGL may fall through to host or container GLVND vendor files; desktop-gl should set __EGL_VENDOR_LIBRARY_FILENAMES by default",
        ));
        return findings;
    }

    if probe.vendor_files.is_empty() {
        findings.push(Finding::warn(
            "no EGL vendor files were reported",
            "set __EGL_VENDOR_LIBRARY_FILENAMES when a host-specific EGL vendor is required",
        ));
        return findings;
    }

    for vendor in &probe.vendor_files {
        if vendor.exists {
            findings.push(Finding::ok(format!(
                "EGL vendor file exists at {}",
                vendor.path
            )));
        } else {
            findings.push(Finding::error(
                format!("EGL vendor file is missing: {}", vendor.path),
                "unset __EGL_VENDOR_LIBRARY_FILENAMES to use the robo-nix default, or point it at an existing host EGL vendor JSON",
            ));
        }
    }

    if nix_libegl_with_host_vendor(probe) {
        findings.push(Finding::warn(
            "Nix libEGL is paired with a non-Nix EGL vendor file",
            "this is valid only when intentionally using a host EGL vendor; otherwise unset __EGL_VENDOR_LIBRARY_FILENAMES",
        ));
    }

    findings.extend(renderer_findings(probe));

    findings
}

pub(crate) fn summary_sections(
    probe: &Probe,
    mujoco_context: MujocoContext,
) -> Vec<SummarySection> {
    vec![
        SummarySection {
            title: "display",
            rows: vec![display_summary_row(probe), dri_summary_row(probe)],
        },
        SummarySection {
            title: "opengl / egl",
            rows: vec![
                libegl_summary_row(probe),
                vendor_summary_row(probe),
                host_driver_summary_row(probe),
            ],
        },
        SummarySection {
            title: "mujoco",
            rows: vec![mujoco_summary_row(mujoco_context)],
        },
    ]
}

fn display_summary_row(probe: &Probe) -> SummaryRow {
    match (
        probe.session_type.as_deref(),
        probe.wayland_display.as_deref(),
        probe.display.as_deref(),
    ) {
        (Some("wayland"), Some(display), _) => {
            summary_row(FindingKind::Ok, "session", format!("Wayland {display}"))
        }
        (Some("x11"), _, Some(display)) | (None, _, Some(display)) => {
            summary_row(FindingKind::Ok, "session", format!("X11 {display}"))
        }
        (Some(session), None, Some(display)) => summary_row(
            FindingKind::Ok,
            "session",
            format!("{session}, DISPLAY={display}"),
        ),
        (Some(session), None, None) => summary_row(
            FindingKind::Warn,
            "session",
            format!("{session}, no display socket"),
        ),
        _ => summary_row(FindingKind::Warn, "session", "no Wayland or X11 display"),
    }
}

fn dri_summary_row(probe: &Probe) -> SummaryRow {
    match probe.dev_dri.as_deref() {
        Some("") => summary_row(FindingKind::Warn, "devices", "/dev/dri has no device nodes"),
        Some(nodes) => summary_row(FindingKind::Ok, "devices", nodes),
        None => summary_row(FindingKind::Warn, "devices", "/dev/dri is not visible"),
    }
}

fn libegl_summary_row(probe: &Probe) -> SummaryRow {
    match probe.libegl.as_deref() {
        Some(path) => summary_row(FindingKind::Ok, "libEGL", graphics_path_summary(path)),
        None => summary_row(FindingKind::Error, "libEGL", "not visible"),
    }
}

fn vendor_summary_row(probe: &Probe) -> SummaryRow {
    if probe.vendor_unset {
        return summary_row(FindingKind::Warn, "vendor", "selection unset");
    }
    match probe.vendor_files.first() {
        Some(vendor) if vendor.exists => {
            summary_row(FindingKind::Ok, "vendor", graphics_vendor_summary(&vendor.path))
        }
        Some(vendor) => summary_row(FindingKind::Error, "vendor", format!("missing {}", vendor.path)),
        None => summary_row(FindingKind::Warn, "vendor", "not reported"),
    }
}

fn host_driver_summary_row(probe: &Probe) -> SummaryRow {
    if probe.run_opengl_driver {
        summary_row(FindingKind::Ok, "host driver", "visible")
    } else if probe.container.is_some() {
        summary_row(FindingKind::Warn, "host driver", "/run/opengl-driver not visible")
    } else {
        summary_row(FindingKind::Warn, "host driver", "not visible")
    }
}

fn mujoco_summary_row(context: MujocoContext) -> SummaryRow {
    match context {
        MujocoContext::Ready => summary_row(
            FindingKind::Ok,
            "context",
            "current GL settings can create a MuJoCo OpenGL context",
        ),
        MujocoContext::SkippedMissingVenv => summary_row(
            FindingKind::Warn,
            "context",
            ".venv is missing, so the Python MuJoCo probe did not run",
        ),
        MujocoContext::NotSelected => {
            summary_row(FindingKind::Ok, "context", "mujoco component is not selected")
        }
    }
}

fn summary_row(
    kind: FindingKind,
    name: &'static str,
    value: impl Into<String>,
) -> SummaryRow {
    SummaryRow {
        kind,
        name,
        value: value.into(),
    }
}

fn graphics_path_summary(path: &str) -> String {
    if path.contains("libglvnd") {
        "Nix libglvnd".to_string()
    } else if path.starts_with("/nix/store/") {
        "Nix store".to_string()
    } else {
        path.to_string()
    }
}

fn graphics_vendor_summary(path: &str) -> String {
    let provider = if path.contains("mesa") || path.ends_with("50_mesa.json") {
        "Mesa"
    } else if path.contains("nvidia") {
        "NVIDIA"
    } else {
        "EGL vendor"
    };
    let source = if path.starts_with("/nix/store/") {
        "Nix"
    } else if path.starts_with("/run/opengl-driver/") {
        "host driver"
    } else {
        path
    };
    format!("{provider} from {source}")
}

fn display_finding(probe: &Probe) -> Finding {
    match (
        probe.session_type.as_deref(),
        probe.wayland_display.as_deref(),
        probe.display.as_deref(),
    ) {
        (Some("wayland"), Some(display), _) => {
            Finding::ok(format!("Wayland display is visible ({display})"))
        }
        (Some("x11"), _, Some(display)) | (None, _, Some(display)) => {
            Finding::ok(format!("X11 display is visible ({display})"))
        }
        (Some(session), None, Some(display)) => {
            Finding::ok(format!("display is visible ({session}, DISPLAY={display})"))
        }
        (Some(session), None, None) => Finding::warn(
            format!("graphics session is {session} but no display socket variable is set"),
            "run graphical simulator viewers from a desktop session or pass through the display socket to the container",
        ),
        _ => Finding::warn(
            "no Wayland or X11 display variable is visible in the runtime shell",
            "set WAYLAND_DISPLAY or DISPLAY by running from a graphical desktop session",
        ),
    }
}

fn libegl_finding(probe: &Probe) -> Finding {
    match probe.libegl.as_deref() {
        Some(path) => Finding::ok(format!("libEGL is visible at {path}")),
        None => Finding::error(
            "libEGL.so.1 is not visible in the runtime library path",
            "include the desktop-gl component for desktop OpenGL/EGL clients",
        ),
    }
}

fn host_container_findings(probe: &Probe) -> Vec<Finding> {
    let mut findings = Vec::new();

    if probe.libgl_always_software.as_deref() == Some("1") {
        findings.push(Finding::warn(
            "LIBGL_ALWAYS_SOFTWARE=1 forces software rendering",
            "unset LIBGL_ALWAYS_SOFTWARE before running simulator viewers that need hardware OpenGL",
        ));
    }

    match probe.dev_dri.as_deref() {
        Some("") => findings.push(Finding::warn(
            "/dev/dri exists but no DRM device nodes were visible",
            "pass GPU render devices into the container; Distrobox usually needs host /dev/dri visibility for hardware rendering",
        )),
        Some(nodes) => findings.push(Finding::ok(format!("/dev/dri devices visible ({nodes})"))),
        None => findings.push(Finding::warn(
            "/dev/dri is not visible",
            "pass GPU render devices into the container; otherwise OpenGL may fall back to software rendering",
        )),
    }

    if let Some(container) = &probe.container {
        findings.push(Finding::ok(format!("container runtime detected ({container})")));
    }

    if probe.run_opengl_driver {
        findings.push(Finding::ok("/run/opengl-driver is visible"));
    } else if probe.container.is_some() {
        findings.push(Finding::warn(
            "/run/opengl-driver is not visible",
            "on NixOS hosts, Distrobox may need /run/opengl-driver mounted so container graphics can use the host driver stack",
        ));
    }

    if let Some(manifests) = &probe.host_graphics_auto {
        findings.push(Finding::ok(format!(
            "robo host graphics bridge selected {manifests}"
        )));
    }
    if let Some(dirs) = &probe.host_graphics_lib_dirs_auto {
        findings.push(Finding::ok(format!(
            "robo host graphics libraries visible at {dirs}"
        )));
    }

    if let Some(value) = &probe.mesa_loader_driver_override {
        findings.push(Finding::warn(
            format!("MESA_LOADER_DRIVER_OVERRIDE={value} is set"),
            "manual Mesa driver overrides can force the wrong renderer inside containers",
        ));
    }
    if let Some(value) = &probe.gbm_backend {
        findings.push(Finding::ok(format!("GBM_BACKEND={value}")));
    }

    findings
}

fn renderer_findings(probe: &Probe) -> Vec<Finding> {
    let mut findings = Vec::new();

    if probe.glxinfo_missing && probe.container.is_some() {
        findings.push(Finding::warn(
            "OpenGL renderer probe is unavailable because glxinfo is missing",
            "robo could not confirm hardware acceleration; install mesa-demos or run an equivalent renderer probe inside the runtime",
        ));
        return findings;
    }

    if let Some(direct) = &probe.glxinfo_direct {
        findings.push(Finding::ok(format!("GLX direct rendering: {direct}")));
    }
    if let Some(vendor) = &probe.glxinfo_vendor {
        findings.push(Finding::ok(format!("OpenGL vendor: {vendor}")));
    }
    if let Some(version) = &probe.glxinfo_version {
        findings.push(Finding::ok(format!("OpenGL version: {version}")));
    }
    if let Some(renderer) = &probe.glxinfo_renderer {
        if software_renderer(renderer) {
            findings.push(Finding::warn(
                format!("OpenGL renderer appears to be software: {renderer}"),
                "hardware acceleration is not active; fix container GPU/device/driver visibility before debugging MuJoCo",
            ));
        } else {
            findings.push(Finding::ok(format!("OpenGL renderer: {renderer}")));
        }
    }

    findings
}

fn software_renderer(renderer: &str) -> bool {
    let renderer = renderer.to_ascii_lowercase();
    renderer.contains("llvmpipe")
        || renderer.contains("softpipe")
        || renderer.contains("software rasterizer")
        || renderer.contains("swrast")
}

fn nix_libegl_with_host_vendor(probe: &Probe) -> bool {
    probe
        .libegl
        .as_deref()
        .is_some_and(|path| path.starts_with("/nix/store/"))
        && probe
            .vendor_files
            .iter()
            .any(|vendor| vendor.exists && non_nix_unmanaged_vendor(probe, &vendor.path))
}

fn non_nix_unmanaged_vendor(probe: &Probe, vendor_path: &str) -> bool {
    !vendor_path.starts_with("/nix/store/")
        && !probe
            .host_graphics_auto
            .as_deref()
            .is_some_and(|manifests| manifests.split(':').any(|path| path == vendor_path))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_display_libegl_and_vendor_files() {
        let probe = parse(
            "XDG_SESSION_TYPE=wayland\n\
             WAYLAND_DISPLAY=wayland-0\n\
             DISPLAY=:0\n\
             LIBGL_ALWAYS_SOFTWARE=\n\
             MESA_LOADER_DRIVER_OVERRIDE=\n\
             GBM_BACKEND=nvidia-drm\n\
             CONTAINER=present:podman\n\
             LIBEGL=/nix/store/abc-libglvnd/lib/libEGL.so.1\n\
             EGL_VENDOR_FILE=exists:/nix/store/def-mesa/share/glvnd/egl_vendor.d/50_mesa.json\n\
             EGL_VENDOR_FILE=missing:/tmp/missing.json\n\
             DEV_DRI=present:card0 renderD128\n\
             RUN_OPENGL_DRIVER=present:/run/opengl-driver\n\
             ROBO_NIX_HOST_GRAPHICS_AUTO=/run/opengl-driver/share/glvnd/egl_vendor.d/10_nvidia.json\n\
             ROBO_NIX_HOST_GRAPHICS_LIB_DIRS_AUTO=/run/opengl-driver/lib\n\
             GLXINFO_DIRECT=Yes\n\
             GLXINFO_VENDOR=NVIDIA Corporation\n\
             GLXINFO_RENDERER=NVIDIA GeForce RTX\n\
             GLXINFO_VERSION=4.6.0 vendor-driver\n",
        );

        assert_eq!(probe.session_type.as_deref(), Some("wayland"));
        assert_eq!(probe.wayland_display.as_deref(), Some("wayland-0"));
        assert_eq!(probe.display.as_deref(), Some(":0"));
        assert_eq!(probe.gbm_backend.as_deref(), Some("nvidia-drm"));
        assert_eq!(probe.container.as_deref(), Some("podman"));
        assert_eq!(
            probe.libegl.as_deref(),
            Some("/nix/store/abc-libglvnd/lib/libEGL.so.1")
        );
        assert_eq!(
            probe.vendor_files,
            vec![
                VendorFile {
                    path: "/nix/store/def-mesa/share/glvnd/egl_vendor.d/50_mesa.json"
                        .to_string(),
                    exists: true,
                },
                VendorFile {
                    path: "/tmp/missing.json".to_string(),
                    exists: false,
                },
            ]
        );
        assert_eq!(probe.dev_dri.as_deref(), Some("card0 renderD128"));
        assert!(probe.run_opengl_driver);
        assert_eq!(
            probe.host_graphics_auto.as_deref(),
            Some("/run/opengl-driver/share/glvnd/egl_vendor.d/10_nvidia.json")
        );
        assert_eq!(
            probe.host_graphics_lib_dirs_auto.as_deref(),
            Some("/run/opengl-driver/lib")
        );
        assert_eq!(probe.glxinfo_direct.as_deref(), Some("Yes"));
        assert_eq!(
            probe.glxinfo_vendor.as_deref(),
            Some("NVIDIA Corporation")
        );
        assert_eq!(probe.glxinfo_renderer.as_deref(), Some("NVIDIA GeForce RTX"));
        assert_eq!(probe.glxinfo_version.as_deref(), Some("4.6.0 vendor-driver"));
        assert!(!probe.vendor_unset);
    }

    #[test]
    fn records_unset_vendor_selection() {
        let probe = parse(
            "XDG_SESSION_TYPE=\n\
             WAYLAND_DISPLAY=\n\
             DISPLAY=\n\
             LIBEGL=\n\
             EGL_VENDOR_FILE=unset:\n",
        );

        assert_eq!(
            probe,
            Probe {
                vendor_unset: true,
                ..Probe::default()
            }
        );
    }

    #[test]
    fn missing_vendor_file_is_an_error() {
        let probe = Probe {
            session_type: Some("wayland".to_string()),
            wayland_display: Some("wayland-0".to_string()),
            libegl: Some("/nix/store/abc-libglvnd/lib/libEGL.so.1".to_string()),
            vendor_files: vec![VendorFile {
                path: "/tmp/missing.json".to_string(),
                exists: false,
            }],
            ..Probe::default()
        };

        assert!(findings(&probe).iter().any(|finding| {
            finding.kind == FindingKind::Error
                && finding
                    .message
                    .contains("EGL vendor file is missing: /tmp/missing.json")
        }));
    }

    #[test]
    fn nix_libegl_with_non_nix_vendor_file_is_a_warning() {
        let probe = Probe {
            session_type: Some("wayland".to_string()),
            wayland_display: Some("wayland-0".to_string()),
            libegl: Some("/nix/store/abc-libglvnd/lib/libEGL.so.1".to_string()),
            vendor_files: vec![VendorFile {
                path: "/usr/share/glvnd/egl_vendor.d/10_nvidia.json".to_string(),
                exists: true,
            }],
            ..Probe::default()
        };

        assert!(findings(&probe).iter().any(|finding| {
            finding.kind == FindingKind::Warn
                && finding
                    .message
                    .contains("Nix libEGL is paired with a non-Nix EGL vendor file")
        }));
    }

    #[test]
    fn nix_libegl_with_robo_managed_host_vendor_file_is_not_a_warning() {
        let probe = Probe {
            session_type: Some("wayland".to_string()),
            wayland_display: Some("wayland-0".to_string()),
            libegl: Some("/nix/store/abc-libglvnd/lib/libEGL.so.1".to_string()),
            vendor_files: vec![VendorFile {
                path: "/run/opengl-driver/share/glvnd/egl_vendor.d/10_nvidia.json".to_string(),
                exists: true,
            }],
            host_graphics_auto: Some(
                "/run/opengl-driver/share/glvnd/egl_vendor.d/10_nvidia.json:/run/opengl-driver/share/vulkan/icd.d/nvidia_icd.json"
                    .to_string(),
            ),
            ..Probe::default()
        };

        assert!(!findings(&probe).iter().any(|finding| {
            finding.kind == FindingKind::Warn
                && finding
                    .message
                    .contains("Nix libEGL is paired with a non-Nix EGL vendor file")
        }));
    }

    #[test]
    fn software_renderer_is_a_container_graphics_warning() {
        let probe = Probe {
            session_type: Some("wayland".to_string()),
            wayland_display: Some("wayland-0".to_string()),
            container: Some("podman".to_string()),
            libegl: Some("/nix/store/abc-libglvnd/lib/libEGL.so.1".to_string()),
            vendor_files: vec![VendorFile {
                path: "/nix/store/def-mesa/share/glvnd/egl_vendor.d/50_mesa.json".to_string(),
                exists: true,
            }],
            dev_dri: Some("card0 renderD128".to_string()),
            glxinfo_renderer: Some("llvmpipe (LLVM 19.1.7, 256 bits)".to_string()),
            ..Probe::default()
        };

        let findings = findings(&probe);
        assert!(findings.iter().any(|finding| {
            finding.kind == FindingKind::Warn
                && finding
                    .message
                    .contains("OpenGL renderer appears to be software")
        }));
        assert!(findings.iter().any(|finding| {
            finding.kind == FindingKind::Warn
                && finding.message.contains("/run/opengl-driver is not visible")
        }));
    }

    #[test]
    fn forced_software_flag_is_a_warning() {
        let probe = Probe {
            libgl_always_software: Some("1".to_string()),
            ..Probe::default()
        };

        assert!(findings(&probe).iter().any(|finding| {
            finding.kind == FindingKind::Warn
                && finding
                    .message
                    .contains("LIBGL_ALWAYS_SOFTWARE=1 forces software rendering")
        }));
    }

    #[test]
    fn summary_sections_use_probe_fields_not_finding_text() {
        let probe = Probe {
            session_type: Some("wayland".to_string()),
            wayland_display: Some("wayland-0".to_string()),
            libegl: Some("/nix/store/abc-libglvnd/lib/libEGL.so.1".to_string()),
            vendor_files: vec![VendorFile {
                path: "/nix/store/def-mesa/share/glvnd/egl_vendor.d/50_mesa.json".to_string(),
                exists: true,
            }],
            dev_dri: Some("card0 renderD128".to_string()),
            run_opengl_driver: true,
            ..Probe::default()
        };

        let sections = summary_sections(&probe, MujocoContext::Ready);

        assert_eq!(sections[0].title, "display");
        assert_eq!(sections[0].rows[0].value, "Wayland wayland-0");
        assert_eq!(sections[0].rows[1].value, "card0 renderD128");
        assert_eq!(sections[1].rows[0].value, "Nix libglvnd");
        assert_eq!(sections[1].rows[1].value, "Mesa from Nix");
        assert_eq!(sections[1].rows[2].value, "visible");
        assert_eq!(
            sections[2].rows[0].value,
            "current GL settings can create a MuJoCo OpenGL context"
        );
    }
}
