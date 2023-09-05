/*
 * vSMTP mail transfer agent
 *
 * Copyright (C) 2003 - viridIT SAS
 * Licensed under the Elastic License 2.0
 *
 * You should have received a copy of the Elastic License 2.0 along with
 * this program. If not, see https://www.elastic.co/licensing/elastic-license.
 *
 */

// TODO: replace this by Box<EvalAltResult>, since those errors are only
//       used in Rhai context.
/// Error emitted by a directive.
#[allow(clippy::module_name_repetitions)]
#[derive(Debug)]
pub struct DirectiveError {
    /// The kind of error raised.
    pub kind: DirectiveErrorKind,
    // NOTE: to prevent doing complicated things with generics,
    //       the Display trait is used to convert the stage to a string.
    /// The stage in which the error was raised.
    pub stage: Option<String>,
    /// The directive in which the error was raised.
    pub directive: Option<String>,
}

impl std::fmt::Display for DirectiveError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "directive '{}' at stage '{}': {}",
            self.directive.as_ref().unwrap_or(&String::from("unknown")),
            self.stage.as_ref().unwrap_or(&String::from("unknown")),
            self.kind
        )
    }
}

impl DirectiveError {
    /// Generate a IO error.
    #[must_use]
    pub const fn read(error: std::io::Error) -> Self {
        Self {
            kind: DirectiveErrorKind::Read(error),
            stage: None,
            directive: None,
        }
    }

    /// Generate a compilation error.
    #[must_use]
    pub fn compile(error: Box<rhai::EvalAltResult>) -> Self {
        Self {
            kind: DirectiveErrorKind::Compile(error),
            stage: None,
            directive: None,
        }
    }

    /// Generate a rhai/core parsing error.
    #[must_use]
    pub const fn parsing(error: ParseError) -> Self {
        Self {
            kind: DirectiveErrorKind::Parse(error),
            stage: None,
            directive: None,
        }
    }

    /// Generate a rhai runtime error.
    #[must_use]
    pub fn runtime(error: Box<rhai::EvalAltResult>, directive: &str) -> Self {
        Self {
            kind: DirectiveErrorKind::Runtime(error),
            stage: None,
            directive: Some(directive.to_string()),
        }
    }

    /// Generate a rhai runtime error.
    #[must_use]
    pub fn get_rules(error: Box<rhai::EvalAltResult>, stage: &str) -> Self {
        Self {
            kind: DirectiveErrorKind::GetRulesRuntime(error),
            stage: Some(stage.to_string()),
            directive: None,
        }
    }
}

/// Error types for a directive error.
#[allow(clippy::module_name_repetitions)]
#[must_use]
#[derive(Debug, thiserror::Error)]
pub enum DirectiveErrorKind {
    /// The Rhai engine emitted an error at runtime.
    #[error("rhai execution produced an error: {0}")]
    Runtime(#[from] Box<rhai::EvalAltResult>),
    /// The Rhai engine emitted an error at runtime.
    #[error("rhai execution produced an error when getting rules: {0}")]
    GetRulesRuntime(Box<rhai::EvalAltResult>),
    /// Failed to read directives from a file.
    #[error("failed to read rules: {0}")]
    Read(#[from] std::io::Error),
    /// Failed to compile directives from a file.
    #[error("failed to compile rules: {0}")]
    Compile(Box<rhai::EvalAltResult>),
    /// Failed to parse directives.
    #[error("failed to parse rules: {0}")]
    Parse(#[from] ParseError),
}

/// Parsing error emitted by directives.
#[allow(clippy::module_name_repetitions)]
#[must_use]
#[derive(Debug, thiserror::Error)]
pub enum ParseError {
    /// The Rhai engine emitted an error at compile time.
    #[error("rhai emitted an error: {0}")]
    Rhai(rhai::ParseError),
    /// A stage is declared but as not been registered in the rule engine.
    #[error("the '{0}' stage does not exist")]
    UnknownStage(/* name of the stage */ String),
    /// The stage content is invalid.
    #[error("the '{0}' stage content must be an array of rules or a map of the email flow")]
    BadStageContent(/* name of the stage */ String),
    /// The rule syntax is invalid.
    #[error(
        "a rule in the '{0}' stage is not declared properly. You must use the rhai anonymous function syntax: `rule/action \"{0}\" || {{}}`"
    )]
    BadRuleContent(/* name of the stage */ String),
    /// The given status failed to be parsed.
    #[error("the '{0}' status is invalid")]
    BadStatus(/* given status */ String),
}
