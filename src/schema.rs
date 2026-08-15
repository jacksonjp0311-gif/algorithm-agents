use serde_json::Value;

use crate::error::AgentError;

pub fn validate_against_schema(schema: &Value, instance: &Value) -> Result<(), AgentError> {
    validate(schema, instance, "$")
}

fn validate(schema: &Value, instance: &Value, path: &str) -> Result<(), AgentError> {
    if let Some(enum_vals) = schema.get("enum").and_then(|v| v.as_array()) {
        if !enum_vals.iter().any(|allowed| allowed == instance) {
            return Err(AgentError::Schema(format!(
                "{path} is not one of the allowed enum values"
            )));
        }
    }

    if let Some(expected) = schema.get("type").and_then(|v| v.as_str()) {
        if !type_matches(expected, instance) {
            return Err(AgentError::Schema(format!(
                "{path} expected type {expected}"
            )));
        }
    }

    match instance {
        Value::Object(map) => {
            if let Some(required) = schema.get("required").and_then(|v| v.as_array()) {
                for key in required {
                    let Some(name) = key.as_str() else { continue };
                    if !map.contains_key(name) {
                        return Err(AgentError::Schema(format!(
                            "{path} missing required field `{name}`"
                        )));
                    }
                }
            }
            if let Some(properties) = schema.get("properties").and_then(|v| v.as_object()) {
                for (key, subschema) in properties {
                    if let Some(value) = map.get(key) {
                        validate(subschema, value, &format!("{path}.{key}"))?;
                    }
                }
            }
        }
        Value::Array(items) => {
            if let Some(min) = schema.get("minItems").and_then(|v| v.as_u64()) {
                if (items.len() as u64) < min {
                    return Err(AgentError::Schema(format!(
                        "{path} needs at least {min} items"
                    )));
                }
            }
            if let Some(max) = schema.get("maxItems").and_then(|v| v.as_u64()) {
                if (items.len() as u64) > max {
                    return Err(AgentError::Schema(format!(
                        "{path} has more than {max} items"
                    )));
                }
            }
            if let Some(item_schema) = schema.get("items") {
                for (index, item) in items.iter().enumerate() {
                    validate(item_schema, item, &format!("{path}[{index}]"))?;
                }
            }
        }
        Value::String(text) => {
            if let Some(min) = schema.get("minLength").and_then(|v| v.as_u64()) {
                if (text.chars().count() as u64) < min {
                    return Err(AgentError::Schema(format!(
                        "{path} is shorter than {min} characters"
                    )));
                }
            }
        }
        _ => {}
    }
    Ok(())
}

fn type_matches(expected: &str, instance: &Value) -> bool {
    match expected {
        "object" => instance.is_object(),
        "array" => instance.is_array(),
        "string" => instance.is_string(),
        "boolean" => instance.is_boolean(),
        "integer" => instance.as_i64().is_some(),
        "number" => instance.as_f64().is_some(),
        "null" => instance.is_null(),
        _ => true,
    }
}

pub fn load_json_file(path: &std::path::Path) -> Result<Value, AgentError> {
    let raw = std::fs::read_to_string(path)
        .map_err(|error| AgentError::Io(format!("{}: {error}", path.display())))?;
    serde_json::from_str(&raw)
        .map_err(|error| AgentError::Schema(format!("{}: {error}", path.display())))
}
