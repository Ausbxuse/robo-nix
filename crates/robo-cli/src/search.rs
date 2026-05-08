use clap::Args;
use std::collections::BTreeSet;
use std::process::{Command, ExitCode};

use crate::{Config, LabelKind, error, inline, label};

#[derive(Args)]
pub(crate) struct SearchArgs {
    #[arg(help = "Shared library name, for example libassimp.so")]
    query: String,
}

pub(crate) fn run(args: SearchArgs, config: Config) -> ExitCode {
    let query = normalize_library_query(&args.query);
    let output = match run_nix_locate(config, &query) {
        SearchAttempt::Found(output) => output,
        SearchAttempt::Failed(message, output) => {
            error(config, &message);
            if let Some(output) = output {
                print_captured(&output);
            }
            print_nix_index_setup(config);
            return ExitCode::from(1);
        }
    };

    let attrs = nix_locate_attrs(&String::from_utf8_lossy(&output.stdout));
    if attrs.is_empty() {
        println!("no library match: {}", inline(config, &args.query));
        println!();
        println!("Try a basename such as `libassimp.so` or `libGL.so.1`.");
        return ExitCode::from(1);
    }

    println!(
        "{} {}",
        label(config, "library:", LabelKind::Status),
        inline(config, &query)
    );
    println!(
        "{}",
        label(config, "extraRuntimeLibraries candidates:", LabelKind::Status)
    );
    for attr in &attrs {
        println!("  pkgs.{attr}");
    }
    println!();
    println!("{}", label(config, "robo.nix:", LabelKind::Status));
    println!("  extraRuntimeLibraries = pkgs: [");
    for attr in attrs.iter().take(5) {
        println!("    pkgs.{attr}");
    }
    println!("  ];");
    println!();
    println!("Use the package that owns the runtime library your failing Python extension loads.");

    ExitCode::SUCCESS
}

enum SearchAttempt {
    Found(std::process::Output),
    Failed(String, Option<std::process::Output>),
}

fn run_nix_locate(config: Config, query: &str) -> SearchAttempt {
    match nix_locate_command("nix-locate", query).output() {
        Ok(output) if output.status.success() => SearchAttempt::Found(output),
        Ok(output) if nix_index_database_missing(&output) => run_prebuilt_nix_locate(config, query),
        Ok(output) => SearchAttempt::Failed(
            "nix-locate could not search the package file index".to_string(),
            Some(output),
        ),
        Err(_) => run_prebuilt_nix_locate(config, query),
    }
}

fn run_prebuilt_nix_locate(config: Config, query: &str) -> SearchAttempt {
    let mut command = crate::nix_command(config);
    command.args(["run", "github:nix-community/nix-index-database", "--"]);
    command.args(nix_locate_args(query));
    match command.output() {
        Ok(output) if output.status.success() => SearchAttempt::Found(output),
        Ok(output) => SearchAttempt::Failed(
            "prebuilt nix-index database search failed".to_string(),
            Some(output),
        ),
        Err(err) => SearchAttempt::Failed(
            format!("failed to run prebuilt nix-index database search: {err}"),
            None,
        ),
    }
}

fn nix_locate_command(program: &str, query: &str) -> Command {
    let mut command = Command::new(program);
    command.args(nix_locate_args(query));
    command
}

fn nix_locate_args(query: &str) -> Vec<&str> {
    vec![
        "--minimal",
        "--whole-name",
        "--type",
        "r",
        "--type",
        "s",
        query,
    ]
}

fn nix_index_database_missing(output: &std::process::Output) -> bool {
    let stderr = String::from_utf8_lossy(&output.stderr);
    nix_index_database_missing_stderr(&stderr)
}

fn nix_index_database_missing_stderr(stderr: &str) -> bool {
    stderr.contains("No such file or directory") && stderr.contains("nix-index")
}

fn print_nix_index_setup(config: Config) {
    eprintln!();
    eprintln!("`robo search` uses nix-locate from nix-index.");
    eprintln!("robo tries the prebuilt nix-index-database first when the local index is missing.");
    eprintln!("For fully local/offline search, build/update the local index with:");
    eprintln!("  {}", label(config, "nix-index", LabelKind::Command));
}

fn print_captured(output: &std::process::Output) {
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stderr = stderr.trim();
    if !stderr.is_empty() {
        eprintln!("{stderr}");
    }
}

fn nix_locate_attrs(output: &str) -> Vec<String> {
    output
        .lines()
        .filter_map(nix_locate_attr)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn nix_locate_attr(line: &str) -> Option<String> {
    let attr = line.split_whitespace().next()?;
    let attr = attr.trim_matches(|ch| ch == '(' || ch == ')');
    let attr = match attr.rsplit_once('.') {
        Some((_, "debug")) => return None,
        Some((name, output)) if matches!(output, "out" | "lib" | "dev" | "bin") => name,
        _ => attr,
    };
    valid_nix_attr(attr).then(|| attr.to_string())
}

fn valid_nix_attr(attr: &str) -> bool {
    !attr.is_empty()
        && attr
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | '.'))
}

fn normalize_library_query(query: &str) -> String {
    let query = query.trim_matches(|ch: char| {
        ch.is_whitespace() || matches!(ch, ':' | ',' | ';' | '\'' | '"' | '`')
    });
    let query = query
        .rsplit_once('/')
        .map(|(_, name)| name)
        .unwrap_or(query);
    let Some((base, _)) = query.split_once(".so.") else {
        return query.to_ascii_lowercase();
    };
    format!("{base}.so").to_ascii_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_versioned_shared_libraries() {
        assert_eq!(normalize_library_query("libassimp.so.5"), "libassimp.so");
        assert_eq!(
            normalize_library_query("/nix/store/path/lib/libassimp.so.5.2.0"),
            "libassimp.so"
        );
        assert_eq!(normalize_library_query("libassimp.so.5:"), "libassimp.so");
    }

    #[test]
    fn parses_minimal_nix_locate_attrs() {
        let output = "kdePackages.qtquick3d.debug\nassimp.lib\n(consumer.out)\nxorg.libX11.out\n";

        assert_eq!(
            nix_locate_attrs(output),
            vec![
                "assimp".to_string(),
                "consumer".to_string(),
                "xorg.libX11".to_string()
            ]
        );
    }

    #[test]
    fn detects_missing_nix_index_database() {
        assert!(nix_index_database_missing_stderr(
            "error: reading from the database at '/home/me/.cache/nix-index/files' failed: I/O error: No such file or directory (os error 2)"
        ));
    }
}
