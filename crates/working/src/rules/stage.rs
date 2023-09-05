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

use vsmtp_rule_engine::Stage;

// FIXME: review those stages, are they relevant ?
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum WorkingStage {
    PostQueue,
}

impl Stage for WorkingStage {
    fn hook(&self) -> &'static str {
        match self {
            Self::PostQueue => "on_post_queue",
        }
    }

    fn stages() -> &'static [&'static str] {
        &["post_queue"]
    }
}

impl std::str::FromStr for WorkingStage {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "post_queue" => Ok(Self::PostQueue),
            _ => Err(()),
        }
    }
}

impl std::fmt::Display for WorkingStage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Self::PostQueue => "post_queue",
            }
        )
    }
}
