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

use vsmtp_common::dns_resolver::DnsResolver;

// mod fs;
mod auth;
mod crypto;
mod dkim;
mod dmarc;
mod dns;
mod envelop;
mod fs;
mod iprev;
mod logging;
mod mail_context;
mod message;
mod net;
mod process;
mod sasl;
mod spf;
mod time;
mod utils;

/// Error produced by Rust API function calls.
pub type Result<T> = std::result::Result<T, Box<rhai::EvalAltResult>>;

/// Context passed to Rust API function calls.
/// This is used to easily read and write the content without boilerplate.
#[derive(Debug)]
pub struct State<T>(rhai::Shared<rhai::Locked<T>>);

// Needed because the base implementation of the `Clone` derive macro adds the trait to ALL
// generic types. (thus, forcing `T` to be clone, which is not what we want)
impl<T> Clone for State<T> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<T> From<T> for State<T> {
    fn from(value: T) -> Self {
        Self(rhai::Shared::new(std::sync::RwLock::new(value)))
    }
}

impl<T> State<T> {
    /// Read the value of the state.
    pub fn read<O>(&self, f: impl FnOnce(&T) -> O) -> O {
        f(&self.0.read().expect("state is poisoned"))
    }

    /// Write to the state.
    pub fn write<O>(&self, f: impl FnOnce(&mut T) -> O) -> O {
        f(&mut self.0.write().expect("state is poisoned"))
    }
}

impl<T: std::fmt::Debug> State<T> {
    #[must_use]
    pub fn into_inner(self) -> T {
        std::sync::Arc::try_unwrap(self.0)
            .expect("state has multiple string references")
            .into_inner()
            .expect("state is poisoned")
    }
}

// FIXME: This can lead to bugs if you try to replace the Arc within the state!
//        Since the implementation of an arc/mutex for the context is only there because the
//        Rhai engine needs it, a single rule engine is created per thread, so we don't
//        care for now.
/// SAFETY: `State` contents are wrapped in thread safe primitives.
unsafe impl<T> Send for State<T> {}
/// SAFETY: `State` contents are wrapped in thread safe primitives.
unsafe impl<T> Sync for State<T> {}

/// Modules that enable access and mutation on the email and it's context.
#[must_use]
pub fn smtp_modules() -> [(String, rhai::Shared<rhai::Module>); 3] {
    [
        (
            "message".to_string(),
            rhai::Shared::new(rhai::exported_module!(message)),
        ),
        (
            "envelop".to_string(),
            rhai::Shared::new(rhai::exported_module!(envelop)),
        ),
        (
            "context".to_string(),
            rhai::Shared::new(rhai::exported_module!(mail_context)),
        ),
    ]
}

#[must_use]
pub fn msa_modules() -> [(String, rhai::Shared<rhai::Module>); 1] {
    [(
        "sasl".to_string(),
        rhai::Shared::new(rhai::exported_module!(sasl)),
    )]
}

#[must_use]
pub fn process_modules() -> [(String, rhai::Shared<rhai::Module>); 1] {
    [(
        "process".to_string(),
        rhai::Shared::new(rhai::exported_module!(process)),
    )]
}

/// Network related modules.
#[must_use]
pub fn net_modules() -> [(String, rhai::Shared<rhai::Module>); 2] {
    [
        (
            "net".to_string(),
            rhai::Shared::new(rhai::exported_module!(net)),
        ),
        (
            "dns".to_string(),
            rhai::Shared::new(rhai::exported_module!(dns)),
        ),
    ]
}

#[must_use]
pub fn server_auth() -> [(String, rhai::Shared<rhai::Module>); 5] {
    [
        (
            "auth".to_string(),
            rhai::Shared::new(rhai::exported_module!(auth)),
        ),
        (
            "iprev".to_string(),
            rhai::Shared::new(rhai::exported_module!(iprev)),
        ),
        (
            "spf".to_string(),
            rhai::Shared::new(rhai::exported_module!(spf)),
        ),
        (
            "dkim".to_string(),
            rhai::Shared::new(rhai::exported_module!(dkim)),
        ),
        (
            "dmarc".to_string(),
            rhai::Shared::new(rhai::exported_module!(dmarc)),
        ),
    ]
}

#[must_use]
pub fn crypto_modules() -> [(String, rhai::Shared<rhai::Module>); 1] {
    [(
        "crypto".to_string(),
        rhai::Shared::new(rhai::exported_module!(crypto)),
    )]
}

fn deserialize_dns_resolver<'de, D>(
    deserializer: D,
) -> std::result::Result<std::sync::Arc<DnsResolver>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let private_key = <rhai::Dynamic as serde::Deserialize>::deserialize(deserializer)?;

    private_key
        .clone()
        .try_cast::<rhai::Shared<DnsResolver>>()
        .map_or_else(
            || match rhai::serde::from_dynamic(&private_key) {
                Ok(private_key) => Ok(std::sync::Arc::new(private_key)),
                Err(e) => Err(serde::de::Error::custom(format!(
                    "failed to parse dns resolver: {e}"
                ))),
            },
            Ok,
        )
}
