use std::env;
use std::fs;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::{Duration, Instant};

use crate::error::AppError;

const DEFAULT_LOCK_TIMEOUT_SECONDS: u64 = 30;

pub(crate) fn with_project_lock<T, F>(
    workspace: &Path,
    name: &str,
    action: F,
) -> Result<T, AppError>
where
    F: FnOnce() -> Result<T, AppError>,
{
    let state_dir = workspace.join(".robo-nix");
    fs::create_dir_all(&state_dir)
        .map_err(|err| AppError::project(format!("failed to create .robo-nix/: {err}")))?;
    let lock_path = state_dir.join(format!("{name}.lock"));
    let timeout = project_lock_timeout();
    let start = Instant::now();

    loop {
        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&lock_path)
        {
            Ok(mut file) => {
                if let Err(err) = writeln!(file, "pid={}\nname={name}", std::process::id()) {
                    let _ = fs::remove_file(&lock_path);
                    return Err(AppError::project(format!(
                        "failed to write {}: {err}",
                        lock_path.display()
                    )));
                }
                let _guard = ProjectLockGuard { path: lock_path };
                return action();
            }
            Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => {
                if start.elapsed() >= timeout {
                    return Err(AppError::project(format!(
                        "timed out waiting for robo project lock {}",
                        lock_path.display()
                    ))
                    .with_hint(format!(
                        "another robo process may be preparing this project; remove {} only if no robo process is active.",
                        lock_path.display()
                    )));
                }
                thread::sleep(Duration::from_millis(50));
            }
            Err(err) => {
                return Err(AppError::project(format!(
                    "failed to create robo project lock {}: {err}",
                    lock_path.display()
                )));
            }
        }
    }
}

fn project_lock_timeout() -> Duration {
    env::var("ROBO_NIX_LOCK_TIMEOUT")
        .ok()
        .and_then(|value| value.trim().parse::<u64>().ok())
        .map(Duration::from_secs)
        .unwrap_or_else(|| Duration::from_secs(DEFAULT_LOCK_TIMEOUT_SECONDS))
}

struct ProjectLockGuard {
    path: PathBuf,
}

impl Drop for ProjectLockGuard {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn project_lock_times_out_when_lock_is_held() {
        let root = temp_project("lock-timeout");
        fs::create_dir_all(root.join(".robo-nix")).unwrap();
        fs::write(root.join(".robo-nix").join("held.lock"), b"pid=test\n").unwrap();
        let previous = env::var_os("ROBO_NIX_LOCK_TIMEOUT");
        env::set_var("ROBO_NIX_LOCK_TIMEOUT", "0");

        let error = with_project_lock(&root, "held", || Ok(())).unwrap_err();

        assert!(error.message().contains("timed out waiting"));
        match previous {
            Some(value) => {
                env::set_var("ROBO_NIX_LOCK_TIMEOUT", value);
            }
            None => {
                env::remove_var("ROBO_NIX_LOCK_TIMEOUT");
            }
        }
        cleanup(root);
    }

    fn temp_project(label: &str) -> PathBuf {
        let root = env::temp_dir().join(format!("robo-lock-{label}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        root
    }

    fn cleanup(root: PathBuf) {
        let _ = fs::remove_dir_all(root);
    }
}
