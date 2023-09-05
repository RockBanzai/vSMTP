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

use vsmtp_protocol::Reply;
use vsmtp_rule_engine::{DirectiveError, Stage, Status};

/// Custom status for this rule engine.
#[allow(dead_code)]
#[derive(Debug, PartialEq, Eq, Clone)]
pub enum ReceiverStatus {
    Next,
    // TODO:  Faccept(Option<vsmtp_common::Reply>),
    Accept(Option<Reply>),
    Deny(Option<Reply>),
    // TODO: Reject(Option<vsmtp_common::Reply>),
    Quarantine(String, Option<Reply>),
}

/// Implement the [`Status`] trait and defining our own rules
/// for each status.
impl Status for ReceiverStatus {
    fn no_rules(_: impl Stage) -> Self {
        Self::Deny(None)
    }

    fn error(error: DirectiveError) -> Self {
        tracing::error!(
            stage = error.stage,
            rule = error.directive,
            error = %error.kind
        );
        Self::Deny(None)
    }

    fn next() -> Self {
        Self::Next
    }
}

impl std::fmt::Display for ReceiverStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{:?}",
            match self {
                Self::Next => "next",
                // ReceiverStatus::Faccept(_) => "faccept",
                Self::Accept(_) => "accept",
                Self::Deny(_) => "deny",
                // ReceiverStatus::Reject(_) => "reject",
                Self::Quarantine(_, _) => "quarantine",
            }
        )
    }
}
