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

use crate::{
    api::State,
    config::RuleEngineConfig,
    module_resolver::{DomainFilterResolver, Domains},
    Directive, DirectiveError, Directives, Flow, FlowType, Stage, Status,
};
use rhai::{
    module_resolvers::{FileModuleResolver, ModuleResolversCollection},
    packages::Package,
};
use rhai_dylib::module_resolvers::libloading::DylibModuleResolver;
use vsmtp_common::stateful_ctx_received::StatefulCtxReceived;
use vsmtp_protocol::Domain;

#[allow(clippy::module_name_repetitions)]
/// Builder to create rule engines configuration.
#[derive(Debug)]
pub struct RuleEngineConfigBuilder<
    // TODO: `with_state` / `with_stage` / `with_status` https://docs.rs/rhai/latest/rhai/trait.RegisterNativeFunction.html
    //        instead of forcing the user to specify every type parameter when instantiating the builder.
    // Persistant data between the scripts and rule engine caller
    CONTEXT,
    // Returned value by the rule engine when running scripts.
    // Help vSMTP to decide what to do next.
    STATUS,
    // Identifies when to run a batch of [`Directives`].
    STAGE,
> where
    STATUS: Status,
    STAGE: Stage,
{
    rhai_engine: rhai::Engine,
    config_module: rhai::Shared<rhai::Module>,
    global_modules: Vec<rhai::Shared<rhai::Module>>,
    static_modules: Vec<(String, rhai::Shared<rhai::Module>)>,
    ast: rhai::AST,
    status: std::marker::PhantomData<STATUS>,
    stage: std::marker::PhantomData<STAGE>,
    state: std::marker::PhantomData<CONTEXT>,
}

/// Errors emitted by the rule engine configuration builder.
#[derive(Debug, thiserror::Error)]
pub enum RuleEngineConfigBuilderError {
    /// Failed to serialize a configuration object.
    #[error("failed to serialize the configuration: {0}")]
    Serialize(serde_json::Error),
    /// Failed to transform a configuration object into a Rhai JSON value.
    #[error("failed to serialize the configuration: {0}")]
    ParseJSON(Box<rhai::EvalAltResult>),
    /// Failed to build directives from a script.
    #[error("failed to build rules: {0}")]
    BuildDirectives(DirectiveError),
    /// Failed to compile the given rhai script into an AST.
    #[error("failed to compile a rhai script: {0}")]
    CompileScript(Box<rhai::EvalAltResult>),
    /// Failed to find the script at the given path.
    #[error("failed to load a rhai script at {0:?}")]
    LoadScript(String),
    /// Failed to customize the engine.
    #[error("failed to customize the engine: {0}")]
    Engine(Box<rhai::EvalAltResult>),
}

/// Alias for the result of a rule engine configuration builder.
pub type Result<T> = std::result::Result<T, RuleEngineConfigBuilderError>;
pub type RhaiResult<T> = std::result::Result<T, Box<rhai::EvalAltResult>>;

impl<CONTEXT, STATUS: Status, STAGE: Stage> Default
    for RuleEngineConfigBuilder<CONTEXT, STATUS, STAGE>
{
    /// Create a new builder.
    /// You must specify the [`Status`] type that will be returned by directives and [`Stage`] that will
    /// be used to select directives batches to run.
    ///
    /// The `CONTEXT` type used when calling rhai apis is generic, but prefer to use the [`State<StatefulCtxReceived>`] context,
    /// because it is the argument used in most functions exposed by the rule engine. Some services will make their
    /// own apis without importing those of the rule engine by default, that is why the context is generic.
    fn default() -> Self {
        if rhai::config::hashing::get_ahash_seed().is_none() {
            rhai::config::hashing::set_ahash_seed(Some([1, 2, 3, 4]))
            .expect("Rhai ahash seed has been set before the rule engine as been built. This is a bug, please report it at https://github.com/viridIT/vSMTP/issues.");
        }

        Self {
            rhai_engine: new_rhai_engine(),
            config_module: rhai::Shared::new(rhai::Module::default()),
            global_modules: Vec::default(),
            static_modules: Vec::default(),
            ast: rhai::AST::default(),
            status: std::marker::PhantomData,
            stage: std::marker::PhantomData,
            state: std::marker::PhantomData,
        }
    }
}

