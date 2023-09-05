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
use vsmtp_mail_parser::Mail;

pub use dkim::*;

#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct SignParams {
    sdid: String,
    selector: String,
    #[serde(deserialize_with = "deserialize_private_key")]
    private_key: std::sync::Arc<backend::PrivateKey>,
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

#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct VerifyParams {
    #[serde(default = "const_usize::<5>::value")]
    header_limit_count: usize,
    #[serde(default = "const_u64::<100>::value")]
    expiration_epsilon: u64,
    #[serde(deserialize_with = "super::deserialize_dns_resolver")]
    dns_resolver: std::sync::Arc<DnsResolver>,
}

fn deserialize_private_key<'de, D>(
    deserializer: D,
) -> Result<std::sync::Arc<backend::PrivateKey>, D::Error>
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

/// Generate and verify DKIM signatures.
/// Implementation of RFC 6376. (<https://www.rfc-editor.org/rfc/rfc6376.html>)
#[rhai::plugin::export_module]
mod dkim {
    use vsmtp_mail_parser::mail::headers::Header;

    /// Produce a DKIM signature with the given parameters.
    /// # rhai-autodocs:index:1
    #[rhai_fn(return_raw)]
    pub fn create_sign(
        mail: &mut std::sync::Arc<std::sync::RwLock<Mail>>,
        params: rhai::Dynamic,
    ) -> Result<String, Box<rhai::EvalAltResult>> {
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
                &mail,
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

    /// # rhai-autodocs:index:2
    #[rhai_fn(return_raw)]
    pub fn add_signature(
        mail: &mut std::sync::Arc<std::sync::RwLock<Mail>>,
        params: rhai::Dynamic,
    ) -> Result<(), Box<rhai::EvalAltResult>> {
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

    /// # rhai-autodocs:index:4
    #[allow(clippy::significant_drop_tightening)]
    #[rhai_fn(return_raw)]
    pub fn verify(
        ctx: &mut std::sync::Arc<std::sync::RwLock<Mail>>,
        params: rhai::Dynamic,
    ) -> Result<rhai::Shared<Vec<DkimVerificationResult>>, Box<rhai::EvalAltResult>> {
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

    /// # rhai-autodocs:index:5
    #[rhai_fn(global, pure, return_raw)]
    pub fn store(
        ctx: &mut State<StatefulCtxReceived>,
        dkim_result: rhai::Shared<Vec<DkimVerificationResult>>,
    ) -> Result<(), Box<rhai::EvalAltResult>> {
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

    if let Err(e) = backend::verify(&signature, mail, &public_key) {
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
