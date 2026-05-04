use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use crate::{Config, quoted_value};

use super::output::{check_hint, check_warn};

const NATIVE_TOOL_WHEEL_PACKAGES: &[&str] = &["cmake", "ninja", "patchelf"];

pub(super) fn check_native_tool_wheel_shims(config: Config, warnings: &mut usize) {
    let tools = native_tool_wheel_shims();
    if tools.is_empty() {
        return;
    }

    check_warn(
        config,
        warnings,
        &format!(
            "Python virtualenv contains native build tool shims: {}",
            tools.join(", ")
        ),
    );
    check_hint(
        config,
        "uv owns Python packages; Nix owns CMake, Ninja, compilers, and native build tools",
    );
    check_hint(
        config,
        "project bootstrap scripts should call native build tools from the runtime, not .venv/bin shims",
    );
}

pub(super) fn native_tool_wheel_shims() -> Vec<String> {
    let package_names = fs::read_to_string("uv.lock")
        .ok()
        .map(|lock| uv_lock_package_names(&lock))
        .unwrap_or_default();

    NATIVE_TOOL_WHEEL_PACKAGES
        .iter()
        .filter(|name| package_names.contains(**name))
        .filter(|name| Path::new(".venv/bin").join(name).exists())
        .map(|name| (*name).to_string())
        .collect()
}

fn uv_lock_package_names(text: &str) -> BTreeSet<String> {
    let mut names = BTreeSet::new();
    let mut in_package = false;
    let mut package_name_seen = false;

    for line in text.lines().map(str::trim) {
        if line == "[[package]]" {
            in_package = true;
            package_name_seen = false;
            continue;
        }

        if line.starts_with('[') {
            in_package = false;
            continue;
        }

        if in_package
            && !package_name_seen
            && line.starts_with("name")
            && let Some(name) = quoted_value(line)
        {
            names.insert(normalize_package_name(name));
            package_name_seen = true;
        }
    }

    names
}

fn normalize_package_name(name: &str) -> String {
    name.chars()
        .map(|ch| match ch {
            '_' | '.' => '-',
            _ => ch.to_ascii_lowercase(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_top_level_uv_lock_package_names() {
        let lock = r#"
[[package]]
name = "cmake"
version = "4.1.3"
dependencies = [
  { name = "not-a-package-entry" },
]

[[package]]
name = "Ninja"
version = "1.13.0"
"#;

        let names = uv_lock_package_names(lock);

        assert!(names.contains("cmake"));
        assert!(names.contains("ninja"));
        assert!(!names.contains("not-a-package-entry"));
    }
}
