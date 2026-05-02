use clap::Args;
use serde::Serialize;
use std::env;
use std::fs;
use std::path::Path;
use std::process::ExitCode;

use crate::runtime::{build_runtime_why, read_project_runtime, WhyEntry};
use crate::{
    add_runtime_source_override, error, ensure_project_runtime, label, nix_command, quoted_value,
    Config, LabelKind,
};

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
    cuda_wheel_version: Option<String>,
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
        cuda_wheel_version: runtime.cuda_wheel_version.clone(),
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

    contract_field(config, &format!("env={}", contract.env_name));
    if let Some(version) = &contract.schema_version {
        contract_field(config, &format!("schemaVersion={version}"));
    }
    if let Some(version) = &contract.cuda_wheel_version {
        contract_field(config, &format!("cudaWheelVersion={version}"));
    }
    contract_field(config, &format!("python={}", contract.python_version));
    contract_field(config, &format!("system={}", contract.system));
    contract_field(config, &format!("workspace={}", contract.workspace_root));
    if let Some(name) = &contract.default_derivation {
        contract_field(config, &format!("derivation={name}"));
    }
    contract_field(
        config,
        &format!("flakeLockPresent={}", contract.flake_lock_present),
    );
    if let Some(source) = &contract.source {
        contract_field(config, &format!("source={source}"));
    }
    for component in &contract.components {
        contract_field(
            config,
            &format!(
                "component={} source={} reason={}",
            component.name, component.source, component.reason
            ),
        );
    }
    for path in &contract.required_directories {
        contract_field(config, &format!("requiredDirectory={}", path.name));
    }
    for path in &contract.required_files {
        contract_field(config, &format!("requiredFile={}", path.name));
    }
    for script in &contract.bootstrap_scripts {
        contract_field(config, &format!("bootstrapScript={}", script.name));
    }
    ExitCode::SUCCESS
}

fn contract_field(config: Config, message: &str) {
    println!("{} {message}", label(config, "contract:", LabelKind::Status));
}

fn default_derivation_name(config: Config) -> Option<String> {
    let mut command = nix_command(config);
    command.args(["eval", "--raw"]);
    add_runtime_source_override(&mut command);
    command.arg(format!(".#packages.{}.default.name", nix_system()));
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
