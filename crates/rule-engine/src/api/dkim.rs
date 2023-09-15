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

use crate::api::State;
use rhai::plugin::{
    mem, Dynamic, FnAccess, FnNamespace, Module, NativeCallContext, PluginFunction, RhaiResult,
    TypeId,
};
use vsmtp_auth::dkim as backend;
use vsmtp_common::stateful_ctx_received::StatefulCtxReceived;
use vsmtp_common::{dkim::DkimVerificationResult, dns_resolver::DnsResolver};
use vsmtp_mail_parser::{mail::headers::Header, Mail};

pub use dkim::*;

#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct SignParams {
    sdid: String,
    selector: String,
    #[serde(deserialize_with = "deserialize_private_key")]
    private_key: rhai::Shared<backend::PrivateKey>,
    #[serde(default)]
    headers_field: Option<Vec<String>>,
    #[serde(default, deserialize_with = "deserialize_canonicalization")]
    canonicalization: Option<backend::Canonicalization>,
}

#[allow(non_camel_case_types)]
struct const_usize<const U: usize>;

impl<const U: usize> const_usize<U> {
    const fn value() -> usize {
        U
    }
}

#[allow(non_camel_case_types)]
struct const_u64<const U: u64>;

impl<const U: u64> const_u64<U> {
    const fn value() -> u64 {
        U
    }
}

struct DkimMail<'a> {
    mail: &'a vsmtp_mail_parser::Mail,
}

struct DkimHeader<'a> {
    header: &'a vsmtp_mail_parser::mail::headers::Header,
}

impl backend::Header for DkimHeader<'_> {
    fn field_name(&self) -> String {
        self.header.name.clone()
    }

    fn get(&self) -> String {
        self.header.to_string()
    }
}

impl<'a> backend::Mail for DkimMail<'a> {
    type H = DkimHeader<'a>;

    fn get_body(&self) -> String {
        self.mail.body.to_string()
    }

    fn get_headers(&self) -> Vec<Self::H> {
        self.mail
            .headers
            .0
            .iter()
            .map(|i| DkimHeader { header: i })
            .collect::<Vec<_>>()
    }
}

#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct VerifyParams {
    #[serde(default = "const_usize::<5>::value")]
    header_limit_count: usize,
    #[serde(default = "const_u64::<100>::value")]
    expiration_epsilon: u64,
    #[serde(deserialize_with = "super::deserialize_dns_resolver")]
    dns_resolver: rhai::Shared<DnsResolver>,
}

fn deserialize_private_key<'de, D>(
    deserializer: D,
) -> Result<rhai::Shared<backend::PrivateKey>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let private_key = <rhai::Dynamic as serde::Deserialize>::deserialize(deserializer)?;

    private_key
        .try_cast::<rhai::Shared<backend::PrivateKey>>()
        .ok_or_else(|| serde::de::Error::custom("failed to parse private key"))
}

fn deserialize_canonicalization<'de, D>(
    deserializer: D,
) -> Result<Option<backend::Canonicalization>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let canonicalization = <rhai::Dynamic as serde::Deserialize>::deserialize(deserializer)?;

    if canonicalization.is_unit() {
        return Ok(None);
    }

    canonicalization
        .into_string()
        .map_err(|t| {
            serde::de::Error::custom(format!(
                "dkim canonicalization parameter is not a string (got {t})"
            ))
        })?
        .parse()
        .map(Some)
        .map_err(|_| serde::de::Error::custom("failed to parse canonicalization"))
}

