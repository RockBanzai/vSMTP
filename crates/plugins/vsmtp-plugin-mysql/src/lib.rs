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

//! # vSMTP MySQL plugin

#![doc(html_no_source)]
#![deny(missing_docs)]
#![deny(unsafe_code)]
//
#![warn(rust_2018_idioms)]
#![warn(clippy::all)]
#![warn(clippy::pedantic)]
#![warn(clippy::nursery)]
#![warn(clippy::cargo)]
// #![warn(clippy::restriction)]
// restriction we ignore
#![allow(
    clippy::blanket_clippy_restriction_lints,
    clippy::implicit_return,
    clippy::missing_docs_in_private_items,
    clippy::shadow_reuse
)]
//
mod api;

use rhai::{config::hashing::set_ahash_seed, exported_module, Module, Shared};
// use vsmtp_rule_engine::rhai;

/// Entry point of the `vSMTP` plugin
///
/// # Panics
///
/// * the `rhai` hashing seed cannot be set.
#[allow(unsafe_code)]
#[allow(improper_ctypes_definitions)]
#[no_mangle]
#[inline]
pub extern "C" fn module_entrypoint() -> Shared<Module> {
    #[allow(clippy::unwrap_used)]
    set_ahash_seed(Some([1, 2, 3, 4])).unwrap();

    #[cfg(debug_assertions)]
    #[allow(clippy::dbg_macro, clippy::std_instead_of_core)]
    {
        // Checking if TypeIDs are the same as the main program.
        dbg!("Map typeid: {:?}", std::any::TypeId::of::<rhai::Map>());
    }

    exported_module!(api::mysql_api).into()
}
