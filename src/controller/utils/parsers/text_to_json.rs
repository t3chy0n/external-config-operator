use serde_json::Value as JsonValue;
use std::io::Cursor;
use std::collections::HashMap;

use toml::Value as TomlValue;
use serde_yaml::Value as YamlValue;
use crate::contract::lib::Error;
use crate::controller::utils::file_format::ConfigFormat;
use env_file_reader::read_str;
use crate::controller::utils::parsers::key_value_to_json;

/// Parses `.properties` files using `java-properties` and converts them into a nested JSON structure.
fn try_parse_from_properties(content: &str) -> Result<ConfigFormat, Error> {
    let reader = Cursor::new(content);

    // Use `java-properties` to read key-value pairs into a HashMap
    let properties: HashMap<String, String> = java_properties::read(reader).map_err(|e| Error::PropertiesSerializationError(e))?;
    let map = key_value_to_json::key_value_pairs_to_json(properties, '.', None);

    Ok(ConfigFormat::Json(JsonValue::Object(map)))

}

// Function to parse the env file and generate a nested JSON structure
fn try_parse_from_env(content: &str) -> Result<ConfigFormat, Error> {

    let reader = Cursor::new(content);

    // Use `env_file_reader` to read key-value pairs into a HashMap
    let env_vars: HashMap<String, String> = read_str(content).map_err(|e| Error::EnvFileSerializationError(e))?;
    let map = key_value_to_json::key_value_pairs_to_json(env_vars, '_', None);

    Ok(ConfigFormat::Json(JsonValue::Object(map)))
}

/// Tries to parse the configuration file content into a JSON structure.
fn try_parse_from_json(content: &str) -> Result<ConfigFormat, Error> {
    let json: JsonValue = serde_json::from_str::<JsonValue>(content).map_err(|_| Error::ParseError())?;
    if !json.is_object() {
        return Err(Error::ParseError())
    }
    Ok(ConfigFormat::Json(json))
}

fn try_parse_from_json5(content: &str) -> Result<ConfigFormat, Error> {
    let json: JsonValue = json5::from_str::<JsonValue>(content).map(serde_json::Value::from).map_err(|_| Error::ParseError())?;
    if !json.is_object() {
        return Err(Error::ParseError())
    }
    Ok(ConfigFormat::Json(json))
}

/// Tries to parse the configuration file content as TOML.
fn try_parse_from_toml(content: &str) -> Result<ConfigFormat, Error> {
    let toml_val: TomlValue = toml::from_str(content).map_err(|_| Error::ParseError())?;
    let json = serde_json::to_value(toml_val).unwrap(); // Convert TOML to JSON
    if !json.is_object() {
        return Err(Error::ParseError())
    }
    Ok(ConfigFormat::Json(json))
}

/// Tries to parse the configuration file content as YAML.
fn try_parse_from_yaml(content: &str) -> Result<ConfigFormat, Error> {
    let yaml_val: YamlValue = serde_yaml::from_str(content).map_err(|_| Error::ParseError())?;
    let json = serde_json::to_value(yaml_val).unwrap(); // Convert YAML to JSON
    if !json.is_object() {
        return Err(Error::ParseError())
    }
    Ok(ConfigFormat::Json(json))
}


/// Chain of parsers. Each parser tries to parse the content until one succeeds.
pub fn try_parse_file_to_json(content: &str) -> Result<ConfigFormat, Error> {
    try_parse_from_json(content)
        .or_else(|_| try_parse_from_json5(content))
        .or_else(|_| try_parse_from_toml(content))
        .or_else(|_| try_parse_from_yaml(content))
        .or_else(|_| try_parse_from_env(content))
        .or_else(|_| try_parse_from_properties(content))
        .map_err(|_| Error::UnsupportedFileType())

}