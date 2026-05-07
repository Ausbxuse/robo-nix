use crate::{error, Config};
use clap::Args;
use std::path::PathBuf;
use std::process::ExitCode;

mod interactive;
mod manifest;
mod pipeline;
mod probe;
mod render;
mod spec;

use manifest::{list_components, list_profiles, load_manifest};
use pipeline::{build_draft, finish_plan};
use render::write_project;

#[derive(Args)]
pub struct InitArgs {
    #[arg(help = "Project directory to initialize")]
    pub target: Option<PathBuf>,

    #[arg(long, help = "Run the guided initializer")]
    pub interactive: bool,

    #[arg(long, help = "List recommended starter profiles")]
    pub list_profiles: bool,

    #[arg(long, help = "List reusable runtime components")]
    pub list_components: bool,

    #[arg(long, help = "Print generated flake.nix instead of writing files")]
    pub stdout: bool,

    #[arg(long, help = "Overwrite generated flake.nix")]
    pub force: bool,

    #[arg(long, help = "Build the runtime after writing project files")]
    pub build: bool,

    #[arg(long, value_name = "NAME", help = "Environment name")]
    pub name: Option<String>,

    #[arg(long, value_name = "NAME", help = "Apply a recommended profile")]
    pub profile: Option<String>,

    #[arg(long = "with", value_name = "LIST", help = "Add comma-separated components")]
    pub with_components: Option<String>,

    #[arg(long, help = "Disable pyproject/workspace runtime probing")]
    pub no_probe: bool,

    #[arg(long, value_name = "TEXT", help = "Environment description")]
    pub description: Option<String>,

    #[arg(long, value_name = "PATH", default_value = ".", help = "Workspace root inside the project")]
    pub workspace_root: String,

    #[arg(long, value_name = "LIST", help = "Comma-separated component names")]
    pub components: Option<String>,

    #[arg(long, value_name = "VERSION", help = "Python version for the Nix interpreter and uv project files")]
    pub python_version: Option<String>,

    #[arg(long, value_name = "LIST", help = "Comma-separated Nix systems")]
    pub systems: Option<String>,

    #[arg(long, value_name = "PATH", help = "Require a project-owned directory")]
    pub required_dir: Vec<String>,

    #[arg(long, value_name = "PATH", help = "Require a project-owned file")]
    pub required_file: Vec<String>,

    #[arg(long, value_name = "PATH", help = "Source a project-owned bootstrap script")]
    pub source_script: Vec<String>,

    #[arg(long, value_name = "NAME=VALUE", help = "Export a project runtime variable")]
    pub env: Vec<String>,

    #[arg(long, value_name = "URL", help = "robo-nix input URL to embed in flake.nix")]
    pub robo_nix_url: Option<String>,
}

impl InitArgs {
    pub(crate) fn generated(target: PathBuf, interactive: bool, force: bool) -> Self {
        Self {
            target: Some(target),
            interactive,
            list_profiles: false,
            list_components: false,
            stdout: false,
            force,
            build: false,
            name: None,
            profile: None,
            with_components: None,
            no_probe: false,
            description: None,
            workspace_root: ".".to_string(),
            components: None,
            python_version: None,
            systems: None,
            required_dir: Vec::new(),
            required_file: Vec::new(),
            source_script: Vec::new(),
            env: Vec::new(),
            robo_nix_url: None,
        }
    }
}

pub fn run(args: InitArgs, config: Config) -> ExitCode {
    run_inner(args, config, false)
}

pub(crate) fn run_quiet(args: InitArgs, config: Config) -> ExitCode {
    run_inner(args, config, true)
}

fn run_inner(args: InitArgs, config: Config, quiet: bool) -> ExitCode {
    let manifest = match load_manifest() {
        Ok(manifest) => manifest,
        Err(message) => {
            error(config, &message);
            return ExitCode::from(1);
        }
    };

    if args.list_profiles {
        list_profiles(&manifest);
        return ExitCode::SUCCESS;
    }
    if args.list_components {
        list_components(&manifest);
        return ExitCode::SUCCESS;
    }

    let mut args = args;
    if args.interactive {
        if let Err(code) = interactive::run(&mut args, &manifest, config) {
            return code;
        }
    }

    let mut draft = match build_draft(&args, &manifest) {
        Ok(draft) => draft,
        Err(message) => {
            error(config, &message);
            return ExitCode::from(1);
        }
    };
    if args.interactive {
        interactive::apply_component_suggestions(&mut draft.spec, config);
    }
    let plan = match finish_plan(&args, &manifest, draft) {
        Ok(plan) => plan,
        Err(message) => {
            error(config, &message);
            return ExitCode::from(1);
        }
    };

    if args.stdout {
        if args.build {
            error(config, "--build cannot be used with --stdout.");
            return ExitCode::from(2);
        }
        println!("{}", plan.flake);
        return ExitCode::SUCCESS;
    }
    match write_project(
        &manifest,
        &plan.target_dir,
        &plan.flake,
        &plan.project,
        &plan.spec,
        args.force,
        &plan.source_url,
        config,
        quiet,
    ) {
        Ok(()) => {
            if args.build {
                crate::command::run_project_build(plan.target_dir, config)
            } else {
                ExitCode::SUCCESS
            }
        }
        Err(code) => code,
    }
}
