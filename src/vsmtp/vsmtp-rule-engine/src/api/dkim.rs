/*
 * vSMTP mail transfer agent
 * Copyright (C) 2022 viridIT SAS
 *
 * This program is free software: you can redistribute it and/or modify it under
 * the terms of the GNU General Public License as published by the Free Software
 * Foundation, either version 3 of the License, or any later version.
 *
 * This program is distributed in the hope that it will be useful, but WITHOUT
 * ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
 * FOR A PARTICULAR PURPOSE.  See the GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License along with
 * this program. If not, see https://www.gnu.org/licenses/.
 *
*/

use crate::api::{Context, EngineResult, Message, Server};
use rhai::plugin::{
    mem, Dynamic, FnAccess, FnNamespace, ImmutableString, Module, NativeCallContext,
    PluginFunction, RhaiResult, TypeId,
};
use vsmtp_auth::dkim::{
    Canonicalization, CanonicalizationAlgorithm, PublicKey, Signature, VerifierError,
};
use vsmtp_common::re::{log, tokio};

#[derive(Debug)]
struct DnsError(trust_dns_resolver::error::ResolveError);

impl Default for DnsError {
    fn default() -> Self {
        Self(trust_dns_resolver::error::ResolveError::from(
            trust_dns_resolver::error::ResolveErrorKind::Message("`default` invoked"),
        ))
    }
}

impl std::fmt::Display for DnsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, strum::EnumString, strum::EnumMessage, thiserror::Error)]
enum DkimErrors {
    #[strum(message = "neutral", detailed_message = "signature_parsing_failed")]
    #[error("the parsing of the signature failed: `{inner}`")]
    SignatureParsingFailed {
        inner: <Signature as std::str::FromStr>::Err,
    },
    #[strum(message = "neutral", detailed_message = "key_parsing_failed")]
    #[error("the parsing of the public key failed: `{inner}`")]
    KeyParsingFailed {
        inner: <PublicKey as std::str::FromStr>::Err,
    },
    #[strum(message = "neutral", detailed_message = "invalid_argument")]
    #[error("invalid argument: `{inner}`")]
    InvalidArgument { inner: String },
    #[strum(message = "temperror", detailed_message = "temp_dns_error")]
    #[error("temporary dns error: `{inner}`")]
    TempDnsError { inner: DnsError },
    #[strum(message = "permerror", detailed_message = "perm_dns_error")]
    #[error("permanent dns error: `{inner}`")]
    PermDnsError { inner: DnsError },
    #[strum(message = "fail", detailed_message = "signature_mismatch")]
    #[error("the signature does not match: `{inner}`")]
    SignatureMismatch { inner: VerifierError },
}

impl From<DkimErrors> for Box<rhai::EvalAltResult> {
    fn from(this: DkimErrors) -> Self {
        Box::new(rhai::EvalAltResult::ErrorSystem(
            strum::EnumMessage::get_detailed_message(&this)
                .expect("`DkimErrors` must have a `detailed message` for each variant")
                .to_string(),
            Box::new(this),
        ))
    }
}

#[rhai::plugin::export_module]
mod dkim_rhai {

    /// get the dkim status from an error produced by this module
    #[rhai_fn(global, return_raw)]
    pub fn handle_dkim_error(err: rhai::Dynamic) -> EngineResult<String> {
        println!("{err:?}");

        todo!()

        // let err = err.cast::<Box<rhai::EvalAltResult>>();
        //
        // let r#type = DkimErrors::try_from(err).map_err::<Box<rhai::EvalAltResult>, _>(|e| {
        //     format!("not the right type: `{e}`").into()
        // })?;
        //
        // Ok(strum::EnumMessage::get_message(&r#type)
        //     .expect("`DkimErrors` must have a `message` for each variant")
        //     .to_string())
    }

    /// return the `sdid` property of the [`Signature`]
    #[rhai_fn(global, get = "sdid", pure)]
    pub fn sdid(signature: &mut Signature) -> String {
        signature.sdid.clone()
    }

    /// return the `auid` property of the [`Signature`]
    #[rhai_fn(global, get = "auid", pure)]
    pub fn auid(signature: &mut Signature) -> String {
        signature.auid.clone()
    }

    /// create a [`Signature`] from a `DKIM-Signature` header
    #[rhai_fn(global, return_raw)]
    pub fn parse_signature(input: &str) -> EngineResult<Signature> {
        <Signature as std::str::FromStr>::from_str(input).map_err::<Box<rhai::EvalAltResult>, _>(
            |inner| {
                log::warn!("failed to parse DKIM-Signature: `{inner}`");
                DkimErrors::SignatureParsingFailed { inner }.into()
            },
        )
    }

    /// Has the signature expired?
    ///
    /// return `true` if the argument are invalid
    #[rhai_fn(global, pure)]
    pub fn has_expired(signature: &mut Signature, epsilon: rhai::INT) -> bool {
        epsilon
            .try_into()
            .map_or(true, |epsilon| signature.has_expired(epsilon))
    }

