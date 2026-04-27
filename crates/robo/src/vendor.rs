use crate::{Config, LabelKind, error, label};
use clap::{Args, Subcommand};
use serde::Deserialize;
use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::path::Path;
use std::process::{Command, ExitCode};

#[derive(Args)]
pub struct VendorArgs {
    #[command(subcommand)]
    command: Option<VendorCommand>,
}

#[derive(Subcommand)]
enum VendorCommand {
    #[command(about = "Inspect a local vendor checkout and show matching curated modules")]
    Add { path: String },

    #[command(about = "List curated local vendor modules")]
    List,

    #[command(about = "Check local vendor source trees and bootstrap wiring")]
    Doctor,

    #[command(about = "Run bootstrap scripts for detected curated vendors")]
    Bootstrap,

    #[command(about = "Print a curated vendor module for upstreaming")]
    Export { name: String },
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Manifest {
    vendor_metadata: BTreeMap<String, VendorModule>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct VendorModule {
    description: String,
    install_path: String,
    detect_paths: Vec<String>,
    source_url: Option<String>,
    components: Vec<String>,
    required_paths: Vec<String>,
    bootstrap_scripts: Vec<String>,
    patches: Vec<String>,
}

pub fn run(args: VendorArgs, config: Config) -> ExitCode {
    let manifest = match load_manifest() {
        Ok(manifest) => manifest,
        Err(message) => {
            error(config, &message);
            return ExitCode::from(1);
        }
    };

    match args.command.unwrap_or(VendorCommand::Bootstrap) {
        VendorCommand::Add { path } => add_vendor(&manifest, &path, config),
        VendorCommand::List => list_vendors(&manifest),
        VendorCommand::Doctor => doctor_vendors(&manifest, config),
        VendorCommand::Bootstrap => bootstrap_vendors(&manifest, config),
        VendorCommand::Export { name } => export_vendor(&manifest, &name, config),
    }
}

fn load_manifest() -> Result<Manifest, String> {
    let path = env::var("ROBO_NIX_COMPONENT_MANIFEST")
        .map_err(|_| "ROBO_NIX_COMPONENT_MANIFEST is not set.".to_string())?;
    let text = fs::read_to_string(&path)
        .map_err(|err| format!("failed to read component manifest {path}: {err}"))?;
    serde_json::from_str(&text).map_err(|err| format!("failed to parse component manifest: {err}"))
}

fn list_vendors(manifest: &Manifest) -> ExitCode {
    for (name, vendor) in &manifest.vendor_metadata {
        let status = if detected_path(vendor).is_some() {
            "present"
        } else {
            "missing"
        };
        println!(
            "{:<34} {:<8} {:<44} {}",
            name, status, vendor.install_path, vendor.description
        );
    }
    ExitCode::SUCCESS
}

fn add_vendor(manifest: &Manifest, path: &str, config: Config) -> ExitCode {
    let vendor_path = Path::new(path);
    if !vendor_path.exists() {
        vendor_error(config, &format!("{path} does not exist"));
        return ExitCode::from(1);
    }

    let Some((name, vendor)) = best_vendor_match(manifest, vendor_path) else {
        vendor_warn(config, &format!("no curated vendor module matched {path}"));
        vendor_hint(config, "keep the source local and add required components manually in robo.nix");
        return ExitCode::SUCCESS;
    };

    vendor_ok(config, &format!("{path} matches curated module {name}"));
    vendor_info(config, &format!("default install path: {}", vendor.install_path));
    match &vendor.source_url {
        Some(url) => vendor_info(config, &format!("install source: {url}")),
        None => vendor_hint(config, "this module has no public sourceUrl; place the checkout locally before bootstrapping"),
    }
    if !vendor.components.is_empty() {
        vendor_hint(
            config,
            &format!("add components to robo.nix: {}", vendor.components.join(",")),
        );
    }
    for required in &vendor.required_paths {
        vendor_info(config, &format!("module expects {required} inside the vendor checkout"));
    }
    ExitCode::SUCCESS
}

fn doctor_vendors(manifest: &Manifest, config: Config) -> ExitCode {
    let mut issues = 0;
    let mut detected = 0;
    for (name, vendor) in &manifest.vendor_metadata {
        let Some(root) = detected_path(vendor) else {
            continue;
        };
        detected += 1;
        vendor_ok(config, &format!("{name} source present at {}", root.display()));

        for path in &vendor.required_paths {
            let full_path = root.join(path);
            if full_path.exists() {
                vendor_ok(config, &format!("{name} has {}", full_path.display()));
            } else {
                issues += 1;
                vendor_error(config, &format!("{name} missing {}", full_path.display()));
            }
        }

        for script in &vendor.bootstrap_scripts {
            if Path::new(script).is_file() {
                vendor_ok(config, &format!("{name} bootstrap script present: {script}"));
            } else {
                issues += 1;
                vendor_error(config, &format!("{name} bootstrap script missing: {script}"));
            }
        }

        for patch in &vendor.patches {
            if Path::new(patch).is_file() {
                vendor_ok(config, &format!("{name} patch present: {patch}"));
            } else {
                issues += 1;
                vendor_error(config, &format!("{name} patch missing: {patch}"));
            }
        }

        if !vendor.components.is_empty() {
            vendor_info(
                config,
                &format!("{name} suggests components: {}", vendor.components.join(",")),
            );
        }
    }

    if detected == 0 {
        vendor_status(config, "ok detected=0", LabelKind::Ok);
        vendor_hint(config, "no curated local vendors were detected under this project root");
        return ExitCode::SUCCESS;
    }

    if issues == 0 {
        vendor_status(config, &format!("ok detected={detected}"), LabelKind::Ok);
        ExitCode::SUCCESS
    } else {
        vendor_hint(config, "run from the project root that contains third_party/");
        vendor_status(
            config,
            &format!("error detected={detected} issues={issues}"),
            LabelKind::Error,
        );
        ExitCode::from(1)
    }
}

fn bootstrap_vendors(manifest: &Manifest, config: Config) -> ExitCode {
    let mut issues = 0;
    let mut ran = 0;

    for (name, vendor) in &manifest.vendor_metadata {
        let root = if let Some(path) = detected_path(vendor) {
            path
        } else if let Some(url) = &vendor.source_url {
            vendor_info(config, &format!("cloning {name} into {}", vendor.install_path));
            if let Some(parent) = Path::new(&vendor.install_path).parent() {
                if let Err(err) = fs::create_dir_all(parent) {
                    issues += 1;
                    vendor_error(config, &format!("failed to create {}: {err}", parent.display()));
                    continue;
                }
            }
            match Command::new("git")
                .args(["clone", "--depth", "1", url, &vendor.install_path])
                .status()
            {
                Ok(status) if status.success() => Path::new(&vendor.install_path).to_path_buf(),
                Ok(status) => {
                    issues += 1;
                    vendor_error(config, &format!("git clone for {name} failed with {status}"));
                    continue;
                }
                Err(err) => {
                    issues += 1;
                    vendor_error(config, &format!("failed to start git clone for {name}: {err}"));
                    continue;
                }
            }
        } else {
            vendor_hint(
                config,
                &format!(
                    "{name} has no sourceUrl; place it at {} or one of: {}",
                    vendor.install_path,
                    vendor.detect_paths.join(",")
                ),
            );
            continue;
        };

        if !root.is_dir() {
            issues += 1;
            vendor_error(config, &format!("{name} source is not a directory: {}", root.display()));
            continue;
        }
        for script in &vendor.bootstrap_scripts {
            if !Path::new(script).is_file() {
                issues += 1;
                vendor_error(config, &format!("{name} bootstrap script missing: {script}"));
                continue;
            }
            vendor_info(config, &format!("running {script} for {name}"));
            let status = Command::new("bash").arg(script).status();
            match status {
                Ok(status) if status.success() => {
                    ran += 1;
                    vendor_ok(config, &format!("{name} bootstrap completed: {script}"));
                }
                Ok(status) => {
                    issues += 1;
                    vendor_error(config, &format!("{name} bootstrap failed with {status}: {script}"));
                }
                Err(err) => {
                    issues += 1;
                    vendor_error(config, &format!("failed to start {script}: {err}"));
                }
            }
        }
    }

    if issues == 0 {
        vendor_status(config, &format!("ok scripts={ran}"), LabelKind::Ok);
        ExitCode::SUCCESS
    } else {
        vendor_status(config, &format!("error scripts={ran} issues={issues}"), LabelKind::Error);
        ExitCode::from(1)
    }
}

fn export_vendor(manifest: &Manifest, name: &str, config: Config) -> ExitCode {
    let Some(vendor) = manifest.vendor_metadata.get(name) else {
        vendor_error(config, &format!("unknown vendor module {name}"));
        return ExitCode::from(1);
    };

    println!("{name} = {{");
    println!("  description = \"{}\";", escape_nix(&vendor.description));
    println!("  installPath = \"{}\";", escape_nix(&vendor.install_path));
    print_list("detectPaths", &vendor.detect_paths);
    match &vendor.source_url {
        Some(url) => println!("  sourceUrl = \"{}\";", escape_nix(url)),
        None => println!("  sourceUrl = null;"),
    }
    print_list("components", &vendor.components);
    print_list("requiredPaths", &vendor.required_paths);
    print_list("bootstrapScripts", &vendor.bootstrap_scripts);
    print_list("patches", &vendor.patches);
    println!("}};");
    ExitCode::SUCCESS
}

fn best_vendor_match<'a>(
    manifest: &'a Manifest,
    vendor_path: &Path,
) -> Option<(&'a str, &'a VendorModule)> {
    let canonical_name = vendor_path.file_name()?.to_string_lossy().to_ascii_lowercase();
    manifest
        .vendor_metadata
        .iter()
        .find(|(_, vendor)| {
            vendor
                .detect_paths
                .iter()
                .chain(std::iter::once(&vendor.install_path))
                .any(|path| {
                    Path::new(path)
                        .file_name()
                        .is_some_and(|name| name.to_string_lossy().eq_ignore_ascii_case(&canonical_name))
                })
        })
        .map(|(name, vendor)| (name.as_str(), vendor))
}

fn detected_path(vendor: &VendorModule) -> Option<std::path::PathBuf> {
    vendor
        .detect_paths
        .iter()
        .chain(std::iter::once(&vendor.install_path))
        .map(Path::new)
        .find(|path| path.is_dir())
        .map(Path::to_path_buf)
}

fn print_list(name: &str, values: &[String]) {
    println!("  {name} = [");
    for value in values {
        println!("    \"{}\"", escape_nix(value));
    }
    println!("  ];");
}

fn escape_nix(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

fn vendor_ok(config: Config, message: &str) {
    vendor_line(config, "ok:", LabelKind::Ok, message);
}

fn vendor_warn(config: Config, message: &str) {
    vendor_line(config, "warn:", LabelKind::Warn, message);
}

fn vendor_error(config: Config, message: &str) {
    vendor_line(config, "error:", LabelKind::Error, message);
}

fn vendor_info(config: Config, message: &str) {
    vendor_line(config, "info:", LabelKind::Status, message);
}

fn vendor_hint(config: Config, message: &str) {
    vendor_line(config, "hint:", LabelKind::Hint, message);
}

fn vendor_status(config: Config, message: &str, kind: LabelKind) {
    println!(
        "{} {}{message}",
        label(config, "vendor:", LabelKind::Status),
        label(config, "status=", kind)
    );
}

fn vendor_line(config: Config, tag: &str, kind: LabelKind, message: &str) {
    println!(
        "{} {} {message}",
        label(config, "vendor:", LabelKind::Status),
        label(config, tag, kind)
    );
}
