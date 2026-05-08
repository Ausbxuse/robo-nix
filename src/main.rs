use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::ffi::OsString;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};
use std::time::{SystemTime, UNIX_EPOCH};

const HELP: &str = include_str!("../templates/help.txt");
const PROJECT_FLAKE_TEMPLATE: &str = include_str!("../templates/project/flake.nix");
const PROJECT_ROBO_TEMPLATE: &str = include_str!("../templates/project/robo.nix");
const RUNTIME_INFERENCE_TSV: &str = include_str!("../metadata/runtime-inference.tsv");
const KNOWN_COMPONENTS: &[&str] = &[
    "python-uv",
    "native-build",
    "linux-headers",
    "desktop-gl",
    "cuda-toolkit",
];

fn main() -> ExitCode {
    match run(env::args_os().skip(1).collect()) {
        Ok(code) => code,
        Err(error) => {
            print_error(&error);
            if error.write_debug_log {
                match write_debug_log(&error) {
                    Ok(path) => eprintln!("debug: wrote {}", path.display()),
                    Err(err) => eprintln!("debug: failed to write .robo-nix/last-error.log: {err}"),
                }
            }
            ExitCode::FAILURE
        }
    }
}

fn run(args: Vec<OsString>) -> Result<ExitCode, AppError> {
    let mut args = args.into_iter();
    let Some(command) = args.next() else {
        print_usage();
        return Ok(ExitCode::SUCCESS);
    };
    let command = command
        .to_str()
        .ok_or_else(|| AppError::user("command must be valid UTF-8"))?;

    match command {
        "shell" => shell_command(args.collect()),
        "run" => run_command(args.collect()),
        "-h" | "--help" | "help" => {
            print_usage();
            Ok(ExitCode::SUCCESS)
        }
        "init" => Err(AppError::user("`robo init` has been removed")
            .with_hint("run `robo shell` from a project with .python-version instead.")),
        "check" => Err(AppError::user("`robo check` is not part of this branch")
            .with_hint("run `robo shell`; future correctness checks will use a separate surface.")),
        other => Err(AppError::user(format!("unknown command `{other}`"))),
    }
}

fn print_usage() {
    print!("{HELP}");
}

fn shell_command(args: Vec<OsString>) -> Result<ExitCode, AppError> {
    if !args.is_empty() {
        return Err(AppError::user(
            "shell does not accept arguments; use `robo run` for commands",
        ));
    }
    run_nix_develop(Vec::new())
}

fn run_command(args: Vec<OsString>) -> Result<ExitCode, AppError> {
    if args.is_empty() {
        return Err(AppError::user("run requires a command"));
    }
    run_nix_develop(args)
}

fn run_nix_develop(command_args: Vec<OsString>) -> Result<ExitCode, AppError> {
    prepare_project(Path::new("."))?;

    let mut command = Command::new("nix");
    command.arg("develop").arg("--accept-flake-config");

    if !command_args.is_empty() {
        command.arg("--command").args(command_args);
    }

    let status = command.status().map_err(|err| {
        AppError::project(format!("failed to start nix: {err}"))
            .with_hint("install Nix with flakes enabled, then rerun `robo shell`.")
    })?;
    if status.success() {
        Ok(ExitCode::SUCCESS)
    } else {
        Err(AppError::project(format!("nix develop exited with {status}"))
            .with_hint("review the Nix output above and attach .robo-nix/last-error.log to an issue if this looks like a robo-nix bug."))
    }
}

fn prepare_project(root: &Path) -> Result<BootstrapReport, AppError> {
    let python_version = read_python_version(root)?;
    fs::create_dir_all(root.join(".robo-nix"))
        .map_err(|err| AppError::project(format!("failed to create .robo-nix/: {err}")))?;

    // NOTE: shell bootstraps missing files only. Existing robo.nix is user-owned.
    let mut report = BootstrapReport::default();
    let flake_path = root.join("flake.nix");
    if flake_path.exists() {
        let flake = fs::read_to_string(&flake_path)
            .map_err(|err| AppError::project(format!("failed to read flake.nix: {err}")))?;
        if !looks_like_robo_flake(&flake) {
            return Err(
                AppError::project("this repository already has a non-robo flake.nix")
                    .with_hint("robo shell will not overwrite an existing non-robo flake."),
            );
        }
    } else {
        fs::write(&flake_path, PROJECT_FLAKE_TEMPLATE)
            .map_err(|err| AppError::project(format!("failed to write flake.nix: {err}")))?;
        report.wrote_flake = true;
        println!("generated: flake.nix");
    }

    let robo_path = root.join("robo.nix");
    if !robo_path.exists() {
        let inference = infer_initial_runtime(root)?;
        let robo_nix = render_robo_nix(&inference)?;
        fs::write(&robo_path, robo_nix)
            .map_err(|err| AppError::project(format!("failed to write robo.nix: {err}")))?;
        report.wrote_robo_nix = true;
        println!("generated: robo.nix");
        print_inference_report(&inference);
    }

    report.python_version = python_version;
    Ok(report)
}