/// Return the one public key found in the DNS record associated with the signature.
/// <https://datatracker.ietf.org/doc/html/rfc6376#section-3.6.2.2>
async fn get_public_key(
    dns_resolver: &DnsResolver,
    signature: &backend::Signature,
) -> Result<backend::PublicKey, vsmtp_common::dkim::Value> {
    match dns_resolver
        .resolver
        .txt_lookup(signature.get_dns_query())
        .await
    {
        Ok(txt_values) if txt_values.iter().count() != 1 => {
            tracing::debug!("Multiple TXT records found");
            Err(vsmtp_common::dkim::Value::Policy)
        }
        Ok(txt_values) => {
            let value = txt_values.into_iter().next().expect("count == 1");
            <backend::PublicKey as std::str::FromStr>::from_str(&value.to_string())
                .map_err(|_| vsmtp_common::dkim::Value::PermFail)
        }
        Err(e)
            if matches!(
                e.kind(),
                vsmtp_common::trust_dns_resolver::error::ResolveErrorKind::NoRecordsFound { .. }
            ) =>
        {
            Err(vsmtp_common::dkim::Value::PermFail)
        }
        Err(e) => {
            tracing::debug!("Failed to look up TXT record: {:?}", e);
            Err(vsmtp_common::dkim::Value::TempFail)
        }
    }
}

/// Implementation of:
/// * [`RFC "DomainKeys Identified Mail (DKIM) Signatures"`](https://datatracker.ietf.org/doc/html/rfc8601)
/// * [`RFC "Cryptographic Algorithm and Key Usage Update to DomainKeys Identified Mail (DKIM)"`](https://datatracker.ietf.org/doc/html/rfc8301)
/// * [`RFC "A New Cryptographic Signature Method for DomainKeys Identified Mail (DKIM)'`](https://datatracker.ietf.org/doc/html/rfc8463)
#[rhai::plugin::export_module]
mod dkim {
    use crate::api::Result;

    /// Produce a DKIM signature with the given parameters.
    ///
    /// This method will produce a new signature for the message, with the given parameters **and
    /// will not add** a `DKIM-Signature` header to the message.
    ///
    /// This method is useful if you want to inspect and add the signature to the message yourself.
    /// If you want to create and add the signature immediately, use [add_signature](http://vsmtp.rs/docs/global/dkim#fn-add_signature).
    ///
    /// # Arguments
    ///
    /// * `params` - A map containing the parameters for the signature.
    ///   * `sdid` - The domain name of the signing domain (used to retrieves the public key).
    ///   * `selector` - The selector for the signing domain (used to retrieves the public key).
    ///   * `private_key` - The private key to use for the signature, loaded with the [crypto] module.
    ///   * `headers_field` - The list of headers to sign, optional `["From", "To", "Date", "Subject", "From"]` by default.
    ///   * `canonicalization` - The canonicalization algorithm to use, optional `"simple/relaxed"` by default.
    ///
    /// [crypto]: http://vsmtp.rs/docs/global/crypto
    ///
    /// # Example
    ///
    ///```js
    /// rule "add dkim signature" |ctx| {
    ///   let signature = dkim::create_sign(ctx.mail, #{
    ///     sdid: "mydomain.tld",
    ///     selector: "myselector",
    ///     private_key: crypto::load_pem_rsa_pkcs8_file("/etc/vsmtp/keys/my_key.pem"),
    ///     headers_field: ["From", "To", "Date", "Subject", "From"],
    ///     canonicalization: "simple/relaxed",
    ///   });
    ///   log("info", `My DKIM signature: ${signature}`);
    ///   ctx.prepend_header("DKIM-Signature", signature);
    ///   status::next();
    /// }
    /// ```
    ///
    /// # rhai-autodocs:index:1
    #[rhai_fn(return_raw)]
    pub fn create_sign(
        mail: &mut rhai::Shared<std::sync::RwLock<Mail>>,
        params: rhai::Dynamic,
    ) -> Result<String> {
        let SignParams {
            sdid,
            selector,
            private_key,
            headers_field,
            canonicalization,
        } = rhai::serde::from_dynamic::<SignParams>(&params)?;

        let signature = {
            let mail = mail.read().unwrap();
            backend::sign(
                &DkimMail { mail: &mail },
                &private_key,
                sdid,
                selector,
                canonicalization
                    .unwrap_or_else(|| "simple/relaxed".parse().expect("default values are valid")),
                headers_field.unwrap_or_else(|| {
                    ["From", "To", "Date", "Subject", "From"]
                        .into_iter()
                        .map(str::to_string)
                        .collect()
                }),
            )
        };

        match signature {
            Ok(signature) => {
                let mut value = signature.get_signature_value();
                // FIXME: enhance whitespace handling
                let removed_char = value.remove(0);
                debug_assert_eq!(removed_char, ' ');
                tracing::trace!("Signature: {:?}: with value '{:?}'", signature, value);
                Ok(value)
            }
            Err(e) => {
                tracing::error!("An error ocurred while signing mail: {:?}", e);
                Err(format!("{e:?}").into())
            }
        }
    }

