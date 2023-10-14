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

use ::vsmtp_common::{
    ctx_received::CtxReceived, delivery_route::DeliveryRoute,
    stateful_ctx_received::StatefulCtxReceived, Mailbox,
};
use vsmtp_config::{broker, logs, semver, Config};
use vsmtp_protocol::Address;
use vsmtp_rule_engine::{
    rhai::plugin::*, DirectiveError, RuleEngine, RuleEngineConfigBuilder, Stage, Status,
};

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum MyStages {
    Connect,
    Helo,
    MailFrom,
    RcptTo,
}

impl Stage for MyStages {
    fn hook(&self) -> &'static str {
        match self {
            Self::Connect => "on_connect",
            Self::Helo => "on_helo",
            Self::MailFrom => "on_mail_from",
            Self::RcptTo => "on_rcpt_to",
        }
    }

    fn stages() -> &'static [&'static str] {
        &["connect", "helo", "mail_from", "rcpt_to"]
    }
}

impl std::str::FromStr for MyStages {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "connect" => Ok(Self::Connect),
            "helo" => Ok(Self::Helo),
            "mail_from" => Ok(Self::MailFrom),
            "rcpt_to" => Ok(Self::RcptTo),
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
                Self::Connect => "connect",
                Self::Helo => "helo",
                Self::MailFrom => "mail_from",
                Self::RcptTo => "rcpt_to",
            }
        )
    }
}

/// Custom status for this rule engine.
#[allow(dead_code)]
#[derive(Debug, PartialEq, Eq, Clone)]
pub enum MyStatus {
    Ok(Option<String>),
    Fail(Option<String>),
}

/// Implement the [`Status`] trait and defining our own rules
/// for each status.
impl Status for MyStatus {
    fn no_rules(_: impl Stage) -> Self {
        Self::Fail(None)
    }

    fn error(error: DirectiveError) -> Self {
        dbg!(error);
        Self::Fail(None)
    }

    fn next() -> Self {
        Self::Ok(None)
    }

    fn is_next(&self) -> bool {
        matches!(self, Self::Ok(None))
    }
}

// Enable the user to access our statuses.
#[rhai::export_module]
mod status {
    use super::*;

    #[rhai_fn(name = "ok")]
    pub const fn ok() -> MyStatus {
        MyStatus::Ok(None)
    }

    #[rhai_fn(name = "ok")]
    pub fn ok_with_message(message: &str) -> MyStatus {
        MyStatus::Ok(Some(message.into()))
    }

    #[rhai_fn(name = "fail")]
    pub const fn fail() -> MyStatus {
        MyStatus::Fail(None)
    }

    #[rhai_fn(name = "fail")]
    pub fn fail_with_message(message: &str) -> MyStatus {
        MyStatus::Fail(Some(message.into()))
    }
}

/// Define a service configuration.
#[derive(Default, serde::Serialize, serde::Deserialize)]
struct MyConfig {
    dummy: bool,
}

impl Config for MyConfig {
    fn api_version(&self) -> &semver::VersionReq {
        unimplemented!()
    }

    fn broker(&self) -> &broker::Broker {
        unimplemented!()
    }

    fn logs(&self) -> &logs::Logs {
        unimplemented!()
    }

    fn path(&self) -> &std::path::Path {
        unimplemented!()
    }
}

