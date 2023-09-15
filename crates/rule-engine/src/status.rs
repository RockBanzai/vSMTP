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

use crate::{DirectiveError, Stage};

/// "Hooks" used to identify when to run a batch of [`crate::Directives`].
pub trait Status: std::fmt::Debug + Clone + PartialEq + Send + Sync + 'static {
    /// The status to return when no rules are found
    /// for a given stage.
    fn no_rules(stage: impl Stage) -> Self;

    /// The status to return when the Rhai engine
    /// emits an error.
    fn error(context: DirectiveError) -> Self;

    /// The status to return when the rule engine jumps
    /// to the next directive.
    fn next() -> Self;
}
