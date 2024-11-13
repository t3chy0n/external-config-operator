use serde_json::{Map, Value};
use std::collections::{BTreeMap, HashMap};
use std::io;
use convert_case::{Case, Casing};
use crate::contract::lib::Error;


/// TODO: There are some gotchas when mapping nested structures to env, and at the same time allow 2 structures to be mergeable and readable for tools
/// Like Quarkus, to retain structure we need . mapping (for level) and case separator, like if it was camelCase or other-case to know fiferent casging
/// occoured.
pub fn json_to_key_value_pairs(
    json_value: &Value,
    separator: char,
    replace_seperator_with: &str,
    transforms: &[Case],
) -> Result<BTreeMap<String, String>, Error> {
    let mut key_value_pairs = BTreeMap::new();
    if let Value::Object(json_map) = json_value {
        let result = flatten_json(&json_map, &mut key_value_pairs, "", separator, replace_seperator_with, transforms);
        return Ok(key_value_pairs);
    }

    return Err(io::Error::new(io::ErrorKind::Other, "Attempted to fold non-object value to key-value pairs")).map_err(Error::EnvFileSerializationError)
}

fn flatten_json(
    current: &Map<String, Value>,
    key_value_pairs: &mut BTreeMap<String, String>,
    parent_key: &str,
    separator: char,
    replace_seperator_with: &str,
    transforms: &[Case],

) {
    for (key, value) in current {
        let transformed_key = transform_key_part(key.clone(), transforms)
            .replace(separator, replace_seperator_with.clone());

        let new_key = if parent_key.is_empty() {
            transformed_key
        } else {
            format!("{}{}{}", parent_key, separator, transformed_key)
        };

        match value {
            Value::Object(map) => {
                flatten_json(map, key_value_pairs, &new_key, separator, replace_seperator_with, transforms);
            }
            Value::Array(arr) => {
                for (index, element) in arr.iter().enumerate() {
                    let array_key = format!("{}{}{}", new_key, separator, index);
                    if let Value::Object(map) = element {
                        flatten_json(map, key_value_pairs, &array_key, separator, replace_seperator_with, transforms);
                    } else {
                        let val = map_value_to_string(&element);
                        key_value_pairs.insert(array_key, val);
                    }
                }
            }
            _ => {
                let val = map_value_to_string(&value);
                key_value_pairs.insert(new_key, val);
            }
        }
    }
}

fn map_value_to_string(value: &Value) -> String {
     match value {
        Value::String(s) => s.clone(),              // Directly use the string without quotes
        Value::Number(n) => n.to_string(),          // Convert numbers to strings
        Value::Bool(b) => b.to_string(),            // Convert bools to "true"/"false"
        Value::Null => "null".to_string(),          // Convert null to "null"
        _ => value.to_string(),                     // For objects/arrays, use `to_string()` (keeps quotes)
    }
}
fn transform_key_part(part: String, transforms: &[Case]) -> String {
    transforms.iter().fold(part, |acc, &case| acc.to_case(case))
}