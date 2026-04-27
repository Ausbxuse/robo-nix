use clap::Args;
use serde::Serialize;
use std::env;
use std::fs;
use std::path::Path;
use std::process::ExitCode;

use crate::runtime::{build_runtime_why, read_project_runtime, WhyEntry};
use crate::{error, ensure_project_runtime, nix_command, quoted_value, Config};

#[derive(Args)]
pub(crate) struct ContractArgs {
    #[arg(long, help = "Emit machine-readable runtime contract")]
    json: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct RuntimeContract {
    env_name: String,
    schema_version: Option<String>,
    python_version: String,
    system: String,
    workspace_root: String,
    default_derivation: Option<String>,
    flake_lock_present: bool,
    source: Option<String>,
    components: Vec<WhyEntry>,
    required_directories: Vec<WhyEntry>,
    required_files: Vec<WhyEntry>,
    bootstrap_scripts: Vec<WhyEntry>,
}

pub(crate) fn run(args: ContractArgs, config: Config) -> ExitCode {
    if let Err(code) = ensure_project_runtime(config) {
        return code;
    }

    let runtime = read_project_runtime();
    let why = build_runtime_why(&runtime);
    let contract = RuntimeContract {
        env_name: runtime.env_name.clone(),
        schema_version: runtime.schema_version.clone(),
        python_version: runtime.python_version.clone(),
        system: nix_system(),
        workspace_root: env::current_dir()
            .map_or_else(|_| ".".into(), |path| path.display().to_string()),
        default_derivation: default_derivation_name(config),
        flake_lock_present: Path::new("flake.lock").is_file(),
        source: flake_robo_nix_source(),
        components: why.components,
        required_directories: why.required_directories,
        required_files: why.required_files,
        bootstrap_scripts: why.bootstrap_scripts,
    };

    if args.json {
        match serde_json::to_string_pretty(&contract) {
            Ok(text) => println!("{text}"),
            Err(err) => {
                error(config, &format!("failed to encode runtime contract: {err}"));
                return ExitCode::from(1);
            }
        }
        return ExitCode::SUCCESS;
    }

    println!("contract: env={}", contract.env_name);
    if let Some(version) = &contract.schema_version {
        println!("contract: schemaVersion={version}");
    }
    println!("contract: python={}", contract.python_version);
    println!("contract: system={}", contract.system);
    println!("contract: workspace={}", contract.workspace_root);
    if let Some(name) = &contract.default_derivation {
        println!("contract: derivation={name}");
    }
    println!("contract: flakeLockPresent={}", contract.flake_lock_present);
    if let Some(source) = &contract.source {
        println!("contract: source={source}");
    }
    for component in &contract.components {
        println!(
            "contract: component={} source={} reason={}",
            component.name, component.source, component.reason
        );
    }
    for path in &contract.required_directories {
        println!("contract: requiredDirectory={}", path.name);
    }
    for path in &contract.required_files {
        println!("contract: requiredFile={}", path.name);
    }
    for script in &contract.bootstrap_scripts {
        println!("contract: bootstrapScript={}", script.name);
    }
    ExitCode::SUCCESS
}

fn default_derivation_name(config: Config) -> Option<String> {
    let mut command = nix_command(config);
    command.args([
        "eval",
        "--raw",
        &format!(".#packages.{}.default.name", nix_system()),
    ]);
    command
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_string())
        .filter(|name| !name.is_empty())
}

fn nix_system() -> String {
    match (env::consts::ARCH, env::consts::OS) {
        ("x86_64", "linux") => "x86_64-linux",
        ("aarch64", "linux") => "aarch64-linux",
        ("x86_64", "macos") => "x86_64-darwin",
        ("aarch64", "macos") => "aarch64-darwin",
        (arch, os) => return format!("{arch}-{os}"),
    }
    .to_string()
}

fn flake_robo_nix_source() -> Option<String> {
    let flake = fs::read_to_string("flake.nix").ok()?;
    flake
        .lines()
        .find_map(|line| {
            line.trim()
                .strip_prefix("inputs.robo-nix.url")
                .and_then(quoted_value)
        })
        .map(ToOwned::to_owned)
}
