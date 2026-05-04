use std::env;
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct ShellLaunch {
    pub(crate) args: Vec<OsString>,
    pub(crate) env: Vec<(String, OsString)>,
}

impl ShellLaunch {
    fn args(args: Vec<OsString>) -> Self {
        Self { args, env: vec![] }
    }
}

pub(crate) fn normalize_shell_args(mut args: Vec<OsString>) -> ShellLaunch {
    if args.is_empty() {
        return default_interactive_shell_args();
    }

    if args.len() != 2 || args[0] != "-c" {
        return ShellLaunch::args(args);
    }

    if !args[1].to_string_lossy().chars().any(char::is_whitespace) {
        return ShellLaunch::args(args);
    }

    let mut normalized = Vec::with_capacity(4);
    normalized.push(OsString::from("-c"));
    normalized.push(OsString::from("bash"));
    normalized.push(OsString::from("-lc"));
    normalized.push(args.swap_remove(1));
    ShellLaunch::args(normalized)
}

pub(crate) fn shell_launch_label(launch: &ShellLaunch) -> String {
    let mut args = launch.args.iter();
    if args.next().is_some_and(|arg| arg == "-c") {
        let Some(shell) = args.next() else {
            return "unknown".to_string();
        };
        let shell_name = Path::new(shell)
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_else(|| shell.to_str().unwrap_or("unknown"));
        shell_name.to_string()
    } else {
        launch
            .args
            .first()
            .and_then(|arg| Path::new(arg).file_name())
            .and_then(|name| name.to_str())
            .unwrap_or("unknown")
            .to_string()
    }
}

pub(crate) fn command_from_launch_args(args: Vec<OsString>) -> Result<Command, String> {
    let mut args = args.into_iter();
    let Some(first) = args.next() else {
        return Err("could not determine an interactive shell to launch.".to_string());
    };

    let (program, args): (OsString, Vec<_>) = if first == "-c" {
        let Some(program) = args.next() else {
            return Err("shell command is missing a program after -c.".to_string());
        };
        (program, args.collect())
    } else {
        (first, args.collect())
    };

    let mut command = Command::new(program);
    command.args(args);
    Ok(command)
}

fn default_interactive_shell_args() -> ShellLaunch {
    let Some(shell) = default_interactive_shell() else {
        return ShellLaunch::args(vec![]);
    };
    shell_args_for(shell.to_string_lossy().as_ref())
}

fn default_interactive_shell() -> Option<PathBuf> {
    select_default_interactive_shell(
        env::var_os("ROBO_NIX_SHELL").map(PathBuf::from),
        env::var_os("SHELL").map(PathBuf::from),
        parent_interactive_shell(),
        login_shell(),
        find_shell_in_path,
    )
}

fn select_default_interactive_shell(
    robo_nix_shell: Option<PathBuf>,
    shell_env: Option<PathBuf>,
    parent_shell: Option<PathBuf>,
    login_shell: Option<PathBuf>,
    find_in_path: impl Fn(&str) -> Option<PathBuf>,
) -> Option<PathBuf> {
    let resolve = |shell| resolve_shell_path_with(shell, &find_in_path);

    if let Some(shell) = robo_nix_shell.and_then(resolve) {
        return Some(shell);
    }

    let shell_env = shell_env.and_then(resolve);
    if let Some(shell) = shell_env.as_deref() {
        if is_nix_bash(shell) {
            return login_shell
                .and_then(resolve)
                .filter(|shell| !is_generic_sh(shell))
                .or_else(|| parent_shell.clone().filter(|shell| !is_generic_sh(shell)))
                .or_else(|| shell_env.clone());
        }
        if !is_generic_sh(shell) {
            return shell_env;
        }
    }

    if let Some(shell) = login_shell
        .and_then(resolve)
        .filter(|shell| !is_generic_sh(shell))
    {
        return Some(shell);
    }

    if let Some(shell) = parent_shell.filter(|shell| !is_generic_sh(shell)) {
        return Some(shell);
    }

    find_in_path("zsh")
        .or_else(|| find_in_path("bash"))
        .or_else(|| find_in_path("fish"))
        .or(shell_env)
        .or_else(|| find_in_path("sh"))
}

fn resolve_shell_path(shell: PathBuf) -> Option<PathBuf> {
    resolve_shell_path_with(shell, &find_shell_in_path)
}

fn resolve_shell_path_with(
    shell: PathBuf,
    find_in_path: &impl Fn(&str) -> Option<PathBuf>,
) -> Option<PathBuf> {
    if shell.is_file() {
        return Some(shell);
    }
    shell
        .file_name()
        .and_then(|name| name.to_str())
        .and_then(find_in_path)
}

fn find_shell_in_path(name: &str) -> Option<PathBuf> {
    if name.contains('/') {
        return None;
    }
    env::var_os("PATH").and_then(|paths| {
        env::split_paths(&paths)
            .map(|dir| dir.join(name))
            .find(|candidate| candidate.is_file())
    })
}

fn parent_interactive_shell() -> Option<PathBuf> {
    let stat = fs::read_to_string("/proc/self/stat").ok()?;
    let after_comm = stat.rsplit_once(") ")?.1;
    let ppid = after_comm.split_whitespace().nth(1)?;
    let shell = fs::read_link(format!("/proc/{ppid}/exe")).ok()?;
    shell.is_file().then_some(shell)
}

