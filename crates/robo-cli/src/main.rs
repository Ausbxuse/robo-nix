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
    nix_command, quoted_value, run_bootstrap_with_progress,
};
pub(crate) use ui::{
    command_row, command_row_err, error, field, field_err, hint, inline, label, ok,
    output_with_spinner, section, section_err, status, warn, Config, LabelKind, UiProgress,
};

fn main() -> std::process::ExitCode {
    cli::run()
}
