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
use ::vsmtp_common::{ctx_received::CtxReceived, stateful_ctx_received::StatefulCtxReceived};
use rhai::plugin::*;
use vsmtp_config::{broker, logs, queues, semver, Config, ConfigResult};
use vsmtp_rule_engine::*;

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum MyStages {
    ConfigCheck,
}

impl Stage for MyStages {
    fn hook(&self) -> &'static str {
        match self {
            Self::ConfigCheck => "on_config_check",
        }
    }

    fn stages() -> &'static [&'static str] {
        &["config_check"]
    }
}

impl std::str::FromStr for MyStages {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "config_check" => Ok(Self::ConfigCheck),
            _ => Err(()),
        }
    }
}

impl std::fmt::Display for MyStages {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "config_check")
    }
}

/// Custom status for this rule engine.
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
        matches!(self, Self::Next(_))
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

/// Define a service configuration.
#[derive(Default, serde::Serialize, serde::Deserialize)]
struct MyConfig {
    pub foo: String,
    pub bar: Bar,
}

#[derive(Default, serde::Serialize, serde::Deserialize)]
struct Bar {
    pub x: usize,
    pub y: usize,
}

impl Config for MyConfig {
    fn with_path(_: &impl AsRef<std::path::Path>) -> ConfigResult<Self>
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
fn configuration() {
    let config = MyConfig {
        foo: "foo".to_string(),
        bar: Bar { x: 15, y: 50 },
    };

    let rule_engine_config = std::sync::Arc::new(
        RuleEngineConfigBuilder::<StatefulCtxReceived, MyStatus, MyStages>::default()
            .with_configuration(&config)
            .expect("failed to build the configuration")
            .with_default_module_resolvers(from_manifest_path!("tests/scripts"))
            .with_static_modules([("status".to_string(), rhai::exported_module!(status).into())])
            .with_standard_global_modules()
            .with_smtp_modules()
            .with_script_at(from_manifest_path!("tests/scripts/config.rhai"), "")
            .expect("failed to build script config.rhai")
            .build(),
    );

    let context = StatefulCtxReceived::Complete(CtxReceived::fake());

    let rule_engine = RuleEngine::from_config_with_state(rule_engine_config, context);

    assert_eq!(
        rule_engine.run(&MyStages::ConfigCheck),
        MyStatus::Next("configuration successful".to_string())
    );
}
