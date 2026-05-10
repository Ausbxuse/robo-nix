use std::env;
use std::ffi::OsString;
use std::fs;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

const BASHRC_TEMPLATE: &str = include_str!("templates/shell/bashrc");
const ZSHENV_TEMPLATE: &str = include_str!("templates/shell/zshenv");
const ZSHRC_TEMPLATE: &str = include_str!("templates/shell/zshrc");
const FISH_INIT: &str = include_str!("templates/shell/fish-init");

#[derive(Debug)]
pub(crate) struct ShellLaunch {
    pub(crate) program: OsString,
    pub(crate) args: Vec<OsString>,
    pub(crate) env: Vec<(String, OsString)>,
    pub(crate) name: String,
}

pub(crate) fn interactive_shell_launch() -> Option<ShellLaunch> {
    let shell = default_interactive_shell()?;
    let name = shell_name(&shell);
    let (args, mut env) = prompted_interactive_shell_args(&name)
        .unwrap_or_else(|| (vec![OsString::from("-i")], prompt_env()));
    env.push(("SHELL".to_string(), shell.clone().into_os_string()));

    Some(ShellLaunch {
        program: shell.into_os_string(),
        args,
        env,
        name,
    })
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

fn prompted_interactive_shell_args(
    shell_name: &str,
) -> Option<(Vec<OsString>, Vec<(String, OsString)>)> {
    match shell_name {
        "bash" => prompted_bash_args(),
        "zsh" => prompted_zsh_args(),
        "fish" => Some(prompted_fish_args()),
        _ => None,
    }
}

fn prompted_bash_args() -> Option<(Vec<OsString>, Vec<(String, OsString)>)> {
    let path = prompt_startup_dir()?.join("bashrc");
    fs::write(&path, BASHRC_TEMPLATE).ok()?;
    Some((
        vec![
            OsString::from("--rcfile"),
            path.into_os_string(),
            OsString::from("-i"),
        ],
        prompt_env(),
    ))
}

fn prompted_zsh_args() -> Option<(Vec<OsString>, Vec<(String, OsString)>)> {
    let dir = prompt_startup_dir()?.join("zsh");
    fs::create_dir_all(&dir).ok()?;
    fs::write(
        dir.join(".zshenv"),
        ZSHENV_TEMPLATE.replace("{{zdotdir}}", &shell_quote(&dir.display().to_string())),
    )
    .ok()?;
    fs::write(dir.join(".zshrc"), ZSHRC_TEMPLATE).ok()?;
    Some((
        vec![OsString::from("-i")],
        vec![
            ("ZDOTDIR".to_string(), dir.into_os_string()),
            (
                "ROBO_NIX_PARENT_ZDOTDIR".to_string(),
                env::var_os("ZDOTDIR").unwrap_or_else(|| {
                    env::var_os("HOME")
                        .map(PathBuf::from)
                        .unwrap_or_else(|| PathBuf::from("."))
                        .into_os_string()
                }),
            ),
            ("ROBO_NIX_PROMPT_PREFIX".to_string(), OsString::from("1")),
        ],
    ))
}

fn prompted_fish_args() -> (Vec<OsString>, Vec<(String, OsString)>) {
    (
        vec![
            OsString::from("--init-command"),
            OsString::from(FISH_INIT),
            OsString::from("-i"),
        ],
        prompt_env(),
    )
}

fn prompt_env() -> Vec<(String, OsString)> {
    vec![("ROBO_NIX_PROMPT_PREFIX".to_string(), OsString::from("1"))]
}

fn prompt_startup_dir() -> Option<PathBuf> {
    let dir = PathBuf::from(".robo-nix").join("shell-startup");
    fs::create_dir_all(&dir).ok()?;
    Some(dir)
}

fn resolve_shell_path_with(
    shell: PathBuf,
    find_in_path: &impl Fn(&str) -> Option<PathBuf>,
) -> Option<PathBuf> {
    if is_executable_file(&shell) {
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
            .find(|candidate| is_executable_file(candidate))
    })
}

fn is_executable_file(path: &Path) -> bool {
    if !path.is_file() {
        return false;
    }
    #[cfg(unix)]
    {
        fs::metadata(path)
            .map(|metadata| metadata.permissions().mode() & 0o111 != 0)
            .unwrap_or(false)
    }
    #[cfg(not(unix))]
    {
        true
    }
}

fn parent_interactive_shell() -> Option<PathBuf> {
    let stat = fs::read_to_string("/proc/self/stat").ok()?;
    let after_comm = stat.rsplit_once(") ")?.1;
    let ppid = after_comm.split_whitespace().nth(1)?;
    let shell = fs::read_link(format!("/proc/{ppid}/exe")).ok()?;
    is_executable_file(&shell).then_some(shell)
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

fn is_nix_bash(shell: &Path) -> bool {
    shell.to_string_lossy().contains("/nix/store/")
        && shell.file_name().is_some_and(|name| name == "bash")
}

fn is_generic_sh(shell: &Path) -> bool {
    shell
        .file_name()
        .is_some_and(|name| name == "sh" || name == "dash")
}

fn shell_name(shell: &Path) -> String {
    shell
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("shell")
        .to_string()
}

fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicit_robo_shell_wins() {
        let selected = select_default_interactive_shell(
            Some(PathBuf::from("/bin/zsh")),
            Some(PathBuf::from("/bin/bash")),
            None,
            None,
            fake_shell,
        )
        .unwrap();

        assert_eq!(selected, PathBuf::from("/bin/zsh"));
    }

    #[test]
    fn nix_bash_prefers_login_shell() {
        let selected = select_default_interactive_shell(
            None,
            Some(PathBuf::from("bash")),
            None,
            Some(PathBuf::from("/bin/zsh")),
            fake_shell_with_nix_bash,
        )
        .unwrap();

        assert_eq!(selected, PathBuf::from("/bin/zsh"));
    }

    #[test]
    fn generic_sh_falls_back_to_zsh() {
        let selected = select_default_interactive_shell(
            None,
            Some(PathBuf::from("/bin/sh")),
            None,
            None,
            fake_shell,
        )
        .unwrap();

        assert_eq!(selected, PathBuf::from("/bin/zsh"));
    }

    #[test]
    fn prompt_env_enables_robo_prompt_prefix_only() {
        let env = prompt_env();

        assert_eq!(
            env,
            vec![("ROBO_NIX_PROMPT_PREFIX".to_string(), OsString::from("1"))]
        );
    }

    fn fake_shell(name: &str) -> Option<PathBuf> {
        match name {
            "zsh" => Some(PathBuf::from("/bin/zsh")),
            "bash" => Some(PathBuf::from("/bin/bash")),
            "fish" => Some(PathBuf::from("/bin/fish")),
            "sh" => Some(PathBuf::from("/bin/sh")),
            _ => None,
        }
    }

    fn fake_shell_with_nix_bash(name: &str) -> Option<PathBuf> {
        match name {
            "bash" => Some(PathBuf::from("/nix/store/test-bash/bin/bash")),
            other => fake_shell(other),
        }
    }
}
