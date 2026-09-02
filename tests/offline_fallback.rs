#![cfg(unix)]

use std::env;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;

const CACHE_MAGIC: &str = "robo-nix-runtime-env-cache-v2";

#[test]
fn failed_nix_evaluation_launches_the_last_working_environment() {
    let expects_automatic_update = build_provenance_available();
    let root = temp_project("offline-fallback");
    let fake_bin = root.join("fake-bin");
    let cache_dir = root.join(".robo-nix/profiles/default");
    fs::create_dir_all(&fake_bin).unwrap();
    fs::create_dir_all(&cache_dir).unwrap();
    fs::write(root.join(".python-version"), "3.12\n").unwrap();
    fs::write(
        root.join("flake.nix"),
        "{ outputs = { self }: {}; } # mkProjectFlakeFromManifest\n",
    )
    .unwrap();
    fs::write(root.join("robo.nix"), "{}\n").unwrap();
    fs::write(
        root.join("flake.lock"),
        r#"{
  "nodes": {
    "root": { "inputs": { "robo-nix": "robo-nix" } },
    "robo-nix": {
      "locked": {
        "lastModified": 1,
        "owner": "ausbxuse",
        "repo": "robo-nix",
        "rev": "0000000000000000000000000000000000000000",
        "type": "github"
      }
    }
  },
  "root": "root",
  "version": 7
}
"#,
    )
    .unwrap();

    let fake_nix = fake_bin.join("nix");
    let nix_log = root.join("nix-commands.log");
    fs::write(
        &fake_nix,
        "#!/bin/sh\nprintf '%s\\n' \"$*\" >> \"$ROBO_NIX_TEST_NIX_LOG\"\nprintf '%s\\n' 'error: offline test: network unavailable' >&2\nexit 1\n",
    )
    .unwrap();
    let mut permissions = fs::metadata(&fake_nix).unwrap().permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&fake_nix, permissions).unwrap();

    let cache_path = cache_dir.join("runtime-env-cache-v2.env0");
    let mut cache = format!("{CACHE_MAGIC}\nold-launch-key\nold-environment-key\n").into_bytes();
    cache.extend_from_slice(b"PATH=/usr/bin:/bin\0");
    cache.extend_from_slice(b"ROBO_NIX_PYTHON_GROUPS=last-working\0");
    fs::write(&cache_path, &cache).unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_robo"))
        .current_dir(&root)
        .arg("run")
        .arg("sh")
        .arg("-c")
        .arg("printf 'fallback-ok:%s\\n' \"$ROBO_NIX_PYTHON_GROUPS\"")
        .env_clear()
        .env("PATH", format!("{}:/usr/bin:/bin", fake_bin.display()))
        .env("ROBO_NIX_TEST_NIX_LOG", &nix_log)
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "stdout:\n{stdout}\nstderr:\n{stderr}"
    );
    assert_eq!(
        stdout.contains("project robo-nix lock was not updated automatically"),
        expects_automatic_update
    );
    assert!(stdout.contains("using the last working runtime environment"));
    assert!(stdout.contains("fallback-ok:last-working"));
    assert!(stderr.contains("offline test: network unavailable"));
    assert_eq!(
        root.join(".robo-nix/tooling-update-attempt-v1").is_file(),
        expects_automatic_update
    );
    assert!(fs::read(&cache_path)
        .unwrap()
        .starts_with(format!("{CACHE_MAGIC}\nold-launch-key\nold-environment-key\n").as_bytes()));
    assert!(fs::read_to_string(root.join(".robo-nix/last-run.json"))
        .unwrap()
        .contains("runtime_environment=last-working-fallback"));

    let second = Command::new(env!("CARGO_BIN_EXE_robo"))
        .current_dir(&root)
        .arg("run")
        .arg("sh")
        .arg("-c")
        .arg("exit 0")
        .env_clear()
        .env("PATH", format!("{}:/usr/bin:/bin", fake_bin.display()))
        .env("ROBO_NIX_TEST_NIX_LOG", &nix_log)
        .output()
        .unwrap();
    assert!(second.status.success());
    assert_eq!(
        fs::read_to_string(&nix_log)
            .unwrap()
            .matches("flake update robo-nix")
            .count(),
        usize::from(expects_automatic_update),
        "the same offline automatic update must be attempted only once"
    );

    cleanup(root);
}

fn build_provenance_available() -> bool {
    let Some(revision) = option_env!("ROBO_NIX_BUILD_REVISION") else {
        return false;
    };
    let Some(last_modified) = option_env!("ROBO_NIX_BUILD_LAST_MODIFIED") else {
        return false;
    };
    revision.len() == 40
        && revision.bytes().all(|byte| byte.is_ascii_hexdigit())
        && last_modified
            .parse::<u64>()
            .ok()
            .is_some_and(|value| value > 0)
}

fn temp_project(label: &str) -> PathBuf {
    let root = env::temp_dir().join(format!("robo-{label}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).unwrap();
    root
}

fn cleanup(root: impl AsRef<Path>) {
    let _ = fs::remove_dir_all(root);
}