    /// This method will produce a new signature for the message, with the given parameters and
    /// add a `DKIM-Signature` header to the message.
    ///
    /// # Arguments
    ///
    /// * `params` - A map containing the parameters for the signature.
    ///   * `sdid` - The domain name of the signing domain (used to retrieves the public key).
    ///   * `selector` - The selector for the signing domain (used to retrieves the public key).
    ///   * `private_key` - The private key to use for the signature, loaded with the [crypto] module.
    ///   * `headers_field` - The list of headers to sign, optional `["From", "To", "Date", "Subject", "From"]` by default.
    ///   * `canonicalization` - The canonicalization algorithm to use, optional `"simple/relaxed"` by default.
    ///
    /// [crypto]: http://vsmtp.rs/docs/global/crypto
    ///
    /// # Example
    ///
    ///```js
    /// rule "add dkim signature" |ctx| {
    ///   dkim::add_signature(ctx.mail, #{
    ///     sdid: "mydomain.tld",
    ///     selector: "myselector",
    ///     private_key: crypto::load_pem_rsa_pkcs8_file("/etc/vsmtp/keys/my_key.pem"),
    ///     headers_field: ["From", "To", "Date", "Subject", "From"],
    ///     canonicalization: "simple/relaxed",
    ///   });
    ///   status::next();
    /// }
    /// ```
    ///
    /// # rhai-autodocs:index:2
    #[rhai_fn(return_raw)]
    pub fn add_signature(
        mail: &mut rhai::Shared<std::sync::RwLock<Mail>>,
        params: rhai::Dynamic,
    ) -> Result<()> {
        let signature = create_sign(mail, params)?;
        mail.write()
            .unwrap()
            .prepend_headers([Header::new("DKIM-Signature", signature)]);
        Ok(())
    }

    /// # rhai-autodocs:index:3
    #[rhai_fn(global, pure)]
    pub fn to_debug(v: &mut rhai::Shared<Vec<DkimVerificationResult>>) -> String {
        format!("{v:?}")
    }

