//! YAML frontmatter: finding it, reading it into JSON-like properties, and
//! writing an `id` into it.

use {
    serde_json::{Map, Value},
    yaml_rust2::{Yaml, YamlLoader},
};

/// Where the frontmatter is in a file: the YAML between the fences, and the
/// byte where the body starts.
pub(crate) struct Fenced<'a> {
    pub yaml: &'a str,
    pub body_start: usize,
}

/// The frontmatter at the very top of `source`, if any: `---` on the first
/// line, closed by `---` (or `...`) on a line of its own.
pub(crate) fn find(source: &str) -> Option<Fenced<'_>> {
    let rest = source.strip_prefix("---")?;
    let rest_start = 3;
    let first_break = rest.find('\n')?;
    if !rest[..first_break].trim_end_matches('\r').trim().is_empty() {
        return None;
    }
    let yaml_start = rest_start + first_break + 1;
    let mut at = yaml_start;
    for line in source[yaml_start..].split_inclusive('\n') {
        let bare = line.trim_end_matches(['\n', '\r']);
        if bare == "---" || bare == "..." {
            return Some(Fenced {
                yaml: &source[yaml_start..at],
                body_start: at + line.len(),
            });
        }
        at += line.len();
    }
    None
}

/// The properties in `yaml` as a JSON object, in the order written. YAML
/// that does not read as a mapping gives none (and the note still indexes).
pub(crate) fn read(yaml: &str) -> Map<String, Value> {
    let Ok(docs) = YamlLoader::load_from_str(yaml) else {
        return Map::new();
    };
    let Some(Yaml::Hash(hash)) = docs.into_iter().next() else {
        return Map::new();
    };
    let mut out = Map::new();
    for (key, value) in hash {
        let key = match key {
            Yaml::String(key) => key,
            Yaml::Integer(number) => number.to_string(),
            Yaml::Real(real) => real,
            Yaml::Boolean(flag) => flag.to_string(),
            _ => continue,
        };
        out.insert(key, to_json(value));
    }
    out
}

fn to_json(value: Yaml) -> Value {
    match value {
        Yaml::String(text) => Value::String(text),
        Yaml::Integer(number) => Value::from(number),
        Yaml::Real(real) => real.parse::<f64>().map_or(Value::String(real), Value::from),
        Yaml::Boolean(flag) => Value::Bool(flag),
        Yaml::Array(items) => Value::Array(items.into_iter().map(to_json).collect()),
        Yaml::Hash(hash) => {
            let mut map = Map::new();
            for (key, value) in hash {
                if let Yaml::String(key) = key {
                    map.insert(key, to_json(value));
                }
            }
            Value::Object(map)
        },
        Yaml::Null | Yaml::BadValue | Yaml::Alias(_) => Value::Null,
    }
}

/// The strings of a property that may be one string or a list of them
/// (`tags: a, b`, `tags: [a, b]`, `aliases:\n  - x`). Comma-separated
/// strings split, as Obsidian reads them.
pub(crate) fn strings(value: Option<&Value>) -> Vec<String> {
    let mut out = Vec::new();
    let mut push = |text: &str| {
        for part in text.split(',') {
            let part = part.trim();
            if !part.is_empty() {
                out.push(part.to_string());
            }
        }
    };
    match value {
        Some(Value::String(text)) => push(text),
        Some(Value::Array(items)) => {
            for item in items {
                match item {
                    Value::String(text) => push(text),
                    Value::Number(number) => push(&number.to_string()),
                    _ => {},
                }
            }
        },
        Some(Value::Number(number)) => push(&number.to_string()),
        _ => {},
    }
    out
}

/// `source` with `id: <id>` in its frontmatter: added as the first key of
/// existing frontmatter, or in new frontmatter at the top. A note that
/// already has an id is returned unchanged.
pub(crate) fn with_id(source: &str, id: &str) -> String {
    match find(source) {
        Some(fenced) => {
            if read(fenced.yaml).contains_key("id") {
                return source.to_string();
            }
            let newline = if source.starts_with("---\r\n") {
                "\r\n"
            } else {
                "\n"
            };
            let yaml_start = source.len() - source[3..].trim_start_matches(['\r', '\n']).len();
            format!(
                "{}id: {id}{newline}{}",
                &source[..yaml_start],
                &source[yaml_start..]
            )
        },
        None => format!("---\nid: {id}\n---\n{source}"),
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;

    #[test]
    fn finds_frontmatter_and_body() {
        let source = "---\ntitle: X\ntags: [a, b]\n---\n# Body\n";
        let fenced = find(source).unwrap();
        assert_eq!(fenced.yaml, "title: X\ntags: [a, b]\n");
        assert_eq!(&source[fenced.body_start..], "# Body\n");
        let props = read(fenced.yaml);
        assert_eq!(props["title"], "X");
        assert_eq!(strings(props.get("tags")), vec!["a", "b"]);
    }

    #[test]
    fn no_frontmatter_without_a_closing_fence_or_at_the_top() {
        assert!(find("---\ntitle: X\n").is_none());
        assert!(find("\n---\ntitle: X\n---\n").is_none());
        assert!(find("---- not yaml\n---\n").is_none());
    }

    #[test]
    fn bad_yaml_gives_no_properties() {
        assert!(read("tags: [unclosed").is_empty());
        assert!(read("- just\n- a list\n").is_empty());
    }

    #[test]
    fn comma_strings_split() {
        assert_eq!(strings(Some(&Value::String("one, two".into()))), vec![
            "one", "two"
        ]);
    }

    #[test]
    fn adds_an_id() {
        assert_eq!(with_id("Hello", "01X"), "---\nid: 01X\n---\nHello");
        assert_eq!(
            with_id("---\ntags: a\n---\nHi", "01X"),
            "---\nid: 01X\ntags: a\n---\nHi"
        );
        let kept = "---\nid: 01A\n---\nHi";
        assert_eq!(with_id(kept, "01X"), kept);
        assert_eq!(
            with_id("---\r\ntags: a\r\n---\r\nHi", "01X"),
            "---\r\nid: 01X\r\ntags: a\r\n---\r\nHi"
        );
    }
}
