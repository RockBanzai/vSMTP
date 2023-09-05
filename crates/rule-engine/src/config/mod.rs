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

pub mod builder;

/// A "template" for light weight rule engines.
///
/// It stores pre-compiled scripts, modules and configuration
/// to spawn cheap rule engines in case of a multi-threaded
/// environment.
///
/// Call the [`RuleEngineConfigBuilder`] to create an instance.
#[allow(clippy::module_name_repetitions)]
#[derive(Debug)]
pub struct RuleEngineConfig<STATE: 'static, STATUS: crate::Status, STAGE: crate::Stage> {
    pub(super) config_module: rhai::Shared<rhai::Module>,
    pub(super) global_modules: Vec<rhai::Shared<rhai::Module>>,
    pub(super) static_modules: Vec<(String, rhai::Shared<rhai::Module>)>,
    pub(super) ast: rhai::AST,
    status: std::marker::PhantomData<STATUS>,
    stage: std::marker::PhantomData<STAGE>,
    state: std::marker::PhantomData<STATE>,
}
