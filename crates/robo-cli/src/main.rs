mod check;
mod cli;
mod command;
mod contract;
mod cuda;
mod init;
mod pyproject;
mod runtime;
mod ui;

mod shell {
    use std::env;
    use std::ffi::OsString;
    use std::path::Path;

    pub(crate) const SUPPORTED_INTERACTIVE_SHELLS: &str = "bash, zsh, fish";

    pub(crate) fn requested_shell_name(
        shell: Option<&OsString>,
        command_name: &str,
    ) -> Result<String, String> {
        let shell = match shell {
            Some(shell) => shell.to_string_lossy().into_owned(),
            None => env::var("SHELL")
                .map_err(|_| format!("{command_name} needs a shell name when SHELL is unknown."))?,
        };

        Ok(shell_basename(&shell))
    }

    pub(crate) fn supports_interactive_shell(shell: &str) -> bool {
        matches!(shell, "bash" | "zsh" | "fish")
    }

    fn shell_basename(shell: &str) -> String {
        Path::new(shell)
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or(shell)
            .to_string()
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn requested_shell_name_accepts_shell_path() {
            let shell = requested_shell_name(Some(&OsString::from("/usr/bin/zsh")), "robo hook")
                .expect("shell path should parse");

            assert_eq!(shell, "zsh");
        }

        #[test]
        fn supported_shells_are_the_interactive_hook_shells() {
            for shell in ["bash", "zsh", "fish"] {
                assert!(supports_interactive_shell(shell));
            }
            assert!(!supports_interactive_shell("nu"));
        }
    }
}

pub(crate) use command::{
    combined_output, command_for_runtime, ensure_project_runtime, nix_command, quoted_value,
    run_bootstrap_with_progress,
};
pub(crate) use pyproject::exact_python_requirement;
pub(crate) use ui::{
    Config, LabelKind, UiProgress, UiSpinner, command_row_err, error, field, field_err, hint,
    inline, label, ok, output_with_spinner, section, section_err, status, warn,
};

fn main() -> std::process::ExitCode {
    cli::run()
}
