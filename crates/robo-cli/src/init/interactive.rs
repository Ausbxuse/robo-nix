use std::collections::BTreeSet;
use std::io::{self, IsTerminal, Write};
use std::path::PathBuf;
use std::process::ExitCode;

use super::manifest::{list_profiles, profile_names, resolve_profile_selection, Manifest};
use super::spec::ProjectSpec;
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
        eprintln!("Project setup:");
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

pub(super) fn apply_component_suggestions(spec: &mut ProjectSpec, config: Config) {
    apply_bootstrap_suggestions(spec, config);

    let suggestions = spec.component_suggestions.clone();
    let mut accepted = BTreeSet::new();
    for suggestion in suggestions {
        let answer = match ask(
            &format!("Add component {}? {}", suggestion.name, suggestion.reason),
            "yes",
            config,
        ) {
            Ok(answer) => answer,
            Err(_) => "no".to_string(),
        };
        if matches!(answer.as_str(), "yes" | "y") {
            spec.add_component_with_source(
                &suggestion.name,
                "interactive workspace inference",
                format!("{}: {}", suggestion.reason, suggestion.evidence),
            );
            accepted.insert(suggestion.name);
        }
    }
    spec.component_suggestions
        .retain(|suggestion| !accepted.contains(&suggestion.name));
}

fn apply_bootstrap_suggestions(spec: &mut ProjectSpec, config: Config) {
    let suggestions = spec.suggestions.clone();
    let mut accepted = BTreeSet::new();
    for suggestion in suggestions {
        if suggestion.kind != "bootstrap" {
            continue;
        }
        let answer = match ask(
            &format!("Enable bootstrap script {}? {}", suggestion.path, suggestion.reason),
            "no",
            config,
        ) {
            Ok(answer) => answer,
            Err(_) => "no".to_string(),
        };
        if matches!(answer.as_str(), "yes" | "y") {
            spec.add_source_script(&suggestion.path);
            accepted.insert(suggestion.path);
        }
    }
    spec.suggestions
        .retain(|suggestion| suggestion.kind != "bootstrap" || !accepted.contains(&suggestion.path));
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
