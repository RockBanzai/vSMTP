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

use vsmtp_rule_engine::{DirectiveError, Stage, Status};

/// Custom status for this rule engine.
#[allow(dead_code)]
#[derive(Debug, PartialEq, Eq, Clone, strum::AsRefStr)]
pub enum WorkingStatus {
    Next,
    Success,
    Quarantine(String),
}

/// Implement the [`Status`] trait and defining our own rules
/// for each status.
impl Status for WorkingStatus {
    fn no_rules(_: impl Stage) -> Self {
        Self::Quarantine("working-failure".to_string())
    }

    fn error(error: DirectiveError) -> Self {
        tracing::warn!(
            stage = error.stage,
            rule = error.directive,
            error = %error.kind
        );
        Self::Quarantine("working-failure".to_string())
    }

    fn next() -> Self {
        Self::Next
    }

    fn is_next(&self) -> bool {
        matches!(self, Self::Next)
    }
}