#[allow(clippy::too_many_lines)]
#[test]
fn resolve_file() {
    let rule_engine_config = std::sync::Arc::new(
        RuleEngineConfigBuilder::<StatefulCtxReceived, MyStatus, MyStages>::default()
            .with_configuration(&MyConfig { dummy: false })
            .expect("failed to build processing config")
            .with_default_module_resolvers(from_manifest_path!("tests/scripts/module-resolver"))
            .with_standard_global_modules()
            .with_smtp_modules()
            .with_static_modules([("status".to_string(), rhai::exported_module!(status).into())])
            .engine(|engine| {
                engine.on_print(|message| {
                    dbg!(message);
                });
                Ok(())
            })
            .unwrap()
            .with_script_at(
                from_manifest_path!("tests/scripts/module-resolver/script.rhai"),
                "",
            )
            .expect("failed to compile processing rules")
            .build(),
    );

    let context = StatefulCtxReceived::Complete(CtxReceived::fake());

    let engine = RuleEngine::from_config_with_state(rule_engine_config, context);

    assert_eq!(engine.run(&MyStages::Connect), MyStatus::Ok(None));
    assert_eq!(
        engine.run(&MyStages::Helo),
        MyStatus::Ok(Some("helo accepted".into()))
    );
    let expected = engine.read_state(|s| {
        if s.get_mail_from()
            .unwrap()
            .reverse_path
            .as_ref()
            .is_some_and(|reverse_path| reverse_path.domain() == "example.com".parse().unwrap())
        {
            MyStatus::Ok(None)
        } else {
            MyStatus::Fail(Some("default used".into()))
        }
    });
    assert_eq!(engine.run(&MyStages::MailFrom), expected);
    let expected = engine.read_state(|s| {
        if s.get_mail_from()
            .unwrap()
            .reverse_path
            .as_ref()
            .is_some_and(|reverse_path| reverse_path.domain() == "example.com".parse().unwrap())
        {
            MyStatus::Ok(None)
        } else {
            MyStatus::Fail(Some("550 5.7.1 Relaying not allowed".into()))
        }
    });
    assert_eq!(engine.run(&MyStages::RcptTo), expected);

    // lets try out the email flow.
    // Outbound
    engine.write_state(|context| {
        context.mut_mail_from().unwrap().reverse_path = Some(Mailbox(Address::new_unchecked(
            "someone@dummy.org".to_string(),
        )));

        let rcpt = context.mut_rcpt_to().unwrap();

        rcpt.recipient.clear();
        rcpt.add_recipient_with_route(
            Mailbox(Address::new_unchecked("someone@test.org".to_string())),
            DeliveryRoute::Basic,
        );
    });

    // Must execute dummy.org/rcpt_to/outbound rules.
    assert_eq!(
        engine.run(&MyStages::RcptTo),
        MyStatus::Ok(Some("250 Outbound emails are authorized".into()))
    );

    // Inbound
    engine.write_state(|context| {
        context.mut_mail_from().unwrap().reverse_path = Some(Mailbox(Address::new_unchecked(
            "someone@test.org".to_string(),
        )));

        let rcpt = context.mut_rcpt_to().unwrap();

        rcpt.recipient.clear();
        rcpt.add_recipient_with_route(
            Mailbox(Address::new_unchecked("someone@dummy.org".to_string())),
            DeliveryRoute::Basic,
        );
    });

    assert_eq!(
        engine.run(&MyStages::RcptTo),
        MyStatus::Ok(Some("250 your email has been analyzed".parse().unwrap()))
    );

    // Local
    engine.write_state(|context| {
        context.mut_mail_from().unwrap().reverse_path = Some(Mailbox(Address::new_unchecked(
            "someone@dummy.org".to_string(),
        )));

        let rcpt = context.mut_rcpt_to().unwrap();

        rcpt.recipient.clear();
        rcpt.add_recipient_with_route(
            Mailbox(Address::new_unchecked("someone@dummy.org".to_string())),
            DeliveryRoute::Basic,
        );
    });

    assert_eq!(
        engine.run(&MyStages::RcptTo),
        MyStatus::Ok(Some("250 All locals are accepted".parse().unwrap()))
    );

    // Relay
    engine.write_state(|context| {
        context.mut_mail_from().unwrap().reverse_path = Some(Mailbox(Address::new_unchecked(
            "someone@test.org".to_string(),
        )));

        let rcpt = context.mut_rcpt_to().unwrap();

        rcpt.recipient.clear();
        rcpt.add_recipient_with_route(
            Mailbox(Address::new_unchecked("someone@google.com".to_string())),
            DeliveryRoute::Basic,
        );
    });

    let expected = engine.read_state(|s| {
        if s.get_mail_from()
            .unwrap()
            .reverse_path
            .as_ref()
            .is_some_and(|reverse_path| reverse_path.domain() == "example.com".parse().unwrap())
        {
            MyStatus::Ok(None)
        } else {
            MyStatus::Fail(Some("550 5.7.1 Relaying not allowed".into()))
        }
    });
    assert_eq!(engine.run(&MyStages::RcptTo), expected);
}
