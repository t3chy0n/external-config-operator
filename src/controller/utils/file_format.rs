use std::collections::HashMap;
use std::io::Cursor;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use crate::contract::lib::Error;

use serde_json::Value as JsonValue;
use toml::Value as TomlValue;
use serde_yaml::Value as YamlValue;
use std::str::FromStr;
use json5;
use config::{Config, File, FileFormat};
use futures::StreamExt;
use std::path::Path;
use convert_case::{Case, Converter, Pattern};
use dotenvy::{dotenv, Iter};
use java_properties::{Line, PropertiesIter};
use serde_json::{Map, Value};
use crate::controller::utils::parsers::json_to_key_value::json_to_key_value_pairs;

#[derive( Debug, Clone, Deserialize, Serialize, JsonSchema)]
pub enum ConfigFileType {
    Json,
    Json5,
    Toml,
    Yaml,
    EnvFile,
    Properties,
}
pub enum ConfigFormat {
    Json(JsonValue),
    Toml(TomlValue),
    Yaml(YamlValue),
    Properties(HashMap<String, String>),
    EnvFile(HashMap<String, String>),
}



/// Merges two structured configurations together.
/// Merges two JSON configurations together.
/// Merges two structured configurations together.
pub fn merge_configs(config1: ConfigFormat, config2: ConfigFormat) -> Result<ConfigFormat, Error> {
    match (config1, config2) {
        // Merging two JSON configurations
        (ConfigFormat::Json(mut json1), ConfigFormat::Json(json2)) => {
            merge_json(&mut json1, &json2);
            Ok(ConfigFormat::Json(json1))
        }
        // Merging two TOML configurations
        (ConfigFormat::Toml(mut toml1), ConfigFormat::Toml(toml2)) => {
            merge_toml(&mut toml1, &toml2);
            Ok(ConfigFormat::Toml(toml1))
        }
        // Merging two YAML configurations
        (ConfigFormat::Yaml(mut yaml1), ConfigFormat::Yaml(yaml2)) => {
            merge_yaml(&mut yaml1, &yaml2);
            Ok(ConfigFormat::Yaml(yaml1))
        }
        // Merging two Properties configurations
        (ConfigFormat::Properties(mut props1), ConfigFormat::Properties(props2)) => {
            props1.extend(props2);
            Ok(ConfigFormat::Properties(props1))
        }
        // Merging two EnvFile configurations
        (ConfigFormat::EnvFile(mut env1), ConfigFormat::EnvFile(env2)) => {
            env1.extend(env2);
            Ok(ConfigFormat::EnvFile(env1))
        }
        // Handling cases where the configuration formats differ (Optional strategy)
        _ => Err(Error::IncompatibleFileTypes()),
    }
}

/// Merges two structured configurations together.
/// Merges two JSON configurations together.
/// Merges two structured configurations together.
pub fn to_file_type(config1: &ConfigFormat) -> Result<ConfigFileType, Error> {
    match config1 {
        ConfigFormat::Json(_)=> {

            Ok(ConfigFileType::Json)
        }
        ConfigFormat::Toml(_) => {

            Ok(ConfigFileType::Toml)
        }
        ConfigFormat::Yaml(_) => {

            Ok(ConfigFileType::Yaml)
        }

        ConfigFormat::Properties(_)=> {

            Ok(ConfigFileType::Properties)
        }

        ConfigFormat::EnvFile(_) => {

            Ok(ConfigFileType::EnvFile)
        }

        _ => Err(Error::UnsupportedFileType()),
    }
}

pub fn to_file_type_from_filename(filename: &str) -> Option<ConfigFileType> {
    // Use `Path::extension` to get the file extension from the filename
    match Path::new(filename).extension().and_then(|ext| ext.to_str()) {
        Some("json") => Some(ConfigFileType::Json),
        Some("json5") => Some(ConfigFileType::Json5),
        Some("toml") => Some(ConfigFileType::Toml),
        Some("yaml") | Some("yml") => Some(ConfigFileType::Yaml),
        Some("properties") => Some(ConfigFileType::Properties),
        Some("env") => Some(ConfigFileType::EnvFile),
        _ => None,
    }
}

