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

#![doc = include_str!("../README.md")]
pub mod crypto;
pub mod env;
pub mod process;
pub mod time;

#[must_use]
pub fn crypto() -> (String, rhai::Shared<rhai::Module>) {
    (
        "crypto".to_string(),
        rhai::Shared::new(rhai::exported_module!(crypto::api)),
    )
}

#[must_use]
pub fn env() -> (String, rhai::Shared<rhai::Module>) {
    (
        "env".to_string(),
        rhai::Shared::new(rhai::exported_module!(env::api)),
    )
}

#[must_use]
pub fn process() -> (String, rhai::Shared<rhai::Module>) {
    (
        "process".to_string(),
        rhai::Shared::new(rhai::exported_module!(process::api)),
    )
}

#[must_use]
pub fn time() -> (String, rhai::Shared<rhai::Module>) {
    (
        "time".to_string(),
        rhai::Shared::new(rhai::exported_module!(time::api)),
    )
}
