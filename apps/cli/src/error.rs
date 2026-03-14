use miette::Diagnostic;
use thiserror::Error;

pub type CliResult<T> = Result<T, CliError>;

#[derive(Debug, Error, Diagnostic)]
pub enum CliError {
    #[error("{0}")]
    Message(String),

    #[error("{name} is required")]
    RequiredArgument {
        name: &'static str,
        #[help]
        hint: Option<String>,
    },

    #[error("invalid {name} '{value}': {reason}")]
    InvalidArgument {
        name: &'static str,
        value: String,
        reason: String,
    },

    #[error("{action} failed: {reason}")]
    ExternalActionFailed {
        action: &'static str,
        reason: String,
    },

    #[error("{action} failed: {reason}")]
    OperationFailed {
        action: &'static str,
        reason: String,
    },

    #[error("{what} not found")]
    NotFound {
        what: String,
        #[help]
        hint: Option<String>,
    },
}

impl CliError {
    pub fn msg(message: impl Into<String>) -> Self {
        Self::Message(message.into())
    }

    pub fn required_argument(name: &'static str) -> Self {
        Self::RequiredArgument { name, hint: None }
    }

    pub fn required_argument_with_hint(name: &'static str, hint: impl Into<String>) -> Self {
        Self::RequiredArgument {
            name,
            hint: Some(hint.into()),
        }
    }

    pub fn invalid_argument(
        name: &'static str,
        value: impl Into<String>,
        reason: impl Into<String>,
    ) -> Self {
        Self::InvalidArgument {
            name,
            value: value.into(),
            reason: reason.into(),
        }
    }

    pub fn external_action_failed(action: &'static str, reason: impl Into<String>) -> Self {
        Self::ExternalActionFailed {
            action,
            reason: reason.into(),
        }
    }

    pub fn operation_failed(action: &'static str, reason: impl Into<String>) -> Self {
        Self::OperationFailed {
            action,
            reason: reason.into(),
        }
    }

    pub fn not_found(what: impl Into<String>, hint: Option<String>) -> Self {
        Self::NotFound {
            what: what.into(),
            hint,
        }
    }
}

impl From<String> for CliError {
    fn from(message: String) -> Self {
        Self::Message(message)
    }
}

impl From<&str> for CliError {
    fn from(message: &str) -> Self {
        Self::msg(message)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invalid_argument_has_structured_fields() {
        let error = CliError::invalid_argument("--language", "xx", "unknown code");

        match error {
            CliError::InvalidArgument {
                name,
                value,
                reason,
            } => {
                assert_eq!(name, "--language");
                assert_eq!(value, "xx");
                assert_eq!(reason, "unknown code");
            }
            _ => panic!("expected invalid argument variant"),
        }
    }

    #[test]
    fn not_found_includes_hint_in_display() {
        let error = CliError::not_found(
            "model 'foo'",
            Some("Run `char model list` to see available models.".to_string()),
        );

        let rendered = error.to_string();
        assert!(rendered.contains("model 'foo' not found"));
    }
}
