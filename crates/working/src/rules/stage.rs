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

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, strum::Display, strum::EnumString)]
#[strum(serialize_all = "snake_case")]
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
