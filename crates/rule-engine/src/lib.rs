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

#![doc = include_str!("../README.md")]

mod dsl {
    /// Rules and actions syntax implementation.
    pub mod directives;
}

/// Settings used to spawn rule engines.
mod config;
/// Custom module resolver to import rules split by stages and email flow for a given domain.
mod module_resolver;
/// "Hooks" used to identify when to run a batch of [`Directives`].
mod stage;
/// Values return by the rule engine when executing a script.
mod status;

pub use crate::config::builder::RuleEngineConfigBuilder;
pub use crate::config::RuleEngineConfig;
pub use crate::stage::Stage;
pub use crate::status::Status;
use api::State;
pub use dsl::directives::{
    directives_try_from, error::DirectiveError, error::ParseError, Directive, Directives, Flow,
    FlowType,
};
pub use rhai;
pub use rhai_dylib;

/// Module containing the backend for the Rhai API.
pub mod api;

use dsl::directives::error::DirectiveErrorKind;

/// Enable running a future inside Rhai code.
fn block_on<O>(f: impl std::future::Future<Output = O>) -> O {
    tokio::task::block_in_place(move || tokio::runtime::Handle::current().block_on(f))
}

/// A runtime to execute rules from scripts.
///
/// See [`RuleEngineConfig`] with contains pre-compiled scripts and [`RuleEngineConfigBuilder`]
/// to build the configuration, then [`RuleEngine::from_config_with_state`] to generate a new rule engine
/// from the configuration.
#[derive(Debug)]
pub struct RuleEngine<CONTEXT, STATUS, STAGE>
where
    // Persistent data between the scripts and rule engine caller
    CONTEXT: 'static,
    // Returned value by the rule engine when running scripts.
    // Help vSMTP to decide what to do next.
    STATUS: Status,
    // Identifies when to run a batch of [`Directives`].
    STAGE: Stage + 'static,
{
    /// Underlying Rhai engine.
    ///
    /// Use this field to customize the engine further. Although any change will be lost
    /// when this engine is dropped. Prefer using a [`RuleEngineConfig`] to configure
    /// a "template" for other engines.
    rhai_engine: rhai::Engine,
    /// Rule engine configuration template used to the configure
    /// the Rhai engine.
    config: rhai::Shared<RuleEngineConfig<CONTEXT, STATUS, STAGE>>,
    status: std::marker::PhantomData<STATUS>,
    stage: std::marker::PhantomData<STAGE>,
    // NOTE: Would a stateless `RuleEngine` be useful ?
    /// Global state that is passed to the rule engine. Is used to store and read
    /// context from the engine's caller.
    state: State<CONTEXT>,
}

impl<CONTEXT: 'static + std::fmt::Debug, STATUS: Status, STAGE: Stage>
    RuleEngine<CONTEXT, STATUS, STAGE>
{
    /// Builds a cheap rhai engine from the given configuration.
    ///
    /// Configuring, registering modules, types, functions and mostly compiling
    /// a Rhai script takes some time and resources.
    /// When iterating through a multi-threaded context, it is simply better
    /// to compile the scripts ahead of time and then spawn a cheap runtime.
    ///
    /// Thus, to create a rule engine, use the [`RuleEngineConfigBuilder`] to
    /// create a configuration and compile scripts. Then use this function
    /// to generate a runtime based on this config.
    ///
    /// # Arguments
    ///
    /// * `config` - A sharable config. Using `[rhai::Shared]` let's us,
    ///              at compile time, to wrap the config into an atomic pointer
    ///              in a multi-threaded environment,
    ///              or a ref cell in a single threaded environment.
    /// * `state` -  A global state to the engine, used to store and inspect data.
    ///             See [`State`] for more information.
    pub fn from_config_with_state(
        config: rhai::Shared<RuleEngineConfig<CONTEXT, STATUS, STAGE>>,
        state: CONTEXT,
    ) -> Self {
        let mut rhai_engine = rhai::Engine::new_raw();

        // Registering all modules.
        rhai_engine.register_global_module(config.config_module.clone());
        for i in &config.global_modules {
            rhai_engine.register_global_module(i.clone());
        }
        for (namespace, module) in &config.static_modules {
            rhai_engine.register_static_module(namespace, module.clone());
        }

        rhai_engine
            .on_print(|msg| tracing::info!("{}", msg))
            .on_debug(|msg, src, pos| tracing::debug!(?src, ?pos, msg));

        // Setting up directive parsing.
        // FIXME: this should not be necessary because the AST as already been parsed, right ?
        rhai_engine
            .register_custom_syntax_with_state_raw(
                "rule",
                Directive::parse_directive,
                true,
                crate::dsl::directives::rule::parse,
            )
            .register_custom_syntax_with_state_raw(
                "action",
                Directive::parse_directive,
                true,
                crate::dsl::directives::action::parse,
            );

        rhai_engine.set_fast_operators(false);

        Self {
            rhai_engine,
            config,
            status: std::marker::PhantomData::<STATUS>,
            stage: std::marker::PhantomData::<STAGE>,
            state: State::from(state),
        }
    }

    /// Run a stage hook function and returns a status.
    #[must_use]
    #[tracing::instrument(skip(self), ret, fields(hook = %stage.hook()))]
    pub fn run(&self, stage: &STAGE) -> STATUS {
        let hook = stage.hook();

        match self
            .rhai_engine
            .call_fn_with_options::<STATUS>(
                // The stage is fetched from the `global_runtime_state` constants from
                // the `run` functions to prevent having to pass it by parameter.
                rhai::CallFnOptions::new().with_tag(*stage),
                &mut rhai::Scope::default(),
                &self.config.ast,
                hook,
                (self.state.clone(),),
            )
            .or_else(|error| match *error {
                // If the user did not define the current stage function hook we simply go to the next stage.
                rhai::EvalAltResult::ErrorFunctionNotFound(func, _) if func == hook => {
                    Ok(STATUS::next())
                }
                _ => Err(error),
            }) {
            Ok(status) => status,
            Err(error) => STATUS::error(DirectiveError {
                kind: DirectiveErrorKind::Runtime(error),
                stage: Some(stage.to_string()),
                directive: None,
            }),
        }
    }

    /// Read the value of the state.
    pub fn read_state<O>(&self, f: impl FnOnce(&CONTEXT) -> O) -> O {
        self.state.read(f)
    }

    /// Write to the state.
    pub fn write_state<O>(&self, f: impl FnOnce(&mut CONTEXT) -> O) -> O {
        self.state.write(f)
    }

    /// Take the inner value of the state.
    ///
    /// # Panics
    ///
    /// This function is used to extract the state from the engine after a single/multiple
    /// run calls. DO NOT use the engine after a call to this function.
    #[must_use]
    pub fn take_state(self) -> CONTEXT {
        self.state.into_inner()
    }
}