fn read_python_version(root: &Path) -> Result<String, AppError> {
    let path = root.join(".python-version");
    let raw = fs::read_to_string(&path).map_err(|err| {
        if err.kind() == io::ErrorKind::NotFound {
            AppError::project("missing .python-version")
                .with_hint("choose the project Python version first, for example with `uv python pin <version>`.")
        } else {
            AppError::project(format!("failed to read .python-version: {err}"))
        }
    })?;
    let version = raw.lines().next().unwrap_or("").trim();
    if version.is_empty() {
        return Err(AppError::project(".python-version is empty")
            .with_hint("write the project Python version, for example `3.11` or `3.12`."));
    }
    Ok(version.to_string())
}

fn looks_like_robo_flake(flake: &str) -> bool {
    flake.contains("nixpkgs-python")
        && flake.contains("import ./robo.nix")
        && flake.contains(".python-version")
}

fn infer_initial_runtime(root: &Path) -> Result<RuntimeInference, AppError> {
    // NOTE: inference is first-bootstrap only; existing robo.nix is canonical.
    let mut inference = RuntimeInference::base();
    let pyproject_path = root.join("pyproject.toml");
    if !pyproject_path.exists() {
        inference.pyproject_status = PyprojectStatus::Missing;
        return Ok(inference);
    }

    let pyproject = fs::read_to_string(&pyproject_path)
        .map_err(|err| AppError::project(format!("failed to read pyproject.toml: {err}")))?;
    let Ok(value) = pyproject.parse::<toml::Value>() else {
        inference.pyproject_status = PyprojectStatus::Invalid;
        return Ok(inference);
    };

    inference.pyproject_status = PyprojectStatus::Read;
    let dependencies = project_dependency_names(&value);
    let rules = runtime_rules()?;
    for dependency in dependencies {
        for rule in rules.iter().filter(|rule| rule.package == dependency) {
            inference.components.insert(rule.component.clone());
            inference.matches.push(RuntimeMatch {
                package: dependency.clone(),
                component: rule.component.clone(),
                note: rule.note.clone(),
            });
        }
    }

    Ok(inference)
}

fn project_dependency_names(value: &toml::Value) -> BTreeSet<String> {
    let mut names = BTreeSet::new();
    let Some(project) = value.get("project").and_then(toml::Value::as_table) else {
        return names;
    };
    let Some(dependencies) = project.get("dependencies").and_then(toml::Value::as_array) else {
        return names;
    };

    for dependency in dependencies {
        let Some(spec) = dependency.as_str() else {
            continue;
        };
        if let Some(name) = requirement_name(spec) {
            names.insert(name);
        }
    }

    names
}

fn requirement_name(spec: &str) -> Option<String> {
    let mut name = String::new();
    for character in spec.trim().chars() {
        if character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.') {
            if matches!(character, '_' | '.') {
                name.push('-');
            } else {
                name.push(character.to_ascii_lowercase());
            }
        } else {
            break;
        }
    }

    (!name.is_empty()).then_some(name)
}

fn runtime_rules() -> Result<Vec<RuntimeRule>, AppError> {
    let mut rules = Vec::new();
    for (index, line) in RUNTIME_INFERENCE_TSV.lines().enumerate() {
        if index == 0 || line.trim().is_empty() {
            continue;
        }
        let columns: Vec<_> = line.split('\t').collect();
        if columns.len() != 3 {
            return Err(AppError::project(format!(
                "metadata/runtime-inference.tsv line {} has {} columns, expected 3",
                index + 1,
                columns.len()
            )));
        }
        let component = columns[1].trim();
        if !KNOWN_COMPONENTS.contains(&component) {
            return Err(AppError::project(format!(
                "metadata/runtime-inference.tsv line {} references unknown component `{component}`",
                index + 1
            )));
        }
        rules.push(RuntimeRule {
            package: normalize_package_name(columns[0].trim()),
            component: component.to_string(),
            note: columns[2].trim().to_string(),
        });
    }
    Ok(rules)
}

fn normalize_package_name(name: &str) -> String {
    name.chars()
        .map(|character| match character {
            '_' | '.' => '-',
            other => other.to_ascii_lowercase(),
        })
        .collect()
}

fn render_robo_nix(inference: &RuntimeInference) -> Result<String, AppError> {
    render_template(
        PROJECT_ROBO_TEMPLATE,
        &[("components", render_component_lines(inference))],
    )
}

