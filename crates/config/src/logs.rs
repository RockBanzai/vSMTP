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

use tracing_subscriber::filter::LevelFilter as TracingLevelFilter;

#[serde_with::serde_as]
#[derive(Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub struct Logs {
    #[serde(default = "Logs::default_log_level")]
    #[serde_as(as = "serde_with::DisplayFromStr")]
    pub default_level: TracingLevelFilter,
    /// Customize the log level of the different part of the program.
    ///
    /// See <https://docs.rs/tracing-subscriber/0.3.15/tracing_subscriber/filter/struct.EnvFilter.html>
    #[serde(default)]
    #[serde_as(as = "serde_with::Map<serde_with::Same, serde_with::DisplayFromStr>")]
    pub levels: Vec<(String, TracingLevelFilter)>,
}

impl Logs {
    const fn default_log_level() -> TracingLevelFilter {
        TracingLevelFilter::WARN
    }
}

impl Default for Logs {
    fn default() -> Self {
        Self {
            default_level: Self::default_log_level(),
            levels: Vec::default(),
        }
    }
}
