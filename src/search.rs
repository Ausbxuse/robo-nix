use std::collections::BTreeSet;
use std::ffi::OsString;
use std::process::{Command, ExitCode, Output};

use crate::ui::{detail, error, hint, list_item, output_with_spinner, row, section, Config};

const MAX_PRINTED_CANDIDATES: usize = 12;
const MAX_SNIPPET_CANDIDATES: usize = 5;

pub(crate) fn run(args: Vec<OsString>, config: Config) -> ExitCode {
    let mut args = args.into_iter();
    let Some(query) = args.next() else {
        error(config, "search requires a shared library name");
        hint(config, "example: robo search libassimp.so");
        return ExitCode::from(2);
    };
    if args.next().is_some() {
        error(config, "search accepts exactly one query");
        hint(config, "example: robo search libGL.so.1");
        return ExitCode::from(2);
    }
    let Some(query) = query.to_str() else {
        error(config, "search query must be valid UTF-8");
        return ExitCode::from(2);
    };

    let query = normalize_library_query(query);
    if query.is_empty() {
        error(config, "search query is empty");
        hint(config, "example: robo search libz.so.1");
        return ExitCode::from(2);
    }

    let attempt = search_nix_index(config, &query);
    let (source, output) = match attempt {
        SearchAttempt::Found { source, output } => (source, output),
        SearchAttempt::Failed { message, output } => {
            error(config, &message);
            if let Some(output) = output {
                print_captured_stderr(&output);
            }
            print_setup_hint(config);
            return ExitCode::FAILURE;
        }
    };

    let attrs = nix_locate_attrs(&String::from_utf8_lossy(&output.stdout));
    if attrs.is_empty() {
        error(config, &format!("no library match for `{query}`"));
        hint(
            config,
            "try a basename such as `libassimp.so`, `libGL.so`, or `libz.so`.",
        );
        return ExitCode::FAILURE;
    }

    print_matches(config, &query, source, &attrs);
    ExitCode::SUCCESS
}

enum SearchAttempt {
    Found {
        source: &'static str,
        output: Output,
    },
    Failed {
        message: String,
        output: Option<Output>,
    },
}

fn search_nix_index(config: Config, query: &str) -> SearchAttempt {
    let mut command = nix_locate_command("nix-locate", query);
    match output_with_spinner(config, &mut command, "checking local nix-index") {
        Ok(output) if output.status.success() => SearchAttempt::Found {
            source: "local nix-index",
            output,
        },
        Ok(output) if nix_index_database_missing(&output) => search_prebuilt_index(config, query),
        Ok(output) => SearchAttempt::Failed {
            message: "nix-locate could not search the package file index".to_string(),
            output: Some(output),
        },
        Err(_) => search_prebuilt_index(config, query),
    }
}

fn search_prebuilt_index(config: Config, query: &str) -> SearchAttempt {
    let mut command = prebuilt_nix_locate_command(query);
    match output_with_spinner(config, &mut command, "checking prebuilt nix-index") {
        Ok(output) if output.status.success() => SearchAttempt::Found {
            source: "prebuilt nix-index",
            output,
        },
        Ok(output) => SearchAttempt::Failed {
            message: "prebuilt nix-index search failed".to_string(),
            output: Some(output),
        },
        Err(err) => SearchAttempt::Failed {
            message: format!("failed to run prebuilt nix-index search: {err}"),
            output: None,
        },
    }
}

fn nix_locate_command(program: &str, query: &str) -> Command {
    let mut command = Command::new(program);
    command.args(nix_locate_args(query));
    command
}

fn prebuilt_nix_locate_command(query: &str) -> Command {
    let mut command = Command::new("nix");
    command.args([
        "--extra-experimental-features",
        "nix-command",
        "--extra-experimental-features",
        "flakes",
        "run",
        "github:nix-community/nix-index-database",
        "--",
    ]);
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

fn nix_index_database_missing(output: &Output) -> bool {
    let stderr = String::from_utf8_lossy(&output.stderr);
    stderr.contains("No such file or directory") && stderr.contains("nix-index")
}

fn print_matches(config: Config, query: &str, source: &str, attrs: &[String]) {
    section(config, "library");
    row(config, "✓", "query", query);
    row(config, "✓", "source", source);

    section(config, "candidates");
    for attr in attrs.iter().take(MAX_PRINTED_CANDIDATES) {
        list_item(config, &format!("pkgs.{attr}"));
    }
    if attrs.len() > MAX_PRINTED_CANDIDATES {
        list_item(
            config,
            &format!("... {} more", attrs.len() - MAX_PRINTED_CANDIDATES),
        );
    }

    section(config, "robo.nix");
    list_item(config, "extraRuntimeLibraries = pkgs: [");
    for attr in attrs.iter().take(MAX_SNIPPET_CANDIDATES) {
        detail(config, &format!("pkgs.{attr}"));
    }
    list_item(config, "];");
    println!();
    list_item(
        config,
        "Use the package that owns the runtime library your failing Python extension loads.",
    );
}

fn print_setup_hint(config: Config) {
    hint(
        config,
        "`robo search` uses `nix-locate`; install or update a local index with `nix-index` for fully offline search.",
    );
}

fn print_captured_stderr(output: &Output) {
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
        return query.to_string();
    };
    format!("{base}.so")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn library_query_accepts_error_log_fragments() {
        assert_eq!(
            normalize_library_query("`/nix/store/x/lib/libGL.so.1':"),
            "libGL.so"
        );
        assert_eq!(normalize_library_query("libz.so.1,"), "libz.so");
        assert_eq!(normalize_library_query("libassimp.so"), "libassimp.so");
    }

    #[test]
    fn nix_locate_attrs_are_deduped_and_output_suffixes_removed() {
        let attrs = nix_locate_attrs(
            r#"
assimp.out /nix/store/x/lib/libassimp.so
assimp.lib /nix/store/y/lib/libassimp.so.5
assimp.debug /nix/store/z/lib/debug/libassimp.so
qt6.qtbase.out /nix/store/q/lib/libGL.so
"#,
        );

        assert_eq!(attrs, vec!["assimp", "qt6.qtbase"]);
    }
}