fn render_component_lines(inference: &RuntimeInference) -> String {
    let mut packages_by_component: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for matched in &inference.matches {
        packages_by_component
            .entry(&matched.component)
            .or_default()
            .push(&matched.package);
    }

    let mut lines = Vec::new();
    for component in KNOWN_COMPONENTS {
        if !inference.components.contains(*component) {
            continue;
        }
        if let Some(packages) = packages_by_component.get(component) {
            lines.push(format!(
                "    \"{component}\" # inferred from pyproject.toml: {}",
                packages.join(", ")
            ));
        } else {
            lines.push(format!("    \"{component}\""));
        }
    }
    lines.join("\n")
}

fn render_template(template: &str, values: &[(&str, String)]) -> Result<String, AppError> {
    let mut rendered = template.to_string();
    for (key, value) in values {
        rendered = rendered.replace(&format!("{{{{{key}}}}}"), value);
    }
    if rendered.contains("{{") || rendered.contains("}}") {
        return Err(AppError::project(
            "template rendering left an unresolved placeholder",
        ));
    }
    Ok(rendered)
}

fn print_inference_report(inference: &RuntimeInference) {
    match inference.pyproject_status {
        PyprojectStatus::Missing => {
            println!("note: pyproject.toml not found; generated base runtime only");
        }
        PyprojectStatus::Invalid => {
            println!("note: pyproject.toml is invalid TOML; generated base runtime only");
        }
        PyprojectStatus::Read => {
            for matched in &inference.matches {
                println!(
                    "inferred: {} from pyproject.toml dependency `{}`",
                    matched.component, matched.package
                );
                println!("note: {}", matched.note);
            }
        }
    }
}

fn print_error(error: &AppError) {
    eprintln!("error: {}", error.message);
    if let Some(hint) = &error.hint {
        eprintln!("hint: {hint}");
    }
}

fn write_debug_log(error: &AppError) -> io::Result<PathBuf> {
    // DEBUG: keep this pasteable; it is for issue reports, not user policy fixes.
    let dir = PathBuf::from(".robo-nix");
    fs::create_dir_all(&dir)?;
    let path = dir.join("last-error.log");
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs().to_string())
        .unwrap_or_else(|_| "before-unix-epoch".to_string());

    let mut text = String::new();
    text.push_str("robo-nix debug log\n");
    text.push_str(&format!("timestamp_unix = {timestamp}\n"));
    text.push_str(&format!("cwd = {}\n", env::current_dir()?.display()));
    text.push_str(&format!(
        "argv = {}\n",
        env::args().collect::<Vec<_>>().join(" ")
    ));
    text.push_str(&format!("error = {}\n", error.message));
    if let Some(hint) = &error.hint {
        text.push_str(&format!("hint = {hint}\n"));
    }
    text.push_str("\nproject files:\n");
    for file in [
        ".python-version",
        "pyproject.toml",
        "flake.nix",
        "flake.lock",
        "robo.nix",
    ] {
        let path = Path::new(file);
        text.push_str(&format!(
            "- {file}: {}\n",
            if path.exists() { "present" } else { "missing" }
        ));
    }
    if let Ok(version) = read_python_version(Path::new(".")) {
        text.push_str(&format!("\npython_version = {version}\n"));
    }

    fs::write(&path, text)?;
    Ok(path)
}

#[derive(Debug)]
struct AppError {
    message: String,
    hint: Option<String>,
    write_debug_log: bool,
}

impl AppError {
    fn user(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            hint: None,
            write_debug_log: false,
        }
    }

    fn project(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            hint: None,
            write_debug_log: true,
        }
    }

    fn with_hint(mut self, hint: impl Into<String>) -> Self {
        self.hint = Some(hint.into());
        self
    }
}

#[derive(Debug, Default)]
struct BootstrapReport {
    python_version: String,
    wrote_flake: bool,
    wrote_robo_nix: bool,
}

struct RuntimeRule {
    package: String,
    component: String,
    note: String,
}

struct RuntimeMatch {
    package: String,
    component: String,
    note: String,
}

struct RuntimeInference {
    components: BTreeSet<String>,
    matches: Vec<RuntimeMatch>,
    pyproject_status: PyprojectStatus,
}

impl RuntimeInference {
    fn base() -> Self {
        Self {
            components: BTreeSet::from(["python-uv".to_string()]),
            matches: Vec::new(),
            pyproject_status: PyprojectStatus::Missing,
        }
    }
}

enum PyprojectStatus {
    Missing,
    Invalid,
    Read,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bootstrap_requires_python_version() {
        let root = temp_project("requires-python");

        let error = prepare_project(&root).unwrap_err();
        assert!(error.message.contains(".python-version"));
        assert!(!root.join("flake.nix").exists());

        cleanup(root);
    }

