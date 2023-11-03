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

#[derive(Default, Debug, Copy, Clone, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Requirement {
    #[default]
    Required,
    Optional,
    Disabled,
}

#[derive(Default, Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct Tls {
    #[serde(default)]
    pub starttls: Requirement,
}
