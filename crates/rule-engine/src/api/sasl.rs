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

use super::State;
use rhai::plugin::{
    Dynamic, FnAccess, FnNamespace, Module, NativeCallContext, PluginFunction, RhaiResult, TypeId,
};
use vsmtp_common::stateful_ctx_received::SaslAuthProps;
use vsmtp_common::stateful_ctx_received::StatefulCtxReceived;
use vsmtp_protocol::auth::Credentials;

pub use sasl_rhai::*;

#[rhai::plugin::export_module]
mod sasl_rhai {
    /// # rhai-autodocs:index:1
    #[rhai_fn(global, get = "is_authenticated")]
    pub fn is_authenticated(ctx: &mut State<StatefulCtxReceived>) -> bool {
        ctx.read(|ctx| {
            ctx.get_connect()
                .sasl
                .as_ref()
                .is_some_and(|sasl| sasl.is_authenticated)
        })
    }

    /// # rhai-autodocs:index:2
    #[rhai_fn(global, get = "sasl", return_raw)]
    pub fn get_sasl_props(
        ctx: &mut State<StatefulCtxReceived>,
    ) -> Result<SaslAuthProps, Box<rhai::EvalAltResult>> {
        ctx.read(|ctx| {
            ctx.get_connect().sasl.as_ref().map_or_else(
                || Err("SASL not initialized".into()),
                |sasl| Ok(sasl.clone()),
            )
        })
    }

    /// # rhai-autodocs:index:3
    #[rhai_fn(global, get = "mechanism")]
    pub fn get_sasl_mechanism(ctx: &mut SaslAuthProps) -> String {
        ctx.mechanism.to_string()
    }

    /// # rhai-autodocs:index:4
    #[rhai_fn(global, get = "authid")]
    pub fn get_authid(ctx: &mut SaslAuthProps) -> String {
        match &ctx.credentials {
            Credentials::Verify {
                authid,
                authpass: _,
            } => authid.clone(),
            Credentials::AnonymousToken { token: _ } => todo!(),
        }
    }

    /// # rhai-autodocs:index:5
    #[rhai_fn(global, get = "password")]
    pub fn get_authpass(ctx: &mut SaslAuthProps) -> String {
        match &ctx.credentials {
            Credentials::Verify {
                authid: _,
                authpass: password,
            } => password.clone(),
            Credentials::AnonymousToken { token: _ } => todo!(),
        }
    }
}
