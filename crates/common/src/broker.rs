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

/// The "delivery" and "deferred" queues exists, but are not **unique**
/// they are created on demand, and are named after the systems route.
#[derive(strum::AsRefStr)]
#[strum(serialize_all = "kebab-case")]
pub enum Queue {
    ToWorking,
    Dead,
    Quarantine,
    NoRoute,
    DSN,
}

#[derive(Clone, PartialEq, Eq, Hash, strum::AsRefStr)]
#[strum(serialize_all = "kebab-case")]
pub enum Exchange {
    Delivery,
    DelayedDeferred,
    Quarantine,
}
