use console::style;

#[derive(Clone, Copy)]
pub(crate) struct Config {
    pub(crate) color: bool,
    pub(crate) debug: bool,
}

pub(crate) enum LabelKind {
    Status,
    Ok,
    Warn,
    Error,
    Hint,
    Why,
    Debug,
}

pub(crate) fn label(config: Config, text: &str, kind: LabelKind) -> String {
    if !config.color {
        return text.to_string();
    }

    match kind {
        LabelKind::Status => style(text).cyan().bold().to_string(),
        LabelKind::Ok => style(text).green().bold().to_string(),
        LabelKind::Warn => style(text).yellow().bold().to_string(),
        LabelKind::Error => style(text).red().bold().to_string(),
        LabelKind::Hint => style(text).dim().to_string(),
        LabelKind::Why => style(text).magenta().bold().to_string(),
        LabelKind::Debug => style(text).magenta().bold().to_string(),
    }
}

pub(crate) fn status(config: Config, message: &str) {
    eprintln!("{} {message}", label(config, "robo:", LabelKind::Status));
}

pub(crate) fn ok(config: Config, message: &str) {
    eprintln!("{} {message}", label(config, "ok:", LabelKind::Ok));
}

pub(crate) fn warn(config: Config, message: &str) {
    eprintln!("{} {message}", label(config, "warn:", LabelKind::Warn));
}

pub(crate) fn error(config: Config, message: &str) {
    eprintln!("{} {message}", label(config, "error:", LabelKind::Error));
}

pub(crate) fn hint(config: Config, message: &str) {
    eprintln!("{} {message}", label(config, "hint:", LabelKind::Hint));
}
