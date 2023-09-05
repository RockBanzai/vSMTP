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

use crate::{api::State, status::Status, Stage};
use error::{DirectiveError, ParseError};
use vsmtp_protocol::Domain;

pub mod action;
pub mod error;
pub mod rule;

pub type Result<T> = std::result::Result<T, DirectiveError>;

/// The type of email flow for a given transaction.
#[derive(Clone, PartialEq, Eq)]
pub enum FlowType {
    Inbound,
    Outbound,
    Local,
}

impl From<FlowType> for &'static str {
    fn from(value: FlowType) -> Self {
        <&'static str as From<&FlowType>>::from(&value)
    }
}

impl From<&FlowType> for &'static str {
    fn from(value: &FlowType) -> Self {
        match value {
            FlowType::Inbound => "inbound",
            FlowType::Outbound => "outbound",
            FlowType::Local => "local",
        }
    }
}

/// Result from flow computation.
/// Used as the return type of the `flow` rhai function.
#[derive(Clone, PartialEq, Eq)]
pub struct Flow {
    pub domain: Domain,
    pub r#type: FlowType,
}

impl Flow {
    /// Build a new flow object for a Rhai api.
    pub fn new_dynamic(domain: Domain, r#type: FlowType) -> rhai::Dynamic {
        rhai::Dynamic::from(Self { domain, r#type })
    }
}

/// Set of rules to be executed all the time or by flow type.
/// The "flow" is the type of transaction that is being processed.
#[derive(Debug, Clone)]
pub enum Directives {
    /// All rules are executed for any email flow type.
    Any(Vec<Directive>),
    /// Rules are split by flow type.
    Flow {
        /// When the sender domain is unknown, but a recipient is, the flow is "inbound".
        inbound: Vec<Directive>,
        /// When the sender domain is known, but a recipient is not, the flow is "outbound".
        outbound: Vec<Directive>,
        /// When the sender domain is known and a recipient is too, the flow is "local".
        local: Vec<Directive>,
    },
}

/// Cast a dynamic value into a set of directives.
pub fn directives_try_from(value: rhai::Dynamic, stage: Option<&impl Stage>) -> Result<Directives> {
    fn get_directives_from_map(map: &rhai::Map, key: &str) -> Option<Vec<Directive>> {
        map.get(key)
            // First cast the array of rules.
            .and_then(|flow| {
                // FIXME: could also be Vec<Directives>
                flow.clone().try_cast::<rhai::Array>()
            })
            // Then cast every item into a directive.
            .map(|rules| {
                rules
                    .into_iter()
                    .map(|d| d.try_cast::<Directive>().unwrap())
                    .collect()
            })
    }

    let stage = stage.map_or("unknown".into(), ToString::to_string);
    let stage_ref = &stage;

    if value.is::<rhai::Array>() {
        // The dynamic is an array of directives and does not care about flow.
        value
            .cast::<rhai::Array>()
            .into_iter()
            .map(|d| {
                d.try_cast::<Directive>().ok_or(DirectiveError::parsing(
                    ParseError::BadRuleContent(stage_ref.clone()),
                ))
            })
            .collect::<Result<Vec<Directive>>>()
            .map(Directives::Any)
    } else if value.is::<rhai::Map>() {
        // The dynamic is a map of email flow types containing directives.
        let map = value.cast::<rhai::Map>();

        Ok(Directives::Flow {
            inbound: get_directives_from_map(&map, FlowType::Inbound.into()).unwrap_or_default(),
            outbound: get_directives_from_map(&map, FlowType::Outbound.into()).unwrap_or_default(),
            local: get_directives_from_map(&map, FlowType::Local.into()).unwrap_or_default(),
        })
    } else {
        Err(DirectiveError::parsing(ParseError::BadStageContent(stage)))
    }
}

impl From<DirectiveError> for Box<rhai::EvalAltResult> {
    fn from(value: DirectiveError) -> Self {
        Self::new(rhai::EvalAltResult::ErrorParsing(
            rhai::ParseErrorType::MalformedInExpr(value.kind.to_string()),
            rhai::Position::NONE,
        ))
    }
}

/// A Rhai function that is executed when a stage is reached.
#[derive(Clone)]
pub enum Directive {
    /// Execute code that changes the behavior of the service. (e.g. deny a transaction)
    Rule {
        // PERF: switch to `rhai::ImmutableString`.
        /// Name of the rule.
        name: String,
        /// Function pointer used to execute the function's code.
        pointer: rhai::FnPtr,
    },

    /// execute code that does not need a return value.
    Action {
        // PERF: switch to `rhai::ImmutableString`.
        /// Name of the action.
        name: String,
        /// Function pointer used to execute the function's code.
        pointer: rhai::FnPtr,
    },
}

impl TryFrom<rhai::Dynamic> for Directive {
    type Error = DirectiveError;

    fn try_from(value: rhai::Dynamic) -> std::result::Result<Self, Self::Error> {
        value
            .try_cast::<Self>()
            .ok_or(Self::Error::parsing(ParseError::BadRuleContent(
                "unknown".to_string(),
            )))
    }
}

impl AsRef<str> for Directive {
    fn as_ref(&self) -> &str {
        match self {
            Self::Rule { .. } => "rule",
            Self::Action { .. } => "action",
        }
    }
}

impl Directive {
    /// Execute the directive and return the result of the evaluation.
    pub(crate) fn execute<S: Status, T: 'static>(
        &self,
        ncc: &rhai::NativeCallContext<'_>,
        state: State<T>,
    ) -> Result<S> {
        match self {
            Self::Rule { pointer, .. } => pointer
                .call_within_context::<S>(ncc, (state,))
                .map_err(|error| DirectiveError::runtime(error, self.name())),
            Self::Action { pointer, .. } => {
                // using `()` as a return value is not enough since any non-`()` return
                // at the end of an action will result in an error: we have to accept a
                // `rhai::Dynamic` by default.
                pointer
                    .call_within_context::<rhai::Dynamic>(ncc, (state,))
                    .map_err(|error| DirectiveError::runtime(error, self.name()))
                    .map(|_| S::next())
            }
        }
    }

    /// Get the name of the directive.
    pub(crate) fn name(&self) -> &str {
        match self {
            Self::Rule { name, .. } | Self::Action { name, .. } => name,
        }
    }

    /// Parse a directive from a list of rhai symbols.
    /// This function is to be called by the rhai custom syntax parser.
    pub(crate) fn parse_directive(
        symbols: &[rhai::ImmutableString],
        look_ahead: &str,
        state: &mut rhai::Dynamic,
    ) -> std::result::Result<Option<rhai::ImmutableString>, rhai::ParseError> {
        // The type of the directive is stored in the custom state during the first
        // parser iteration so the parsing is easier later.
        if symbols.len() == 1 {
            *state = rhai::Dynamic::from(symbols[0].clone());
        }

        let directive_type = state.to_string();

        match symbols.len() {
            // directive keyword -> directive name
            1 => Ok(Some("$string$".into())),
            // directive name    -> directive body
            2 => Ok(Some("$expr$".into())),
            3 => Ok(None),

            _ => Err(rhai::ParseError(
                Box::new(rhai::ParseErrorType::BadInput(
                    rhai::LexError::UnexpectedInput(format!(
                        "Improper {directive_type} declaration: the '{look_ahead}' keyword is unknown.",
                    )),
                )),
                rhai::Position::NONE,
            )),
        }
    }
}

impl std::fmt::Debug for Directive {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct(self.as_ref())
            .field("name", &self.name())
            .finish_non_exhaustive()
    }
}
