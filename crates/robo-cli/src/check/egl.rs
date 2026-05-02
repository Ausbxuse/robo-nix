pub(crate) const PROBE_SCRIPT: &str = r#"
set +u
printf 'XDG_SESSION_TYPE=%s\n' "${XDG_SESSION_TYPE:-}"
printf 'WAYLAND_DISPLAY=%s\n' "${WAYLAND_DISPLAY:-}"
printf 'DISPLAY=%s\n' "${DISPLAY:-}"

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
    libegl: Option<String>,
    vendor_files: Vec<VendorFile>,
    vendor_unset: bool,
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

    if probe.vendor_unset {
        findings.push(Finding::warn(
            "EGL vendor file selection is unset",
            "EGL may fall through to host or container GLVND vendor files; x11-gl should set __EGL_VENDOR_LIBRARY_FILENAMES by default",
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

    findings
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
            "include the x11-gl component for desktop OpenGL/EGL clients",
        ),
    }
}

fn nix_libegl_with_host_vendor(probe: &Probe) -> bool {
    probe
        .libegl
        .as_deref()
        .is_some_and(|path| path.starts_with("/nix/store/"))
        && probe
            .vendor_files
            .iter()
            .any(|vendor| vendor.exists && !vendor.path.starts_with("/nix/store/"))
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
             LIBEGL=/nix/store/abc-libglvnd/lib/libEGL.so.1\n\
             EGL_VENDOR_FILE=exists:/nix/store/def-mesa/share/glvnd/egl_vendor.d/50_mesa.json\n\
             EGL_VENDOR_FILE=missing:/tmp/missing.json\n",
        );

        assert_eq!(probe.session_type.as_deref(), Some("wayland"));
        assert_eq!(probe.wayland_display.as_deref(), Some("wayland-0"));
        assert_eq!(probe.display.as_deref(), Some(":0"));
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
}