    /// Verify all the DKIM signature of the message. This method will return a list of
    /// DKIM verification result, one for each signature (or an array of one element with
    /// the value `none` if there is no `DKIM-Signature` header).
    ///
    /// You can then store the result in the `ctx` with [store](http://vsmtp.rs/docs/global/dkim#fn-store),
    /// it will then be used by the [add_header](http://vsmtp.rs/docs/global/auth#fn-add_header) method.
    ///
    /// # Arguments
    ///
    /// * `params` - A map containing the parameters for the verification.
    ///   * `header_limit_count` - The maximum number of `DKIM-Signature` header to verify, optional `5` by default.
    ///   * `expiration_epsilon` - The number of seconds of tolerance for the signature expiration, optional `100` by default.
    ///   * `dns_resolver` - The DNS resolver to use for the verification, loaded with the [dns] module.
    ///
    /// [dns]: http://vsmtp.rs/docs/global/dns
    ///
    /// # Example
    ///
    /// ```js
    /// rule "verify dkim signature" |ctx| {
    ///   const dkim_results = dkim::verify(#{
    ///     header_limit_count: 5,
    ///     expiration_epsilon: 100,
    ///     dns_resolver: dns::resolver(),
    ///   });
    ///   log("info", `DKIM results: ${dkim_results}`);
    ///   dkim::store(dkim_results);
    ///   status::next();
    /// }
    /// ```
    ///
    /// # rhai-autodocs:index:4
    #[allow(clippy::significant_drop_tightening)]
    #[rhai_fn(return_raw)]
    pub fn verify(
        ctx: &mut rhai::Shared<std::sync::RwLock<Mail>>,
        params: rhai::Dynamic,
    ) -> Result<rhai::Shared<Vec<DkimVerificationResult>>> {
        let VerifyParams {
            header_limit_count,
            expiration_epsilon,
            dns_resolver,
        } = rhai::serde::from_dynamic::<VerifyParams>(&params)?;

        let mail = ctx.read().unwrap();

        let verifications = mail
            .get_headers_raw_without_crlf("DKIM-Signature")
            .take(header_limit_count)
            .map(|header| verify_one(header, expiration_epsilon, &mail, &dns_resolver))
            .collect::<Vec<_>>();

        if verifications.is_empty() {
            Ok(vec![DkimVerificationResult {
                value: vsmtp_common::dkim::Value::None,
                signature: None,
            }]
            .into())
        } else {
            Ok(crate::block_on(futures_util::future::join_all(verifications)).into())
        }
    }

    /// See the documentation of [verify](http://vsmtp.rs/docs/global/dkim#fn-verify).
    /// # rhai-autodocs:index:5
    #[rhai_fn(global, pure, return_raw)]
    pub fn store(
        ctx: &mut State<StatefulCtxReceived>,
        dkim_result: rhai::Shared<Vec<DkimVerificationResult>>,
    ) -> Result<()> {
        ctx.write(|ctx| {
            ctx.mut_complete()?.dkim = Some(dkim_result);
            Ok(())
        })
    }
}

async fn verify_one(
    header: String,
    expiration_epsilon: u64,
    mail: &Mail,
    dns_resolver: &DnsResolver,
) -> DkimVerificationResult {
    tracing::trace!(?header, "Verifying DKIM signature ...");

    let Ok(signature) = <backend::Signature as std::str::FromStr>::from_str(&header) else {
        tracing::debug!("error parsing the DKIM signature header");
        return DkimVerificationResult {
            value: vsmtp_common::dkim::Value::PermFail,
            signature: None,
        };
    };

    if signature.has_expired(expiration_epsilon) {
        tracing::warn!("The DKIM signature has expired");
        return DkimVerificationResult {
            value: vsmtp_common::dkim::Value::PermFail,
            signature: Some(signature),
        };
    }

    let public_key = match get_public_key(dns_resolver, &signature).await {
        Ok(public_key) => public_key,
        Err(value) => {
            tracing::debug!(
                "Failed to retrieve the public key signature: {}",
                value.to_string()
            );
            return DkimVerificationResult {
                value,
                signature: Some(signature),
            };
        }
    };

    if let Err(e) = backend::verify(&signature, &DkimMail { mail }, &public_key) {
        tracing::debug!("Failed to verify the DKIM signature: {:?}", e);
        return DkimVerificationResult {
            value: vsmtp_common::dkim::Value::PermFail,
            signature: Some(signature),
        };
    }

    tracing::debug!("DKIM signature successfully verified.");

    if public_key.has_debug_flag() {
        tracing::warn!("DKIM signature contains `debug_flag`");
        return DkimVerificationResult {
            value: vsmtp_common::dkim::Value::Policy,
            signature: Some(signature),
        };
    }

    DkimVerificationResult {
        value: vsmtp_common::dkim::Value::Pass,
        signature: Some(signature),
    }
}
