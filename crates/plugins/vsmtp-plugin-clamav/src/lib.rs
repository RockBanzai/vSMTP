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

mod api;

/// Export the `cache_plugin` module.
#[allow(improper_ctypes_definitions)]
#[no_mangle]
#[inline]
pub extern "C" fn module_entrypoint() -> rhai::Shared<rhai::Module> {
    // The seed must be the same as the one used in the program that will
    // load this module.
    rhai::config::hashing::set_ahash_seed(Some([1, 2, 3, 4]))
        .expect("ahash seed as already been set once");

    #[cfg(debug_assertions)]
    {
        dbg!("Map typeid: {:?}", std::any::TypeId::of::<rhai::Map>());
    }

    rhai::exported_module!(api::clamav).into()
}
