use serde_json::{Map, Value};
use std::collections::HashMap;
use regex::Regex;

/// Enum representing the kinds of transformations that can be applied to key parts.
#[derive(Debug, Clone, Copy)]
pub enum TransformKind {
    Lowercase,
    Uppercase,
    ToCamelCase,
    ToSnakeCase,
}

/// Inserts a key like `foo.bar.baz` or `foo_bar_baz` into a nested `Map<String, Value>` structure splitting over separator.
fn insert_into_map(
    current_map: &mut Map<String, Value>,
    parts: &[&str],
    value: &str,
    separator: char,
) {
    let part = parts[0];

    if let Ok(index) = part.parse::<usize>() {
        // Handle array case
        handle_numeric_part(current_map, parts, value, separator, part, index);
    } else {
        // Handle object case
        handle_string_part(current_map, parts, value, separator, part);
    }
}

fn handle_string_part(current_map: &mut Map<String, Value>, parts: &[&str], value: &str, separator: char, part: &str) {
    let entry = current_map
        .entry(part.to_string())
        .or_insert_with(|| Value::Object(Map::new()));

    if parts.len() == 1 {
        // We're at the last part, insert the value
        *entry = Value::String(value.to_string());
    } else {
        // We need to go deeper
        if let Value::Object(obj) = entry {
            insert_into_map(obj, &parts[1..], value, separator);
        } else {
            panic!("Expected object but found a different value");
        }
    }
}

fn handle_numeric_part(current_map: &mut Map<String, Value>, parts: &[&str], value: &str, separator: char, part: &str, index: usize) {
    let entry = current_map
        .entry(part.to_string())
        .or_insert_with(|| Value::Array(Vec::new()));

    if let Value::Array(current_array) = entry {
        if parts.len() == 1 {
            // We're at the last part, insert the value
            if index >= current_array.len() {
                current_array.resize(index + 1, Value::Null);
            }
            current_array[index] = Value::String(value.to_string());
        } else {
            // We need to go deeper
            if index >= current_array.len() {
                current_array.resize(index + 1, Value::Object(Map::new()));
            }
            if let Value::Object(obj) = current_array.get_mut(index).expect("Expected object") {
                insert_into_map(obj, &parts[1..], value, separator);
            } else {
                panic!("Expected object but found a different value");
            }
        }
    } else {
        panic!("Expected array but found a different value");
    }
}

fn transform_key_part(part: &str, transforms: &[TransformKind]) -> String {
    transforms.iter().fold(part.to_string(), |acc, &transform| {
        match transform {
            TransformKind::Lowercase => acc.to_lowercase(),
            TransformKind::Uppercase => acc.to_uppercase(),
            TransformKind::ToCamelCase => {
                let mut chars = acc.chars();
                chars.next().map(|c| c.to_ascii_lowercase()).into_iter().collect::<String>() + chars.as_str()
            }
            TransformKind::ToSnakeCase => {
                let re = Regex::new(r"([a-z0-9])([A-Z])").unwrap();
                let snake_case = re.replace_all(&acc, "$1_$2");
                snake_case.to_lowercase()
            }
        }
    })
}

pub fn key_value_pairs_to_json(
    properties: HashMap<String, String>,
    separator: char,
    escape: Option<&str>,
    transforms: &[TransformKind],
) -> Map<String, Value> {
    let mut map = Map::new();

    for (key, value) in properties {
        let parts: Vec<&str> = if let Some(escape_seq) = escape {
            let split_regex = Regex::new(&format!(
                r"(?<!{}){}",
                regex::escape(escape_seq),
                regex::escape(&separator.to_string())
            ))
                .unwrap();
            split_regex.split(&key).collect()
        } else {
            key.split(separator).collect()
        };

        let transformed_parts: Vec<String> = parts
            .into_iter()
            .map(|part| transform_key_part(part, transforms))
            .collect();

        insert_into_map(&mut map, &transformed_parts.iter().map(AsRef::as_ref).collect::<Vec<&str>>(), &value, separator);
    }

    map
}