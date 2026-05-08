use std::env;
use std::ffi::OsString;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

const PYTHON_VERSION: &str = "3.11";
const REQUIRED_FILES: &[&str] = &["flake.nix", "robo.nix", "pyproject.toml", ".python-version"];

fn main() -> ExitCode {
    match run(env::args_os().skip(1).collect()) {
        Ok(code) => code,
        Err(message) => {
            eprintln!("error: {message}");
            ExitCode::FAILURE
        }
    }
}

fn run(args: Vec<OsString>) -> Result<ExitCode, String> {
    let mut args = args.into_iter();
    let Some(command) = args.next() else {
        print_usage();
        return Ok(ExitCode::SUCCESS);
    };
    let command = command
        .to_str()
        .ok_or_else(|| "command must be valid UTF-8".to_string())?;

    match command {
        "init" => init_command(args.collect()),
        "check" => check_command(args.collect()),
        "shell" => shell_command(args.collect()),
        "run" => run_command(args.collect()),
        "-h" | "--help" | "help" => {
            print_usage();
            Ok(ExitCode::SUCCESS)
        }
        other => Err(format!("unknown command `{other}`")),
    }
}

fn print_usage() {
    println!(
        "Usage:
  robo init [path] [--force]
  robo check
  robo shell
  robo run <command> [args...]"
    );
}

fn init_command(args: Vec<OsString>) -> Result<ExitCode, String> {
    let mut target = None;
    let mut force = false;

    for arg in args {
        if arg == "--force" {
            force = true;
            continue;
        }
        if arg.to_string_lossy().starts_with('-') {
            return Err(format!("unknown init option `{}`", arg.to_string_lossy()));
        }
        if target.is_some() {
            return Err("init accepts at most one path".to_string());
        }
        target = Some(PathBuf::from(arg));
    }

    let target = target.unwrap_or_else(|| PathBuf::from("."));
    init_project(&target, force).map_err(|err| format!("init failed: {err}"))?;
    println!("created {}", target.display());
    Ok(ExitCode::SUCCESS)
}

fn check_command(args: Vec<OsString>) -> Result<ExitCode, String> {
    if !args.is_empty() {
        return Err("check does not accept arguments".to_string());
    }

    let mut missing = Vec::new();
    for file in REQUIRED_FILES {
        if Path::new(file).is_file() {
            println!("ok: {file}");
        } else {
            println!("missing: {file}");
            missing.push(*file);
        }
    }

    if Path::new("uv.lock").is_file() {
        println!("ok: uv.lock");
    } else {
        println!("note: uv.lock is absent; run `uv sync` when project dependencies are ready");
    }

    if missing.is_empty() {
        Ok(ExitCode::SUCCESS)
    } else {
        Err(format!("missing generated files: {}", missing.join(", ")))
    }
}

fn shell_command(args: Vec<OsString>) -> Result<ExitCode, String> {
    if !args.is_empty() {
        return Err("shell does not accept arguments; use `robo run` for commands".to_string());
    }
    run_nix_develop(Vec::new())
}

fn run_command(args: Vec<OsString>) -> Result<ExitCode, String> {
    if args.is_empty() {
        return Err("run requires a command".to_string());
    }
    run_nix_develop(args)
}

fn run_nix_develop(command_args: Vec<OsString>) -> Result<ExitCode, String> {
    let mut command = Command::new("nix");
    command.arg("develop").arg("--accept-flake-config");

    if !command_args.is_empty() {
        command.arg("--command").args(command_args);
    }

    let status = command
        .status()
        .map_err(|err| format!("failed to start nix: {err}"))?;
    if status.success() {
        Ok(ExitCode::SUCCESS)
    } else {
        Ok(ExitCode::FAILURE)
    }
}

fn init_project(target: &Path, force: bool) -> io::Result<()> {
    fs::create_dir_all(target)?;
    let project_name = project_name(target);
    let files = [
        (".python-version", format!("{PYTHON_VERSION}\n")),
        ("pyproject.toml", render_pyproject(&project_name)),
        ("flake.nix", render_flake()),
        ("robo.nix", render_robo_nix()),
        (".gitignore", ".venv/\n__pycache__/\n.robo-nix/\nresult\n".to_string()),
    ];

    for (name, _) in &files {
        let path = target.join(name);
        if path.exists() && !force {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                format!("{} already exists; pass --force to replace generated files", path.display()),
            ));
        }
    }

    for (name, contents) in files {
        fs::write(target.join(name), contents)?;
    }

    Ok(())
}