// Internal directive functions used in Rhai.
impl<CONTEXT: 'static, STATUS: Status, STAGE: Stage + 'static>
    RuleEngineConfigBuilder<CONTEXT, STATUS, STAGE>
{
    fn get_stage(ncc: &rhai::NativeCallContext<'_>) -> Option<STAGE> {
        ncc.tag()
            .and_then(|stage| stage.clone().try_cast::<STAGE>())
    }

    /// Get the "flow" of the email.
    ///
    /// # Return
    ///
    /// The flow of the mail, a `Flow` object, see `domain` and `type` functions.
    ///
    /// Can also return a unit `()` value if the server was not able to determine the flow of the email
    /// (When used before the `rcpt_to` stage) or if the client is attempting to relay emails.
    ///
    /// # Effective Stage
    ///
    /// From the `rcpt_to` stage of the receiver service.
    /// Accessible to all stages of post-reception services.
    #[allow(clippy::needless_pass_by_value)]
    pub fn flow(
        ctx: &mut State<StatefulCtxReceived>,
        domains: rhai::Shared<Domains<STAGE>>,
    ) -> RhaiResult<rhai::Dynamic> {
        Ok(ctx.read(|ctx| {
            let (Ok(sender), Ok(recipients)) = (ctx.get_mail_from(), ctx.get_rcpt_to()) else {
                return rhai::Dynamic::UNIT;
            };

            if sender
                .reverse_path
                .as_ref()
                .map_or(false, |sender| domains.contains_key(&sender.domain()))
            {
                // The sender is known by our configuration. (rules have been setup for this domain)
                let reverse_path = sender
                    .reverse_path
                    .as_ref()
                    .expect("reverse path has been checked above")
                    .0
                    .domain();

                // The current recipient as the same domain as the sender, this is a local transaction.
                if recipients
                    .recipient_values()
                    .last()
                    .map_or(false, |r| r.forward_path.domain() == reverse_path)
                {
                    Flow::new_dynamic(reverse_path, FlowType::Local)
                } else {
                    // Otherwise, the messages is being sent to an unknown domain, this is an outbound message.
                    Flow::new_dynamic(reverse_path, FlowType::Outbound)
                }
            } else if let Some(recipient) =
                // The sender is not known by our configuration, but the recipient is.
                recipients.recipient_values().last().and_then(|recipient| {
                        if domains.contains_key(&recipient.forward_path.domain()) {
                            Some(recipient)
                        } else {
                            None
                        }
                    })
            {
                Flow::new_dynamic(recipient.forward_path.domain(), FlowType::Inbound)
            } else {
                // Neither the sender nor the recipient domain are known by our configuration,
                // this is a relay attempt.
                rhai::Dynamic::UNIT
            }
        }))
    }

    /// Get the domain targeted by the flow.
    ///
    /// # Return
    ///
    ///  The domain targeted by the flow.
    ///  e.g. if the email is "inbound", then the recipients domain is used.
    ///            if the email is "outbound", then the sender domain is used.
    ///            if the email is "local", then the sender/recipient (which are the same) domain is used.
    ///
    /// # Effective stage
    ///
    /// From the `rcpt_to` stage of the receiver service.
    pub fn flow_get_domain(flow: &mut Flow) -> RhaiResult<String> {
        Ok(flow.domain.to_string())
    }

    /// Get the type of email flow.
    ///
    /// # Return
    ///
    /// The type of the flow, either "inbound", "outbound" or "local".
    ///
    /// # Effective stage
    ///
    /// From the `rcpt_to` stage of the receiver service.
    pub fn flow_get_type(flow: &mut Flow) -> RhaiResult<String> {
        Ok(<&'static str as From<&FlowType>>::from(&flow.r#type).to_string())
    }

    /// Equal comparison for the Flow object.
    pub fn flow_eq(flow: &mut Flow, other: rhai::Dynamic) -> RhaiResult<bool> {
        Ok(other
            .try_cast::<Flow>()
            .map_or(false, |other| *flow == other))
    }

    /// Not equal comparison for the Flow object.
    pub fn flow_neq(flow: &mut Flow, other: rhai::Dynamic) -> RhaiResult<bool> {
        Ok(other
            .try_cast::<Flow>()
            .map_or(true, |other| *flow != other))
    }

    /// Run a batch of directives from a rhai Array.
    /// Mostly used to execute inline rules that are not split by domain.
    #[allow(clippy::unnecessary_wraps)]
    fn run_directives_array(
        ncc: rhai::NativeCallContext<'_>,
        ctx: &mut State<CONTEXT>,
        directives: rhai::Array,
    ) -> RhaiResult<rhai::Dynamic> {
        Ok(
            match directives
                .into_iter()
                .map(Directive::try_from)
                .collect::<crate::dsl::directives::Result<Vec<Directive>>>()
            {
                Ok(directives) => rhai::Dynamic::from(Self::run_directives(ncc, ctx, &directives)),
                Err(error) => rhai::Dynamic::from(STATUS::error(error)),
            },
        )
    }

    /// Run a batch of directives from a vector of directives.
    /// Mostly used to execute inline rules that are not split by domain.
    #[allow(clippy::unnecessary_wraps, clippy::needless_pass_by_value)]
    fn run_directives_vec(
        ncc: rhai::NativeCallContext<'_>,
        ctx: &mut State<CONTEXT>,
        directives: Vec<Directive>,
    ) -> RhaiResult<rhai::Dynamic> {
        Ok(rhai::Dynamic::from(Self::run_directives(
            ncc,
            ctx,
            &directives,
        )))
    }

    /// Run directives for a given stage using a domain to determine which one to use.
    /// Used when directives are split by domain.
    #[allow(clippy::unnecessary_wraps, clippy::needless_pass_by_value)]
    fn run_directives_domain(
        ncc: rhai::NativeCallContext<'_>,
        ctx: &mut State<CONTEXT>,
        domains: rhai::Shared<Domains<STAGE>>,
        domain: &str,
    ) -> RhaiResult<rhai::Dynamic> {
        let Some(stage) = Self::get_stage(&ncc) else {
            return Ok(rhai::Dynamic::UNIT);
        };
        let Some(directives) = <Domain as std::str::FromStr>::from_str(domain)
            .ok()
            .and_then(|domain| domains.get(&domain))
            .and_then(|stages| stages.0.get(&stage))
        else {
            return Ok(rhai::Dynamic::UNIT);
        };

        let directives = match directives {
            Directives::Any(directives) => directives,
            Directives::Flow { .. } => return Ok(rhai::Dynamic::UNIT),
        };

        Ok(rhai::Dynamic::from(Self::run_directives(
            ncc, ctx, directives,
        )))
    }

    /// Run directives for a given stage using the flow of the email to determine which one to use.
    /// Used when directives are split by domain and email flow.
    #[allow(clippy::unnecessary_wraps, clippy::needless_pass_by_value)]
    fn run_directives_flow(
        ncc: rhai::NativeCallContext<'_>,
        ctx: &mut State<CONTEXT>,
        domains: rhai::Shared<Domains<STAGE>>,
        flow: rhai::Dynamic,
    ) -> RhaiResult<rhai::Dynamic> {
        let (Some(stage), Some(flow)) = (Self::get_stage(&ncc), flow.try_cast::<Flow>()) else {
            return Ok(rhai::Dynamic::UNIT);
        };

        let Some(directives) = domains
            .get(&flow.domain)
            .and_then(|stages| stages.0.get(&stage))
            .map(|rules| {
                match rules {
                    // Rules not split by flow are executed for any flow type.
                    Directives::Any(rules) => rules,
                    Directives::Flow {
                        inbound,
                        outbound,
                        local,
                    } => match flow.r#type {
                        FlowType::Inbound => inbound,
                        FlowType::Outbound => outbound,
                        FlowType::Local => local,
                    },
                }
            })
        else {
            return Ok(rhai::Dynamic::UNIT);
        };

        Ok(rhai::Dynamic::from(Self::run_directives(
            ncc, ctx, directives,
        )))
    }

    /// Runs all rules for a given stage.
    ///
    /// If no rules are found, the [`Status::no_rules`] value is returned.
    /// If an error occurs at runtime, the [`Status::error`] value is returned.
    /// The base status used to run the script is [`Status::next`].
    #[allow(clippy::needless_pass_by_value)]
    fn run_directives(
        ncc: rhai::NativeCallContext<'_>,
        ctx: &mut State<CONTEXT>,
        directives: &[Directive],
    ) -> STATUS {
        if directives.is_empty() {
            let status = STATUS::no_rules(Self::get_stage(&ncc).unwrap());
            tracing::debug!("No rules defined, returning status {:?} instead", status);
            return status;
        }

        let stage = Self::get_stage(&ncc)
            .as_ref()
            .map_or("unknown".into(), ToString::to_string);

        for directive in directives {
            tracing::trace!(rule = directive.name(), stage, "Executing directive");
            let status = match directive.execute(&ncc, ctx.clone()) {
                Ok(status) => status,
                Err(mut error) => {
                    error.stage = Some(stage);
                    return STATUS::error(error);
                }
            };

            if status != STATUS::next() {
                return status;
            }
        }

        STATUS::next()
    }
}

