use std::env;

use console::measure_text_width;

use crate::{Config, LabelKind, label};

use super::shell_launch::{ShellLaunch, shell_launch_label};
use super::{RuntimeState, nix_system_name};

pub(super) fn print_shell_card(config: Config, launch: &ShellLaunch) {
    let state = RuntimeState::read();
    let system = nix_system_name();
    let workspace = shorten_middle(&home_tilde(&state.workspace), 62);
    let shell = shell_launch_label(launch);

    let rows = [
        (
            format!("{} runtime", state.env_name),
            label(
                config,
                &format!("{} runtime", state.env_name),
                LabelKind::Status,
            ),
        ),
        card_field_pair(config, "python", &state.python_version, "system", system),
        card_field(config, "path", &workspace),
        card_field(config, "shell", &shell),
        (String::new(), String::new()),
        (
            "commands".to_string(),
            label(config, "commands", LabelKind::Hint),
        ),
        card_action(config, "uv sync", "sync Python packages from uv.lock"),
        card_action(config, "exit", "leave this runtime shell"),
    ];
    let row_width = rows
        .iter()
        .map(|(plain, _)| measure_text_width(plain))
        .max()
        .unwrap_or(0);
    let inner_width = row_width + 2;
    let (top_left, horizontal, top_right, vertical, bottom_left, bottom_right) = if config.color {
        ("╭", "─", "╮", "│", "╰", "╯")
    } else {
        ("+", "-", "+", "|", "+", "+")
    };

    println!(
        "{}{}{}",
        label(config, top_left, LabelKind::Status),
        label(config, &horizontal.repeat(inner_width), LabelKind::Status),
        label(config, top_right, LabelKind::Status)
    );
    for (plain, rendered) in rows {
        let plain_len = measure_text_width(&plain);
        let padding = " ".repeat(row_width.saturating_sub(plain_len));
        println!(
            "{} {}{} {}",
            label(config, vertical, LabelKind::Status),
            rendered,
            padding,
            label(config, vertical, LabelKind::Status),
        );
    }
    println!(
        "{}{}{}",
        label(config, bottom_left, LabelKind::Status),
        label(config, &horizontal.repeat(inner_width), LabelKind::Status),
        label(config, bottom_right, LabelKind::Status),
    );
}

fn card_field(config: Config, name: &str, value: &str) -> (String, String) {
    (
        format!("{name:<7} {value}"),
        format!(
            "{} {}",
            label(config, &format!("{name:<7}"), LabelKind::Hint),
            label(config, value, LabelKind::Status)
        ),
    )
}

fn card_field_pair(
    config: Config,
    left_name: &str,
    left_value: &str,
    right_name: &str,
    right_value: &str,
) -> (String, String) {
    (
        format!("{left_name:<7} {left_value:<8}  {right_name} {right_value}"),
        format!(
            "{} {}  {} {}",
            label(config, &format!("{left_name:<7}"), LabelKind::Hint),
            label(config, &format!("{left_value:<8}"), LabelKind::Status),
            label(config, right_name, LabelKind::Hint),
            label(config, right_value, LabelKind::Status)
        ),
    )
}

fn card_action(config: Config, command: &str, description: &str) -> (String, String) {
    (
        format!("  {command:<9} {description}"),
        format!(
            "  {} {}",
            label(config, &format!("{command:<9}"), LabelKind::Command),
            label(config, description, LabelKind::Hint)
        ),
    )
}

fn home_tilde(value: &str) -> String {
    let Ok(home) = env::var("HOME") else {
        return value.to_string();
    };
    if value == home {
        return "~".to_string();
    }
    value
        .strip_prefix(&format!("{home}/"))
        .map_or_else(|| value.to_string(), |rest| format!("~/{rest}"))
}

fn shorten_middle(value: &str, max_len: usize) -> String {
    let len = value.chars().count();
    if len <= max_len {
        return value.to_string();
    }

    let keep = max_len.saturating_sub(3);
    let tail: String = value.chars().skip(len.saturating_sub(keep)).collect();
    format!("...{tail}")
}
