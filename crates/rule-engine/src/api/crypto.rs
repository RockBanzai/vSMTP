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

use rhai::plugin::{
    mem, Dynamic, FnAccess, FnNamespace, ImmutableString, Module, NativeCallContext,
    PluginFunction, RhaiResult, TypeId,
};
use vsmtp_auth::dkim as backend;

pub use crypto::*;

#[rhai::plugin::export_module]
mod crypto {

    /// # rhai-autodocs:index:1
    #[rhai_fn(return_raw)]
    pub fn load_pem_rsa_pkcs8_file(
        filepath: &str,
    ) -> Result<std::sync::Arc<backend::PrivateKey>, Box<rhai::EvalAltResult>> {
        match <rsa::RsaPrivateKey as rsa::pkcs8::DecodePrivateKey>::read_pkcs8_pem_file(filepath) {
            Ok(key) => Ok(std::sync::Arc::new(backend::PrivateKey::Rsa(Box::new(key)))),
            Err(e) => Err(e.to_string().into()),
        }
    }

    /// # rhai-autodocs:index:2
    #[rhai_fn(return_raw)]
    pub fn load_pem_rsa_pkcs1_file(
        filepath: &str,
    ) -> Result<std::sync::Arc<backend::PrivateKey>, Box<rhai::EvalAltResult>> {
        match <rsa::RsaPrivateKey as rsa::pkcs1::DecodeRsaPrivateKey>::read_pkcs1_pem_file(filepath)
        {
            Ok(key) => Ok(std::sync::Arc::new(backend::PrivateKey::Rsa(Box::new(key)))),
            Err(e) => Err(e.to_string().into()),
        }
    }

    /// # rhai-autodocs:index:3
    #[rhai_fn(return_raw)]
    pub fn load_pem_ed_pkcs8_file(
        filepath: &str,
    ) -> Result<std::sync::Arc<backend::PrivateKey>, Box<rhai::EvalAltResult>> {
        let content = std::fs::read_to_string(filepath).map_err(|e| e.to_string())?;
        let (_type_label, data) =
            pem_rfc7468::decode_vec(content.as_bytes()).map_err(|e| e.to_string())?;

        let ed25519 =
            ring_compat::ring::signature::Ed25519KeyPair::from_pkcs8_maybe_unchecked(&data)
                .map_err(|e| e.to_string())?;

        Ok(std::sync::Arc::new(backend::PrivateKey::Ed25519(Box::new(
            ed25519,
        ))))
    }
}
