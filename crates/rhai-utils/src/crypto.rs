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

use vsmtp_auth::TlsCertificate;
use vsmtp_auth::TlsPrivateKey;

pub type Result<T> = std::result::Result<T, Box<rhai::EvalAltResult>>;

/// Utility functions to load certificates and keys from file.
///
/// This modules is accessible in filtering AND configuration scripts.
#[rhai::plugin::export_module]
pub mod api {
    /// Load a RSA private key from a PEM file, with the format **pkcs8*.
    ///
    /// # Arguments
    ///
    /// * `filepath` - The absolute path to the file containing the private key.
    ///
    /// # Example
    ///
    /// ```
    /// const my_key = crypto::load_pem_rsa_pkcs8("/etc/vsmtp/keys/my_key.pem");
    /// ```
    ///
    /// # rhai-autodocs:index:1
    #[rhai_fn(return_raw)]
    pub fn load_pem_rsa_pkcs8(filepath: &str) -> Result<rhai::Dynamic> {
        let private_key =
            TlsPrivateKey::load_pem_rsa_pkcs8_file(filepath).map_err(|e| e.to_string())?;
        rhai::serde::to_dynamic(&private_key).map_err(|e| e.to_string().into())
    }

    /// Load a RSA private key from a PEM file, with the format **pkcs1**.
    ///
    /// # Arguments
    ///
    /// * `filepath` - The absolute path to the file containing the private key.
    ///
    /// # Example
    ///
    /// ```
    /// const my_key = crypto::load_pem_rsa_pkcs1("/etc/vsmtp/keys/my_key.pem");
    /// ```
    ///
    /// # rhai-autodocs:index:2
    #[rhai_fn(return_raw)]
    pub fn load_pem_rsa_pkcs1(filepath: &str) -> Result<rhai::Dynamic> {
        let private_key =
            TlsPrivateKey::load_pem_rsa_pkcs1_file(filepath).map_err(|e| e.to_string())?;
        rhai::serde::to_dynamic(&private_key).map_err(|e| e.to_string().into())
    }

    /// Load an Ed25519 private key from a PEM file.
    ///
    /// # Arguments
    ///
    /// * `filepath` - The absolute path to the file containing the private key.
    ///
    /// # Example
    ///
    /// ```
    /// const my_key = crypto::load_pem_ed_pkcs8("/etc/vsmtp/keys/my_key.pem");
    /// ```
    ///
    /// # rhai-autodocs:index:3
    #[rhai_fn(return_raw)]
    pub fn load_pem_ed_pkcs8(filepath: &str) -> Result<rhai::Dynamic> {
        let private_key =
            TlsPrivateKey::load_pem_ed_pkcs8_file(filepath).map_err(|e| e.to_string())?;
        rhai::serde::to_dynamic(&private_key).map_err(|e| e.to_string().into())
    }

    /// Load a certificate from a PEM file.
    ///
    /// # Arguments
    ///
    /// * `filepath` - The absolute path to the file containing the certificate.
    ///
    /// # Example
    ///
    /// ```
    /// const my_cert = crypto::load_cert("/etc/vsmtp/cert/mydomain.tld.crt");
    /// ```
    /// # rhai-autodocs:index:4
    #[rhai_fn(return_raw)]
    pub fn load_cert(filepath: &str) -> Result<rhai::Dynamic> {
        let certificate = TlsCertificate::load_pem_file(filepath).map_err(|e| e.to_string())?;
        rhai::serde::to_dynamic(&certificate).map_err(|e| e.to_string().into())
    }
}
