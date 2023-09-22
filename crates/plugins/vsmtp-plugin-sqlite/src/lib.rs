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

//! # `vSMTP` Sqlite plugin

#![doc(html_no_source)]
#![deny(missing_docs)]
//

// #![warn(clippy::restriction)]
// restriction we ignore
#![allow(
    clippy::blanket_clippy_restriction_lints,
    clippy::implicit_return,
    clippy::missing_docs_in_private_items,
    clippy::shadow_reuse
)]

mod api;

#[cfg(test)]
mod tests;

/// Entry point of the `sqlite` plugin
///
/// # Panics
///
/// * the `rhai` hashing seed cannot be set.
#[allow(unsafe_code)]
#[allow(improper_ctypes_definitions)]
#[no_mangle]
pub extern "C" fn module_entrypoint() -> rhai::Shared<rhai::Module> {
    // The seed must be the same as the one used in the program that will
    // load this module.
    rhai::config::hashing::set_ahash_seed(Some([1, 2, 3, 4])).unwrap();

    #[cfg(debug_assertions)]
    {
        dbg!("Map typeid: {:?}", std::any::TypeId::of::<rhai::Map>());
    }

    rhai::exported_module!(api::sqlite_api).into()
}
