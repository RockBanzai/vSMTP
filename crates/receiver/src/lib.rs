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

pub mod smtp {
    /// SMTP receiver service configuration.
    pub mod config;
    /// SMTP receiver rules settings and rhai apis.
    pub mod rules;
    pub mod server;
    pub mod session;
}
