use std::io::{self, IsTerminal, Write};
use std::path::PathBuf;
use std::process::ExitCode;

use super::manifest::{list_profiles, profile_names, resolve_profile_selection, Manifest};
use super::InitArgs;
use crate::{error, Config};

pub(super) fn run(args: &mut InitArgs, manifest: &Manifest, config: Config) -> Result<(), ExitCode> {
    eprintln!(
        "{} init",
        crate::label(config, "robo", crate::LabelKind::Status)
    );
    let advanced = ask("Advanced component selection?", "no", config)?;
    if matches!(advanced.as_str(), "yes" | "y") {
        list_profiles(manifest);
        let profile = ask("Profile", &manifest.runtime_inference.default_profile, config)?;
        args.profile = Some(profile);
    } else {
        let profiles = profile_names(manifest);
        eprintln!("{}", crate::label(config, "project setup", crate::LabelKind::Status));
        for (index, profile) in profiles.iter().enumerate() {
            eprintln!("  {}. {}", index + 1, profile);
        }
        let setup = ask("Selection", "1", config)?;
        args.profile = Some(resolve_profile_selection(&profiles, &setup).ok_or_else(|| {
            error(config, &format!("unknown setup selection: {setup}"));
            ExitCode::from(1)
        })?);
    }

    if args.target.is_none() {
        let target = ask("Target directory", ".", config)?;
        args.target = Some(PathBuf::from(target));
    }
    if io::stdin().is_terminal() && args.name.is_none() {
        let env_name = ask("Environment name", "project", config)?;
        if !env_name.is_empty() {
            args.name = Some(env_name);
        }
    }
    let proceed = ask("Write runtime files?", "yes", config)?;
    if !matches!(proceed.as_str(), "yes" | "y") {
        return Err(ExitCode::SUCCESS);
    }
    Ok(())
}

fn ask(prompt: &str, default: &str, config: Config) -> Result<String, ExitCode> {
    if !io::stdin().is_terminal() {
        return Ok(default.to_string());
    }
    eprint!("{prompt} [{default}]: ");
    let _ = io::stderr().flush();
    let mut line = String::new();
    if let Err(err) = io::stdin().read_line(&mut line) {
        error(config, &format!("failed to read input: {err}"));
        return Err(ExitCode::from(1));
    }
    let value = line.trim();
    if value.is_empty() {
        Ok(default.to_string())
    } else {
        Ok(value.to_string())
    }
}