/// Converts a structured configuration format back to its string representation.
pub fn convert_to_format(config : &ConfigFormat, file_type: &ConfigFileType) -> Result<String, Error> {
    match (config, file_type) {
        (ConfigFormat::Json(json), ConfigFileType::Json | ConfigFileType::Json5) => {
            serde_json::to_string_pretty(&json).map_err(|r| Error::JsonSerializationError(r))
        }
        (ConfigFormat::Json(json), ConfigFileType::Toml) => {
            let toml_val: TomlValue = serde_json::from_value::<TomlValue>(json.clone()).map_err(|e| Error::JsonSerializationError(e))?;
            toml::to_string(&toml_val).map_err(|e| Error::TomlSerializationError(e))
        }
        (ConfigFormat::Json(json), ConfigFileType::Yaml) => {
            serde_yaml::to_string(&json).map_err(|e| Error::YamlSerializationError(e))
        }
        (ConfigFormat::Json(json), ConfigFileType::Properties) => {

            let transforms = [];

            let result = json_to_key_value_pairs(
                json, '.', ".", &transforms
            );

            match result {
                Ok(key_value_map) => {
                    let result = key_value_map
                        .into_iter()
                        .map(|(k, v)| format!("{}={}", k, v))
                        .collect::<Vec<String>>()
                        .join("\n");
                    Ok(result)
                },
                Err(e) => Err(e)
            }

        }
        (ConfigFormat::Json(json), ConfigFileType::EnvFile ) => {

            let transforms = [ Case::UpperSnake];

            let result = json_to_key_value_pairs(
                json, '_', "__", &transforms
            );

            match result {
                Ok(key_value_map) => {
                    let result = key_value_map
                        .into_iter()
                        .map(|(k, v)|
                            format!("{}={}", k, v
                            ))
                        .collect::<Vec<String>>()
                        .join("\n");
                    Ok(result)
                },
                Err(e) => Err(e)
            }

        }
        _ => Err(Error::UnsupportedFileType()),
    }
}
pub fn convert_to_json(config: &ConfigFormat) -> Result<ConfigFormat, Error> {
    match config {
        ConfigFormat::Json(json) => {
            // If it's already JSON, no conversion is needed.
            Ok(ConfigFormat::Json(json.clone()))
        }
        ConfigFormat::Toml(toml_value) => {
            // Convert TOML to JSON
            let json: JsonValue = serde_json::to_value(toml_value).map_err(|e| Error::JsonSerializationError(e))?;
            Ok(ConfigFormat::Json(json))
        }
        ConfigFormat::Yaml(yaml_value) => {
            // Convert YAML to JSON
            let json: JsonValue = serde_json::to_value(yaml_value).map_err(|e| Error::JsonSerializationError(e))?;
            Ok(ConfigFormat::Json(json))
        }
        ConfigFormat::EnvFile(env_map) => {
            // Convert Properties or EnvFile (key-value pairs) to JSON
            let json: JsonValue = serde_json::to_value(env_map).map_err(|e| Error::JsonSerializationError(e))?;
            Ok(ConfigFormat::Json(json))
        }
        ConfigFormat::Properties(properties_map)  => {
            // Convert Properties or EnvFile (key-value pairs) to JSON
            let json: JsonValue = serde_json::to_value(properties_map).map_err(|e| Error::JsonSerializationError(e))?;
            Ok(ConfigFormat::Json(json))
        }
    }
}
/// Helper functions to merge JSON, TOML, YAML configurations.

/// Helper function to merge JSON objects.
fn merge_json(target: &mut JsonValue, source: &JsonValue) {
    if let (Some(target_map), Some(source_map)) = (target.as_object_mut(), source.as_object()) {
        for (key, value) in source_map {
            match target_map.get_mut(key) {
                Some(target_value) if target_value.is_object() && value.is_object() => {
                    merge_json(target_value, value);
                }
                _ => {
                    target_map.insert(key.clone(), value.clone());
                }
            }
        }
    }
}


/// Helper function to merge TOML values.
fn merge_toml(target: &mut TomlValue, source: &TomlValue) {
    if let (Some(target_table), Some(source_table)) = (target.as_table_mut(), source.as_table()) {
        for (key, value) in source_table {
            match target_table.get_mut(key) {
                Some(target_value) if target_value.is_table() && value.is_table() => {
                    merge_toml(target_value, value); // Recursively merge tables
                }
                _ => {
                    target_table.insert(key.clone(), value.clone()); // Overwrite or add new key-value pair
                }
            }
        }
    }
}

/// Helper function to merge YAML mappings.
fn merge_yaml(target: &mut YamlValue, source: &YamlValue) {
    if let (Some(target_map), Some(source_map)) = (target.as_mapping_mut(), source.as_mapping()) {
        for (key, value) in source_map {
            match target_map.get_mut(&key) {
                Some(target_value) if target_value.is_mapping() && value.is_mapping() => {
                    merge_yaml(target_value, value); // Recursively merge mappings
                }
                _ => {
                    target_map.insert(key.clone(), value.clone()); // Overwrite or add new key-value pair
                }
            }
        }
    }
}