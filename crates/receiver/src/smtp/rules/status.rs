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

use crate::smtp::session::{default_accept, default_deny};

/// Custom status for this rule engine.
#[allow(dead_code)]
#[derive(PartialEq, Eq, Clone)]
pub enum ReceiverStatus {
    Next,
    // TODO:  Faccept(Option<vsmtp_common::Reply>),
    Accept(Option<Reply>),
    Deny(Option<Reply>),
    // TODO: Reject(Option<vsmtp_common::Reply>),
    Quarantine(String, Option<Reply>),
}

impl std::fmt::Debug for ReceiverStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Next => write!(f, "Next"),
            Self::Accept(arg0) => f
                .debug_tuple("Accept")
                .field(&arg0.as_ref().unwrap_or(&default_accept()).to_string())
                .finish(),
            Self::Deny(arg0) => f
                .debug_tuple("Deny")
                .field(&arg0.as_ref().unwrap_or(&default_deny()).to_string())
                .finish(),
            Self::Quarantine(arg0, arg1) => f
                .debug_struct("Quarantine")
                .field("directory", arg0)
                .field(
                    "reply",
                    &arg1.as_ref().unwrap_or(&default_accept()).to_string(),
                )
                .finish(),
        }
    }
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

    fn is_next(&self) -> bool {
        matches!(self, Self::Next)
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