    #[test]
    fn bootstrap_writes_runtime_files_without_pyproject() {
        let root = temp_project("base");
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join(".python-version"), "3.12\n").unwrap();

        let report = prepare_project(&root).unwrap();

        assert_eq!(report.python_version, "3.12");
        assert!(report.wrote_flake);
        assert!(report.wrote_robo_nix);
        assert!(root.join(".robo-nix").is_dir());
        assert!(!root.join("pyproject.toml").exists());
        assert!(fs::read_to_string(root.join("flake.nix"))
            .unwrap()
            .contains("builtins.readFile ./.python-version"));
        assert!(fs::read_to_string(root.join("robo.nix"))
            .unwrap()
            .contains("\"python-uv\""));

        cleanup(root);
    }

    #[test]
    fn first_bootstrap_infers_initial_components_from_pyproject() {
        let root = temp_project("inference");
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join(".python-version"), "3.11\n").unwrap();
        fs::write(
            root.join("pyproject.toml"),
            r#"[project]
dependencies = [
  "torch>=2",
  "mujoco",
]
"#,
        )
        .unwrap();

        prepare_project(&root).unwrap();
        let robo_nix = fs::read_to_string(root.join("robo.nix")).unwrap();

        assert!(robo_nix.contains("\"python-uv\""));
        assert!(robo_nix.contains("\"native-build\" # inferred from pyproject.toml: torch"));
        assert!(robo_nix.contains("\"desktop-gl\" # inferred from pyproject.toml: mujoco"));

        cleanup(root);
    }

    #[test]
    fn first_bootstrap_infers_cuda_toolkit_for_cuda_python_packages() {
        let root = temp_project("cuda-inference");
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join(".python-version"), "3.11\n").unwrap();
        fs::write(
            root.join("pyproject.toml"),
            r#"[project]
dependencies = [
  "cuda-python",
  "cupy-cuda12x",
]
"#,
        )
        .unwrap();

        prepare_project(&root).unwrap();
        let robo_nix = fs::read_to_string(root.join("robo.nix")).unwrap();

        assert!(robo_nix.contains("\"cuda-toolkit\" # inferred from pyproject.toml:"));
        assert!(robo_nix.contains("cuda-python"));
        assert!(robo_nix.contains("cupy-cuda12x"));

        cleanup(root);
    }

    #[test]
    fn first_bootstrap_infers_linux_headers_for_evdev() {
        let root = temp_project("evdev-inference");
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join(".python-version"), "3.11\n").unwrap();
        fs::write(
            root.join("pyproject.toml"),
            r#"[project]
dependencies = [
  "evdev<1.9.3; sys_platform == 'linux'",
]
"#,
        )
        .unwrap();

        prepare_project(&root).unwrap();
        let robo_nix = fs::read_to_string(root.join("robo.nix")).unwrap();

        assert!(robo_nix.contains("\"native-build\" # inferred from pyproject.toml: evdev"));
        assert!(robo_nix.contains("\"linux-headers\" # inferred from pyproject.toml: evdev"));

        cleanup(root);
    }

    #[test]
    fn existing_robo_nix_is_canonical() {
        let root = temp_project("existing-robo");
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join(".python-version"), "3.11\n").unwrap();
        fs::write(
            root.join("robo.nix"),
            "{ components = [ \"python-uv\" ]; }\n",
        )
        .unwrap();
        fs::write(
            root.join("pyproject.toml"),
            r#"[project]
dependencies = ["opencv-python"]
"#,
        )
        .unwrap();

        prepare_project(&root).unwrap();

        assert_eq!(
            fs::read_to_string(root.join("robo.nix")).unwrap(),
            "{ components = [ \"python-uv\" ]; }\n"
        );

        cleanup(root);
    }

    #[test]
    fn non_robo_flake_is_refused() {
        let root = temp_project("non-robo-flake");
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join(".python-version"), "3.11\n").unwrap();
        fs::write(root.join("flake.nix"), "{ outputs = _: {}; }\n").unwrap();

        let error = prepare_project(&root).unwrap_err();

        assert!(error.message.contains("non-robo flake"));
        assert!(!root.join("robo.nix").exists());

        cleanup(root);
    }

    #[test]
    fn requirement_names_are_normalized() {
        assert_eq!(
            requirement_name("opencv_python>=4").as_deref(),
            Some("opencv-python")
        );
        assert_eq!(
            requirement_name("torch[dev] == 2").as_deref(),
            Some("torch")
        );
    }

    fn temp_project(label: &str) -> PathBuf {
        let root = env::temp_dir().join(format!("robo-minimal-{label}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        root
    }

    fn cleanup(root: PathBuf) {
        let _ = fs::remove_dir_all(root);
    }
}
