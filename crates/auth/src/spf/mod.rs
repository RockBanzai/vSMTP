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

///
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub enum Details {
    ///
    Mechanism(String),
    ///
    Problem(String),
}

/// The result of evaluating an SPF query.
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub struct Result {
    ///
    pub result: String,
    ///
    pub details: Details,
}

impl From<viaspf::QueryResult> for Result {
    fn from(other: viaspf::QueryResult) -> Self {
        Self {
            result: other.spf_result.to_string(),
            details: other.cause.map_or_else(
                || Details::Mechanism("default".to_string()),
                |cause| match cause {
                    viaspf::SpfResultCause::Match(mechanism) => {
                        Details::Mechanism(mechanism.to_string())
                    }
                    viaspf::SpfResultCause::Error(error) => Details::Problem(error.to_string()),
                },
            ),
        }
    }
}
