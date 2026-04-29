mod check;
mod cli;
mod command;
mod contract;
mod cuda;
mod init;
mod runtime;
mod ui;

pub(crate) use command::{
    command_for_runtime, combined_output, ensure_project_runtime, exact_python_requirement,
    nix_command, quoted_value, run_bootstrap,
};
pub(crate) use ui::{error, hint, label, ok, status, warn, Config, LabelKind};

fn main() -> std::process::ExitCode {
    cli::run()
}