fn is_nix_bash(shell: &Path) -> bool {
    shell.to_string_lossy().contains("/nix/store/")
        && shell.file_name().is_some_and(|name| name == "bash")
}

fn is_generic_sh(shell: &Path) -> bool {
    shell
        .file_name()
        .is_some_and(|name| name == "sh" || name == "dash")
}

fn login_shell() -> Option<PathBuf> {
    let user = env::var("USER").ok()?;
    let passwd = fs::read_to_string("/etc/passwd").ok()?;
    passwd.lines().find_map(|line| {
        let mut fields = line.split(':');
        if fields.next()? != user {
            return None;
        }
        fields.nth(5).map(PathBuf::from)
    })
}

fn shell_args_for(shell: &str) -> ShellLaunch {
    let Some(shell) = resolve_shell_path(PathBuf::from(shell)) else {
        return ShellLaunch::args(vec![]);
    };
    let shell_name = shell
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default();
    let interactive_args = clean_interactive_shell_args(shell_name);

    ShellLaunch {
        args: std::iter::once(OsString::from("-c"))
            .chain(std::iter::once(shell.clone().into_os_string()))
            .chain(interactive_args.into_iter().map(OsString::from))
            .collect(),
        env: vec![("SHELL".to_string(), shell.clone().into_os_string())],
    }
}

fn clean_interactive_shell_args(shell_name: &str) -> Vec<&'static str> {
    match shell_name {
        "bash" | "zsh" | "fish" => vec!["-i"],
        _ => vec!["-i"],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shell_command_with_single_quoted_string_uses_shell() {
        let args = vec![OsString::from("-c"), OsString::from("python test.py")];
        let normalized = normalize_shell_args(args);
        let values: Vec<_> = normalized
            .args
            .iter()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect();

        assert_eq!(values, vec!["-c", "bash", "-lc", "python test.py"]);
    }

    #[test]
    fn shell_command_without_args_uses_user_shell() {
        let normalized = shell_args_for("/bin/sh");
        let values: Vec<_> = normalized
            .args
            .iter()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect();

        assert_eq!(values, vec!["-c", "/bin/sh", "-i"]);
        assert_eq!(
            normalized.env,
            vec![("SHELL".to_string(), OsString::from("/bin/sh"))]
        );
    }

    #[test]
    fn default_shell_loads_user_startup_files() {
        assert_eq!(clean_interactive_shell_args("zsh"), vec!["-i"]);
        assert_eq!(clean_interactive_shell_args("bash"), vec!["-i"]);
        assert_eq!(clean_interactive_shell_args("fish"), vec!["-i"]);
    }

    #[test]
    fn shell_card_labels_shell_name() {
        let launch = ShellLaunch::args(vec![
            OsString::from("-c"),
            OsString::from("/usr/bin/zsh"),
            OsString::from("-i"),
        ]);

        assert_eq!(shell_launch_label(&launch), "zsh");
    }

    #[test]
    fn generic_sh_is_not_treated_as_user_default_shell() {
        assert!(is_generic_sh(Path::new("/bin/sh")));
        assert!(is_generic_sh(Path::new("/usr/bin/dash")));
        assert!(!is_generic_sh(Path::new("/bin/zsh")));
        assert!(!is_generic_sh(Path::new("/bin/bash")));
    }

    #[test]
    fn generic_shell_env_defers_to_parent_zsh() {
        let selected = select_default_interactive_shell(
            None,
            Some(PathBuf::from("/bin/sh")),
            Some(PathBuf::from("/usr/bin/zsh")),
            Some(PathBuf::from("/bin/sh")),
            |_| None,
        );

        assert_eq!(selected, Some(PathBuf::from("/usr/bin/zsh")));
    }

    #[test]
    fn nix_bash_with_generic_login_shell_defers_to_parent_zsh() {
        let selected = select_default_interactive_shell(
            None,
            Some(PathBuf::from("/nix/store/abc-bash-5.3/bin/bash")),
            Some(PathBuf::from("/usr/bin/zsh")),
            Some(PathBuf::from("/bin/sh")),
            |name| {
                (name == "bash").then(|| PathBuf::from("/nix/store/abc-bash-5.3/bin/bash"))
            },
        );

        assert_eq!(selected, Some(PathBuf::from("/usr/bin/zsh")));
    }

    #[test]
    fn shell_command_uses_program_after_develop_command_flag() {
        let command = command_from_launch_args(vec![
            OsString::from("-c"),
            OsString::from("/bin/sh"),
            OsString::from("-i"),
        ])
        .expect("shell command should parse");

        assert_eq!(command.get_program(), "/bin/sh");
        assert_eq!(
            command
                .get_args()
                .map(|arg| arg.to_string_lossy().into_owned())
                .collect::<Vec<_>>(),
            vec!["-i"]
        );
    }

    #[test]
    fn shell_command_keeps_split_argv_intact() {
        let args = vec![
            OsString::from("-c"),
            OsString::from("python"),
            OsString::from("test.py"),
        ];
        let normalized = normalize_shell_args(args.clone());
        let values: Vec<_> = normalized
            .args
            .iter()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect();

        assert_eq!(values, vec!["-c", "python", "test.py"]);
        assert_eq!(normalized, ShellLaunch::args(args));
    }
}