fn project_name(target: &Path) -> String {
    let raw = target
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .unwrap_or("robot-learning");

    let mut name = String::new();
    for character in raw.chars() {
        if character.is_ascii_alphanumeric() {
            name.push(character.to_ascii_lowercase());
        } else if !name.ends_with('-') {
            name.push('-');
        }
    }

    let name = name.trim_matches('-');
    if name.is_empty() {
        "robot-learning".to_string()
    } else {
        name.to_string()
    }
}

fn render_pyproject(project_name: &str) -> String {
    format!(
        r#"[project]
name = "{project_name}"
version = "0.1.0"
requires-python = "=={PYTHON_VERSION}.*"
dependencies = []
"#
    )
}

fn render_flake() -> String {
    r#"{
  description = "Robot learning development environment";

  nixConfig = {
    substituters = ["https://cache.nixos.org"];
    extra-substituters = ["https://nixpkgs-python.cachix.org"];
    extra-trusted-public-keys = [
      "nixpkgs-python.cachix.org-1:hxjI7pFxTyuTHn2NkvWCrAUcNZLNS3ZAvfYNuYifcEU="
    ];
  };

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    nixpkgs-python = {
      url = "github:cachix/nixpkgs-python";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = { nixpkgs, nixpkgs-python, ... }:
    let
      systems = ["x86_64-linux" "aarch64-linux"];
      forAllSystems = nixpkgs.lib.genAttrs systems;
    in {
      devShells = forAllSystems (system:
        let
          pkgs = import nixpkgs { inherit system; };
          python = nixpkgs-python.packages.${system}."3.11";
          project = import ./robo.nix { inherit pkgs python; };
        in {
          default = pkgs.mkShell {
            packages = project.packages;
            shellHook = project.shellHook;
          };
        });
    };
}
"#
    .to_string()
}

fn render_robo_nix() -> String {
    r#"{ pkgs, python }:

{
  packages = [
    python
    pkgs.uv
  ];

  shellHook = ''
    export ROBO_NIX_PYTHON="${python}/bin/python"
    export UV_PYTHON="$ROBO_NIX_PYTHON"
    export UV_PYTHON_DOWNLOADS=never
    export UV_PROJECT_ENVIRONMENT="$PWD/.venv"
    unset PYTHONHOME
    unset PYTHONPATH
  '';
}
"#
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn project_name_is_python_package_friendly() {
        assert_eq!(project_name(Path::new("Robot Learning!")), "robot-learning");
        assert_eq!(project_name(Path::new("___")), "robot-learning");
    }

    #[test]
    fn init_writes_minimal_project_files() {
        let root = temp_project("writes");
        init_project(&root, false).unwrap();

        assert_eq!(fs::read_to_string(root.join(".python-version")).unwrap(), "3.11\n");
        assert!(fs::read_to_string(root.join("pyproject.toml"))
            .unwrap()
            .contains("requires-python = \"==3.11.*\""));
        assert!(fs::read_to_string(root.join("flake.nix"))
            .unwrap()
            .contains("nixpkgs-python"));
        assert!(fs::read_to_string(root.join("robo.nix"))
            .unwrap()
            .contains("UV_PYTHON_DOWNLOADS=never"));

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn init_refuses_to_replace_generated_files_without_force() {
        let root = temp_project("refuses");
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("flake.nix"), "existing").unwrap();

        let error = init_project(&root, false).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::AlreadyExists);
        assert_eq!(fs::read_to_string(root.join("flake.nix")).unwrap(), "existing");

        init_project(&root, true).unwrap();
        assert!(fs::read_to_string(root.join("flake.nix"))
            .unwrap()
            .contains("nixpkgs-python"));

        fs::remove_dir_all(root).unwrap();
    }

    fn temp_project(label: &str) -> PathBuf {
        let root = env::temp_dir().join(format!("robo-minimal-{label}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        root
    }
}
