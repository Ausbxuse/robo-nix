use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

const REVISION_ENV: &str = "ROBO_NIX_BUILD_REVISION";
const LAST_MODIFIED_ENV: &str = "ROBO_NIX_BUILD_LAST_MODIFIED";

fn main() {
    println!("cargo:rerun-if-env-changed={REVISION_ENV}");
    println!("cargo:rerun-if-env-changed={LAST_MODIFIED_ENV}");
    track_git_head();

    if let (Ok(revision), Ok(last_modified)) = (env::var(REVISION_ENV), env::var(LAST_MODIFIED_ENV))
    {
        if valid_provenance(&revision, &last_modified) {
            return;
        }
    }

    let Some(revision) = git_output(&["rev-parse", "HEAD"]) else {
        return;
    };
    let Some(last_modified) = git_output(&["show", "-s", "--format=%ct", "HEAD"]) else {
        return;
    };
    if !valid_provenance(&revision, &last_modified) {
        return;
    }

    println!("cargo:rustc-env={REVISION_ENV}={revision}");
    println!("cargo:rustc-env={LAST_MODIFIED_ENV}={last_modified}");
}

fn valid_provenance(revision: &str, last_modified: &str) -> bool {
    revision.len() == 40
        && revision.bytes().all(|byte| byte.is_ascii_hexdigit())
        && last_modified
            .parse::<u64>()
            .ok()
            .is_some_and(|value| value > 0)
}

fn track_git_head() {
    let Some(git_dir) = git_output(&["rev-parse", "--git-dir"]) else {
        return;
    };
    let git_dir = PathBuf::from(git_dir);
    let head = git_dir.join("HEAD");
    println!("cargo:rerun-if-changed={}", head.display());
    let Ok(contents) = fs::read_to_string(&head) else {
        return;
    };
    if let Some(reference) = contents.trim().strip_prefix("ref: ") {
        println!(
            "cargo:rerun-if-changed={}",
            git_dir.join(reference).display()
        );
    }
}

fn git_output(args: &[&str]) -> Option<String> {
    let output = Command::new("git").args(args).output().ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).trim().to_string())
}
