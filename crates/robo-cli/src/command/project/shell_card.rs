use console::measure_text_width;

use crate::{Config, LabelKind, label};

use super::shell_launch::ShellLaunch;

pub(super) fn print_shell_card(config: Config, _launch: &ShellLaunch) {
    println!("{}", label(config, "commands", LabelKind::Status));
    shell_action(config, "uv sync", "sync Python packages from uv.lock");
    shell_action(config, "exit", "leave this runtime shell");
}

fn shell_action(config: Config, command: &str, description: &str) {
    println!(
        "  {} {}",
        pad_display(label(config, command, LabelKind::Command), 9),
        label(config, description, LabelKind::Hint)
    );
}

fn pad_display(value: String, width: usize) -> String {
    let padding = width.saturating_sub(measure_text_width(&value));
    format!("{value}{}", " ".repeat(padding))
}
