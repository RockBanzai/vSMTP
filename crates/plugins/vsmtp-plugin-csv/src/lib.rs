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

//! # vSMTP CSV plugin

#![doc(html_no_source)]
#![deny(missing_docs)]
#![deny(unsafe_code)]
//
#![warn(rust_2018_idioms)]
#![warn(clippy::all)]
#![warn(clippy::pedantic)]
#![warn(clippy::nursery)]
#![warn(clippy::cargo)]

mod api;

use rhai::{config::hashing::set_ahash_seed, exported_module, Module, Shared};

/// `rhai-dylib` will fetch this symbol to load the module into `vSMTP`.
///
/// # Panics
///
/// * the `rhai` hashing seed cannot be set.
#[allow(improper_ctypes_definitions)]
#[allow(unsafe_code)]
#[no_mangle]
#[inline]
pub extern "C" fn module_entrypoint() -> Shared<Module> {
    set_ahash_seed(Some([1, 2, 3, 4])).unwrap();

    #[cfg(debug_assertions)]
    {
        // Checking if TypeIDs are the same as the main program.
        dbg!("Map typeid: {:?}", std::any::TypeId::of::<rhai::Map>());
    }

    exported_module!(api::csv_api).into()
}
