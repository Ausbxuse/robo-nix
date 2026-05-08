use crate::{Config, LabelKind, inline, label};

pub(super) fn check_field(config: Config, message: &str) {
    if let Some((name, value)) = message.split_once('=') {
        println!("{}={}", label(config, name, LabelKind::Hint), value);
    } else {
        println!("{}", label(config, message, LabelKind::Status));
    }
}

pub(super) fn check_line(config: Config, tag: &str, kind: LabelKind, message: &str) {
    println!("{} {}", label(config, tag, kind), inline(config, message));
}

pub(super) fn check_ok(config: Config, message: &str) {
    check_line(config, "ok:", LabelKind::Ok, message);
}

pub(super) fn check_warn(config: Config, warnings: &mut usize, message: &str) {
    *warnings += 1;
    check_line(config, "warn:", LabelKind::Warn, message);
}

pub(super) fn check_error(config: Config, issues: &mut usize, message: &str) {
    *issues += 1;
    check_line(config, "error:", LabelKind::Error, message);
}

pub(super) fn check_hint(config: Config, message: &str) {
    for line in message.lines() {
        check_line(config, "hint:", LabelKind::Hint, line);
    }
}

pub(super) fn check_why(config: Config, message: &str) {
    check_line(config, "why:", LabelKind::Why, message);
}

pub(super) fn check_why_item(config: Config, message: &str) {
    println!("  {}", inline(config, message));
}

pub(super) fn check_next(config: Config, message: &str) {
    check_line(config, "next:", LabelKind::Status, message);
}

pub(super) fn check_status(
    config: Config,
    status: &str,
    status_kind: LabelKind,
    issues: usize,
    warnings: usize,
) {
    let mut output = format!(
        "{}{}",
        label(config, "status=", LabelKind::Hint),
        label(config, status, status_kind)
    );
    if issues > 0 {
        output.push(' ');
        output.push_str(&label(config, "issues=", LabelKind::Hint));
        output.push_str(&label(config, &issues.to_string(), LabelKind::Error));
    }
    output.push(' ');
    output.push_str(&label(config, "warnings=", LabelKind::Hint));
    output.push_str(&label(config, &warnings.to_string(), LabelKind::Warn));
    println!("{output}");
}