    /// Get the list of public keys associated with this [`Signature`]
    ///
    /// The current implementation will make a TXT query on the dns of the signer
    ///
    /// `on_multiple_key_records` value can be `first` or `cycle` :
    /// * `first` return the first key found (one element array)
    /// * `cycle` return all the keys found
    #[rhai_fn(global, pure, return_raw)]
    pub fn get_public_key(
        server: &mut Server,
        signature: Signature,
        on_multiple_key_records: &str,
    ) -> EngineResult<rhai::Dynamic> {
        const VALID_POLICY: [&str; 2] = ["first", "cycle"];
        if !VALID_POLICY.contains(&on_multiple_key_records) {
            return Err(DkimErrors::InvalidArgument {
                inner: format!(
                    "expected values in `[first, cycle]` but got `{on_multiple_key_records}`",
                ),
            }
            .into());
        }

        let resolver = server.resolvers.get(&server.config.server.domain).unwrap();

        let txt_record = tokio::task::block_in_place(move || {
            tokio::runtime::Handle::current()
                .block_on(resolver.txt_lookup(signature.get_dns_query()))
        })
        .map_err::<Box<rhai::EvalAltResult>, _>(|e| {
            use trust_dns_resolver::error::ResolveErrorKind;
            log::warn!("failed to get public key: `{e}`");
            if matches!(
                e.kind(),
                ResolveErrorKind::Message(_)
                    | ResolveErrorKind::Msg(_)
                    | ResolveErrorKind::NoConnections
                    | ResolveErrorKind::NoRecordsFound { .. }
            ) {
                DkimErrors::PermDnsError { inner: DnsError(e) }
            } else {
                DkimErrors::TempDnsError { inner: DnsError(e) }
            }
            .into()
        })?;

        let keys = txt_record
            .into_iter()
            .map(|i| <PublicKey as std::str::FromStr>::from_str(&i.to_string()));

        let keys = keys
            .collect::<Result<Vec<_>, <PublicKey as std::str::FromStr>::Err>>()
            .map_err::<Box<rhai::EvalAltResult>, _>(|inner| {
                DkimErrors::KeyParsingFailed { inner }.into()
            })?;

        Ok(if on_multiple_key_records == "first" {
            match keys.into_iter().next() {
                Some(i) => vec![i],
                None => vec![],
            }
        } else {
            keys
        }
        .into())
    }

    /// A public key may contains a `debug flag`, used for testing purpose.
    #[rhai_fn(global, pure, get = "has_debug_flag")]
    pub fn has_debug_flag(key: &mut PublicKey) -> bool {
        key.has_debug_flag()
    }

    /// Operate the hashing of the `message`'s headers and body, and compare the result with the
    /// `signature` and `key` data.
    #[allow(clippy::needless_pass_by_value)]
    #[rhai_fn(global, pure, return_raw)]
    pub fn verify_dkim(
        message: &mut Message,
        signature: Signature,
        key: PublicKey,
    ) -> EngineResult<()> {
        let guard = vsl_guard_ok!(message.read());

        log::debug!("key={key:?}");
        log::debug!("signature={signature:?}");

        signature
            .verify(guard.inner(), &key)
            .map_err::<Box<rhai::EvalAltResult>, _>(|inner| {
                DkimErrors::SignatureMismatch { inner }.into()
            })
    }

    ///
    #[rhai_fn(global, pure, return_raw)]
    #[allow(clippy::needless_pass_by_value)]
    pub fn sign_dkim(
        message: &mut Message,
        context: Context,
        server: Server,
        selector: &str,
        headers_field: rhai::Array,
        canonicalization: &str,
    ) -> EngineResult<()> {
        let (header, body) = canonicalization
            .split_once('/')
            .ok_or_else::<Box<rhai::EvalAltResult>, _>(|| {
                "invalid canonicalization: expected `header/body`".into()
            })?;
        let (header, body) = (
            <CanonicalizationAlgorithm as std::str::FromStr>::from_str(header).map_err::<Box<
                rhai::EvalAltResult,
            >, _>(
                |e| format!("got error for canonicalization of headers: `{e}`").into(),
            )?,
            <CanonicalizationAlgorithm as std::str::FromStr>::from_str(body).map_err::<Box<
                rhai::EvalAltResult,
            >, _>(
                |e| format!("got error for canonicalization of body: `{e}`").into(),
            )?,
        );

        let mut msg_guard = vsl_guard_ok!(message.write());
        let ctx_guard = vsl_guard_ok!(context.read());

        let sdid = &ctx_guard.connection.server_name;
        let dkim_params = server
            .config
            .server
            .r#virtual
            .get(sdid)
            .map_or_else(|| &server.config.server.dkim, |i| &i.dkim);

        match dkim_params {
            None => Err(format!("dkim params are empty for this `{sdid}`").into()),
            Some(dkim_params) => {
                let signature = Signature::sign(
                    msg_guard.inner(),
                    selector,
                    sdid,
                    headers_field.iter().map(ToString::to_string).collect(),
                    &dkim_params.private_key.inner,
                    Canonicalization { header, body },
                )
                .map_err::<Box<rhai::EvalAltResult>, _>(|e| e.to_string().into())?;

                msg_guard.add_header("DKIM-Signature", &signature.get_signature_value());

                Ok(())
            }
        }
    }
}

pub use dkim_rhai::*;