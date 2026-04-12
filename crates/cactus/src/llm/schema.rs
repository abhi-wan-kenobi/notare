use jsonschema::{ValidationError, Validator, validator_for};

use crate::error::{Error, JsonSchemaViolation, Result};

use super::CompletionResult;

pub(super) fn compile_json_schema(schema: Option<&serde_json::Value>) -> Result<Option<Validator>> {
    let Some(schema) = schema else {
        return Ok(None);
    };

    validator_for(schema)
        .map(Some)
        .map_err(|error| Error::InvalidJsonSchema {
            message: error.to_string(),
        })
}

pub(super) fn validate_completion_result(
    result: &CompletionResult,
    validator: Option<&Validator>,
) -> Result<()> {
    let Some(validator) = validator else {
        return Ok(());
    };

    let instance = serde_json::from_str::<serde_json::Value>(&result.text).map_err(|error| {
        Error::InvalidStructuredOutput {
            message: format!("final output is not valid JSON: {error}"),
            raw_output: result.text.clone(),
        }
    })?;

    let violations = validator
        .iter_errors(&instance)
        .map(JsonSchemaViolation::from)
        .collect::<Vec<_>>();

    if violations.is_empty() {
        return Ok(());
    }

    Err(Error::JsonSchemaValidation {
        message: format!(
            "final output does not match the provided JSON schema ({} violation{})",
            violations.len(),
            if violations.len() == 1 { "" } else { "s" }
        ),
        violations,
        raw_output: result.text.clone(),
    })
}

impl From<ValidationError<'_>> for JsonSchemaViolation {
    fn from(error: ValidationError<'_>) -> Self {
        let message = error.to_string();
        let parts = error.into_parts();

        Self {
            message,
            keyword: parts.kind.keyword().to_string(),
            instance_path: parts.instance_path.as_str().to_string(),
            schema_path: parts.schema_path.as_str().to_string(),
            evaluation_path: parts.evaluation_path.as_str().to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn completion_result(text: &str) -> CompletionResult {
        CompletionResult {
            text: text.to_string(),
            ..Default::default()
        }
    }

    #[test]
    fn rejects_invalid_json_schema() {
        let error = compile_json_schema(Some(&serde_json::json!({
            "type": 42
        })))
        .unwrap_err();

        assert!(matches!(error, Error::InvalidJsonSchema { .. }));
    }

    #[test]
    fn rejects_non_json_output() {
        let validator = compile_json_schema(Some(&serde_json::json!({
            "type": "object"
        })))
        .unwrap()
        .expect("validator");

        let error =
            validate_completion_result(&completion_result("hello"), Some(&validator)).unwrap_err();

        assert!(matches!(error, Error::InvalidStructuredOutput { .. }));
    }

    #[test]
    fn rejects_schema_mismatch_with_owned_violations() {
        let validator = compile_json_schema(Some(&serde_json::json!({
            "type": "object",
            "required": ["name"],
            "properties": {
                "name": { "type": "string" }
            }
        })))
        .unwrap()
        .expect("validator");

        let error =
            validate_completion_result(&completion_result(r#"{"name": 1}"#), Some(&validator))
                .unwrap_err();

        let Error::JsonSchemaValidation { violations, .. } = error else {
            panic!("expected schema validation error");
        };
        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].keyword, "type");
        assert_eq!(violations[0].instance_path, "/name");
        assert_eq!(violations[0].schema_path, "/properties/name/type");
        assert_eq!(violations[0].evaluation_path, "/properties/name/type");
    }

    #[test]
    fn accepts_matching_json_output() {
        let validator = compile_json_schema(Some(&serde_json::json!({
            "type": "object",
            "required": ["name"],
            "properties": {
                "name": { "type": "string" }
            }
        })))
        .unwrap()
        .expect("validator");

        validate_completion_result(&completion_result(r#"{"name":"Ada"}"#), Some(&validator))
            .unwrap();
    }
}
