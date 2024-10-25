use serde_json::{Map, Value};
use std::collections::HashMap;
use convert_case::{Case, Casing};



fn insert_into_map(
    current_map: &mut Map<String, Value>,
    parent: Option<&str>,
    parts: &[&str],
    value: &str,
    separator: char,
) {
    let part = parts[0];



    if let Ok(index) = part.parse::<usize>() {
        if None == parent {
            panic!("Expected parent node but found nonee");
        }
        // Handle array case
        let entryArray = current_map
            .entry(parent.unwrap())
            .or_insert_with(|| Value::Array(Vec::new()));

        if let Value::Array(obj) = entryArray {
            handle_numeric_part(entryArray, parts, value, separator, part, index);
        } else {
            panic!("Expected object but found a different value");
        }
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
            insert_into_map(obj, Some(parts[0]), &parts[1..], value, separator);
        } else {
           // panic!("Expected object but found a different value");
        }
    }
}

//TODO: Fix this, inner object with index nunber is created rather to insert to array at current value.
fn handle_numeric_part(current_array: &mut Value, parts: &[&str], value: &str, separator: char, part: &str, index: usize) {

    if let Value::Array(current_array) = current_array {
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
                insert_into_map(obj, Some(parts[0]), &parts[1..], value, separator);
            } else {
                panic!("Expected object but found a different value");
            }
        }
    } else {
        panic!("Expected array but found a different value");
    }
}

fn transform_key_part(part: String, transforms: &[Case]) -> String {
    transforms.iter().fold(part.to_string(), |acc, &case| {
        acc.to_case(case)
    })
}


// Custom function to split a string with escape character handling
fn split_with_escape(s: &str, separator: char, escape_char: char) -> Vec<String> {
    let mut result = Vec::new();
    let mut current = String::new();
    let mut chars = s.chars().peekable();
    let mut escape_next = false;

    while let Some(c) = chars.next() {
        let is_nex_char_separator = match chars.peek() {
            Some(c) => c.clone() == separator,
            None => false
        };

        if escape_next {
            // If previous character was an escape character, include this character literally
            current.push(c);
            escape_next = false;
        } else if c == escape_char && is_nex_char_separator {
            // Next character should be escaped
            escape_next = true;
        } else if c == separator {
            // If we encounter the separator, start a new part
            result.push(current.clone());
            current.clear();
        } else {
            // Regular character, add to current part
            current.push(c);
        }
    }

    // Add the last part
    result.push(current);

    result
}

pub fn key_value_pairs_to_json(
    properties: HashMap<String, String>,
    separator: char,
    escape: Option<&str>,
    transforms: &[Case],
) -> Map<String, Value> {
    let mut map = Map::new();

    for (key, value) in properties {
        let parts: Vec<String> = if let Some(escape_seq) = escape {

            // Ensure the escape sequence is a single character
            assert!(
                escape_seq.chars().count() == 1,
                "Escape sequence must be a single character."
            );

            // Use a custom splitting function instead of regex
            split_with_escape(&key, separator, escape_seq.chars().next().unwrap())

        } else {
            key.split(separator).map(|s| s.to_string()).collect()
        };

        let transformed_parts: Vec<String> = parts
            .into_iter()
            .map(|part| transform_key_part(part, transforms))
            .collect();

        insert_into_map(&mut map, None, &transformed_parts.iter().map(AsRef::as_ref).collect::<Vec<&str>>(), &value, separator);
    }

    map
}