// Builder functions.
impl<CONTEXT: 'static, STATUS: Status, STAGE: Stage + 'static>
    RuleEngineConfigBuilder<CONTEXT, STATUS, STAGE>
{
    /// Build a configuration module from a configuration object.
    ///
    /// # Errors
    ///
    /// * Failed to serialize the configuration object.
    /// * Failed to transform a configuration object into a Rhai JSON value.
    pub fn with_configuration(mut self, config: &impl vsmtp_config::Config) -> Result<Self> {
        let string_config =
            serde_json::to_string(&config).map_err(RuleEngineConfigBuilderError::Serialize)?;
        let config_module = {
            let mut config_module = rhai::Module::new();
            config_module.set_var(
                "config",
                self.rhai_engine
                    .parse_json(string_config, true)
                    .map_err(RuleEngineConfigBuilderError::ParseJSON)?,
            );
            rhai::Shared::new(config_module)
        };

        self.rhai_engine
            .register_global_module(config_module.clone());

        self.config_module = config_module;

        Ok(self)
    }

    /// Add global Rhai modules to the engine configuration.
    #[must_use]
    pub fn with_global_modules(
        mut self,
        modules: impl IntoIterator<Item = rhai::Shared<rhai::Module>>,
    ) -> Self {
        self.global_modules.extend(modules);
        self
    }

    /// Add standard Rhai modules to the engine configuration.
    #[must_use]
    pub fn with_standard_global_modules(mut self) -> Self {
        self.global_modules
            .push(rhai::packages::StandardPackage::new().as_shared_module());

        self
    }

    /// Add static Rhai modules to the engine configuration.
    #[must_use]
    pub fn with_static_modules(
        mut self,
        modules: impl IntoIterator<Item = (String, rhai::Shared<rhai::Module>)>,
    ) -> Self {
        for i in modules {
            self.static_modules.push(i.clone());
            let (name, module) = i;
            self.rhai_engine.register_static_module(name, module);
        }
        self
    }

    /// Add SMTP static modules declared in `[crate::api::smtp_modules]` to the engine configuration.
    #[must_use]
    pub fn with_smtp_modules(mut self) -> Self {
        self.static_modules.extend(crate::api::smtp_modules());

        self
    }

    /// Add a file and dynamic module resolver to the engine.
    /// The file resolver will look for modules with the `rhai` extension.
    ///
    /// # Arguments
    ///
    /// * `path`   - Rhai modules will be resolved from this directory.
    /// * `stages` - Stages to supply to the domain filter resolver.
    #[must_use]
    pub fn with_default_module_resolvers(mut self, path: impl Into<std::path::PathBuf>) -> Self {
        let path = path.into();
        let mut resolvers = ModuleResolversCollection::new();

        resolvers.push(DomainFilterResolver::<STAGE>::new(path.clone()));
        resolvers.push(FileModuleResolver::new_with_path_and_extension(
            path.clone(),
            "rhai",
        ));
        resolvers.push(DylibModuleResolver::with_path(path));

        self.rhai_engine.set_module_resolver(resolvers);

        self
    }

    /// Add a custom module resolver to the engine.
    ///
    /// # Arguments
    ///
    /// * `resolver` - Rhai resolver to set on the engine
    #[must_use]
    pub fn with_module_resolver(mut self, resolver: impl rhai::ModuleResolver + 'static) -> Self {
        self.rhai_engine.set_module_resolver(resolver);

        self
    }

    /// Compile and add directives from a script file to the engine configuration.
    /// Additional default sources can be passed if the script cannot be found.
    ///
    /// Prefer calling [`Self::with_module_resolver`] before this method.
    ///
    /// # Errors
    /// * Failed to build directives from script.
    pub fn with_script_at(
        mut self,
        path: impl Into<std::path::PathBuf>,
        defaults: impl Into<String>,
    ) -> Result<Self> {
        let path = path.into();
        let path_ptr = std::sync::Arc::new(path.clone());

        let sources = if path.exists() {
            std::fs::read_to_string(path).map_or_else(
                |error| Err(RuleEngineConfigBuilderError::LoadScript(error.to_string())),
                |sources| {
                    tracing::debug!("compiling rhai script {}", path_ptr.display());
                    Ok(sources)
                },
            )?
        } else {
            tracing::warn!("Rhai script not found, vsmtp will use default scripts instead");

            defaults.into()
        };

        self.ast = self
            .rhai_engine
            .compile_into_self_contained(&rhai::Scope::new(), sources)
            .map_err(RuleEngineConfigBuilderError::CompileScript)?;

        Ok(self)
    }

    /// Get a mutable reference on the Rhai engine for further customization.
    pub fn engine(
        mut self,
        f: impl FnOnce(&mut rhai::Engine) -> std::result::Result<(), Box<rhai::EvalAltResult>>,
    ) -> Result<Self> {
        f(&mut self.rhai_engine).map_err(RuleEngineConfigBuilderError::Engine)?;

        Ok(self)
    }

    /// Build the rule engine configuration.
    #[allow(clippy::missing_const_for_fn)] // False positive.
    pub fn build(mut self) -> RuleEngineConfig<CONTEXT, STATUS, STAGE> {
        #[cfg(debug_assertions)]
        {
            // Checking if TypeIDs are the same as plugins.
            let type_id = std::any::TypeId::of::<rhai::ImmutableString>();
            tracing::trace!(?type_id);
        }

        // Registering the rule execution module.
        // NOTE: We do not use a rhai plugin module here because we need to pass a generic
        //       to the functions.
        let rule_module = {
            let mut rule_module = rhai::Module::new();

            rule_module.set_native_fn("run", Self::run_directives_array);
            rule_module.set_native_fn("run", Self::run_directives_vec);
            rule_module.set_native_fn("run", Self::run_directives_domain);
            rule_module.set_native_fn("run", Self::run_directives_flow);
            rule_module.set_native_fn("flow", Self::flow);
            rule_module.set_getter_fn("domain", Self::flow_get_domain);
            rule_module.set_getter_fn("type", Self::flow_get_type);
            rule_module.set_native_fn("==", Self::flow_eq);
            rule_module.set_native_fn("!=", Self::flow_neq);

            rule_module
        };

        self.global_modules.push(rule_module.into());

        RuleEngineConfig {
            config_module: self.config_module,
            global_modules: self.global_modules,
            static_modules: self.static_modules,
            ast: self.ast,
            status: std::marker::PhantomData,
            stage: std::marker::PhantomData,
            state: std::marker::PhantomData,
        }
    }
}

/// Create a rhai engine containing custom parsers and desired configuration
/// for `rhai` scripts.
#[must_use]
fn new_rhai_engine() -> rhai::Engine {
    let mut engine = rhai::Engine::new();

    // NOTE: on_parse_token is not deprecated, just subject to change in future releases.
    #[allow(deprecated)]
    engine.on_parse_token(|token, _, _| {
        match token {
            // remap 'is' operator to '==', it's easier than creating a new operator.
            // NOTE: warning => "is" is a reserved keyword in rhai's tokens, maybe change to "eq" ?
            rhai::Token::Reserved(s) if &*s == "is" => rhai::Token::EqualsTo,
            rhai::Token::Identifier(s) if &*s == "not" => rhai::Token::NotEqualsTo,
            // Pass through all other tokens unchanged
            _ => token,
        }
    });

    engine
        .disable_symbol("eval")
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

    // New operators can be registered into the engine, fast operators
    // to true would break them.
    engine.set_fast_operators(false);

    engine
}
