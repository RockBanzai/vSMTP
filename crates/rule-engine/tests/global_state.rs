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

mod common;

use rhai::plugin::*;
use vsmtp_config::{broker, logs, queues, semver, Config, ConfigResult};
use vsmtp_rule_engine::*;

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum MyStages {
    MutateState,
    FetchState,
}

impl Stage for MyStages {
    fn hook(&self) -> &'static str {
        match self {
            Self::MutateState => "on_mutate_state",
            Self::FetchState => "on_fetch_state",
        }
    }

    fn stages() -> &'static [&'static str] {
        &["mutate_state", "fetch_state"]
    }
}

impl std::str::FromStr for MyStages {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "mutate_state" => Ok(Self::MutateState),
            "fetch_state" => Ok(Self::FetchState),
            _ => Err(()),
        }
    }
}

impl std::fmt::Display for MyStages {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Self::MutateState => "mutate_state",
                Self::FetchState => "fetch_state",
            }
        )
    }
}

/// Custom status for this rule engine.
#[allow(dead_code)]
#[derive(Debug, PartialEq, Eq, Clone)]
pub enum MyStatus {
    Next(String),
    Stop,
}

/// Implement the [`Status`] trait and defining our own rules
/// for each status.
impl Status for MyStatus {
    fn no_rules(_: impl Stage) -> Self {
        Self::Stop
    }

    fn error(error: DirectiveError) -> Self {
        dbg!(error);
        Self::Stop
    }

    fn next() -> Self {
        Self::Next("default next called".to_string())
    }

    fn is_next(&self) -> bool {
        matches!(self, Self::Next(s) if s == "default next called")
    }
}

// Enable the user to access our statuses.
#[rhai::export_module]
mod status {
    pub const fn next(message: String) -> MyStatus {
        MyStatus::Next(message)
    }

    pub const fn stop() -> MyStatus {
        MyStatus::Stop
    }
}

// Mutate a global state directly from a rule. (email, envelop, etc.)
#[rhai::export_module]
mod global_state_mutator {
    use crate::api::State;

    #[rhai_fn(global, pure)]
    pub fn inc(ctx: &mut State<MyGlobalState>) {
        // Call a function that olds the global state, enabling mutation
        // of a global variable without the need of a parameter.
        ctx.write(|ctx| ctx.value += 1);
    }

    #[rhai_fn(global, get = "value", pure)]
    pub fn value(ctx: &mut State<MyGlobalState>) -> rhai::INT {
        // Call a function that olds the global state, enabling mutation
        // of a global variable without the need of a parameter.
        ctx.read(|ctx| ctx.value) as rhai::INT
    }
}

/// Define a global state that will be mutated by the engine.
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct MyGlobalState {
    pub value: usize,
}

/// Define a service configuration.
#[derive(Default, serde::Serialize, serde::Deserialize)]
struct MyConfig {
    dummy: bool,
}

impl Config for MyConfig {
    fn with_path(_path: &impl AsRef<std::path::Path>) -> ConfigResult<Self>
    where
        Self: Config + serde::de::DeserializeOwned + serde::Serialize,
    {
        Ok(Self::default())
    }

    fn api_version(&self) -> &semver::VersionReq {
        unimplemented!()
    }

    fn broker(&self) -> &broker::Broker {
        unimplemented!()
    }

    fn queues(&self) -> &queues::Queues {
        unimplemented!()
    }

    fn logs(&self) -> &logs::Logs {
        unimplemented!()
    }

    fn path(&self) -> &std::path::Path {
        unimplemented!()
    }
}

#[test]
fn global_state() {
    let config = MyConfig::default();

    let rule_engine_config = std::sync::Arc::new(
        RuleEngineConfigBuilder::<MyGlobalState, MyStatus, MyStages>::default()
            .with_configuration(&config)
            .expect("failed to build the configuration")
            .with_default_module_resolvers(from_manifest_path!("tests/scripts"))
            .with_static_modules([
                ("status".to_string(), rhai::exported_module!(status).into()),
                (
                    "mutator".to_string(),
                    rhai::exported_module!(global_state_mutator).into(),
                ),
            ])
            .with_standard_global_modules()
            .with_smtp_modules()
            .with_script_at(from_manifest_path!("tests/scripts/global_state.rhai"), "")
            .expect("failed to build script global_state.rhai")
            .build(),
    );

    let global_state = MyGlobalState { value: 0 };
    let rule_engine = RuleEngine::from_config_with_state(rule_engine_config, global_state);

    rule_engine.read_state(|v| assert_eq!(v.value, 0));
    assert_eq!(
        rule_engine.run(&MyStages::MutateState),
        MyStatus::Next("state mutated".to_string())
    );
    assert_eq!(
        rule_engine.run(&MyStages::FetchState),
        MyStatus::Next("state fetched".to_string())
    );
    rule_engine.read_state(|v| assert_eq!(v.value, 5));
}
