use std::collections::BTreeSet;

use toml::Value;

pub(crate) fn project_name(text: &str) -> Option<String> {
    value(text)?
        .get("project")?
        .get("name")?
        .as_str()
        .map(ToOwned::to_owned)
}

pub(crate) fn exact_python_requirement(text: &str) -> Option<String> {
    let value = value(text)?;
    let raw = value
        .get("project")?
        .get("requires-python")?
        .as_str()?
        .trim();
    parse_exact_python_requirement(raw).map(ToOwned::to_owned)
}

pub(crate) fn python_requirement(text: &str) -> Option<String> {
    value(text)?
        .get("project")?
        .get("requires-python")?
        .as_str()
        .map(ToOwned::to_owned)
}

pub(crate) fn dependency_names(text: &str) -> BTreeSet<String> {
    let Some(value) = value(text) else {
        return BTreeSet::new();
    };
    let mut names = BTreeSet::new();

    if let Some(project) = value.get("project") {
        collect_dependency_array(project.get("dependencies"), &mut names);
        if let Some(optional) = project
            .get("optional-dependencies")
            .and_then(Value::as_table)
        {
            for dependencies in optional.values() {
                collect_dependency_array(Some(dependencies), &mut names);
            }
        }
    }

    if let Some(groups) = value.get("dependency-groups").and_then(Value::as_table) {
        for dependencies in groups.values() {
            collect_dependency_array(Some(dependencies), &mut names);
        }
    }

    if let Some(poetry) = value
        .get("tool")
        .and_then(Value::as_table)
        .and_then(|tool| tool.get("poetry"))
    {
        collect_poetry_dependencies(poetry.get("dependencies"), &mut names);
        collect_poetry_dependencies(poetry.get("dev-dependencies"), &mut names);
        if let Some(groups) = poetry.get("group").and_then(Value::as_table) {
            for group in groups.values() {
                collect_poetry_dependencies(group.get("dependencies"), &mut names);
            }
        }
    }

    names
}

pub(crate) fn has_dependency_name<'a>(
    dependencies: &BTreeSet<String>,
    names: impl IntoIterator<Item = &'a str>,
) -> bool {
    names
        .into_iter()
        .map(normalize_name)
        .any(|name| dependencies.contains(&name))
}

fn value(text: &str) -> Option<Value> {
    text.parse::<Value>().ok()
}

fn collect_dependency_array(value: Option<&Value>, names: &mut BTreeSet<String>) {
    let Some(items) = value.and_then(Value::as_array) else {
        return;
    };
    for item in items {
        if let Some(raw) = item.as_str() {
            if let Some(name) = dependency_name(raw) {
                names.insert(name);
            }
        }
    }
}

fn collect_poetry_dependencies(value: Option<&Value>, names: &mut BTreeSet<String>) {
    let Some(table) = value.and_then(Value::as_table) else {
        return;
    };
    for name in table.keys() {
        if name != "python" {
            names.insert(normalize_name(name));
        }
    }
}

fn dependency_name(raw: &str) -> Option<String> {
    let raw = raw.trim();
    let end = raw
        .find(|ch: char| !(ch.is_ascii_alphanumeric() || ch == '_' || ch == '-' || ch == '.'))
        .unwrap_or(raw.len());
    if end == 0 {
        return None;
    }
    Some(normalize_name(&raw[..end]))
}

fn normalize_name(name: &str) -> String {
    let mut normalized = String::new();
    let mut last_was_dash = false;
    for ch in name.chars().flat_map(char::to_lowercase) {
        if ch == '_' || ch == '-' || ch == '.' {
            if !last_was_dash {
                normalized.push('-');
                last_was_dash = true;
            }
        } else {
            normalized.push(ch);
            last_was_dash = false;
        }
    }
    normalized.trim_matches('-').to_string()
}

fn parse_exact_python_requirement(raw: &str) -> Option<&str> {
    let Some(rest) = raw.strip_prefix("===").or_else(|| raw.strip_prefix("==")) else {
        return None;
    };
    let rest = rest.trim_start();
    let end = rest
        .find(|ch: char| !(ch.is_ascii_digit() || ch == '.' || ch == '*'))
        .unwrap_or(rest.len());
    let token = &rest[..end];
    let version = token.strip_suffix(".*").unwrap_or(token);
    let parts = version.split('.').collect::<Vec<_>>();
    if matches!(parts.len(), 2 | 3)
        && parts
            .iter()
            .all(|part| !part.is_empty() && part.chars().all(|ch| ch.is_ascii_digit()))
    {
        Some(version)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_structured_dependencies() {
        let text = r#"
[project]
name = "demo"
requires-python = "==3.11.*"
dependencies = ["torch>=2", "opencv_python[headless]"]

[project.optional-dependencies]
dev = ["PyQt6"]

[dependency-groups]
train = ["flash-attn"]

[tool.poetry.dependencies]
python = "^3.11"
dm-control = "*"
"#;

        let names = dependency_names(text);
        assert!(has_dependency_name(&names, ["torch"]));
        assert!(has_dependency_name(&names, ["opencv-python"]));
        assert!(has_dependency_name(&names, ["pyqt6"]));
        assert!(has_dependency_name(&names, ["flash-attn"]));
        assert!(has_dependency_name(&names, ["dm_control"]));
        assert_eq!(project_name(text), Some("demo".to_string()));
        assert_eq!(exact_python_requirement(text), Some("3.11".to_string()));
    }
}
