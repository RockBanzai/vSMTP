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

use futures_util::TryFutureExt;
use vsmtp_common::broker::{Exchange, Queue};
use vsmtp_config::Config;
use vsmtp_protocol::ConnectionKind;
use vsmtp_receiver::smtp::{
    config::SMTPReceiverConfig, rules::api, server::Server, session::Handler,
};
use vsmtp_rule_engine::{
    api::{msa_modules, net_modules, server_auth, utils_modules},
    rhai, RuleEngineConfigBuilder,
};

async fn init(channel: &lapin::Channel) -> lapin::Result<(lapin::Queue, lapin::Queue)> {
    let to_working_queue = channel
        .queue_declare(
            Queue::ToWorking.as_ref(),
            lapin::options::QueueDeclareOptions {
                durable: true,
                ..Default::default()
            },
            lapin::types::FieldTable::default(),
        )
        .await?;

    let all_quarantine = channel
        .queue_declare(
            Queue::Quarantine.as_ref(),
            lapin::options::QueueDeclareOptions {
                durable: true,
                ..Default::default()
            },
            lapin::types::FieldTable::default(),
        )
        .await?;

    channel
        .exchange_declare(
            Exchange::Quarantine.as_ref(),
            lapin::ExchangeKind::Topic,
            lapin::options::ExchangeDeclareOptions {
                durable: true,
                ..Default::default()
            },
            lapin::types::FieldTable::default(),
        )
        .await?;

    channel
        .queue_bind(
            Queue::Quarantine.as_ref(),
            Exchange::Quarantine.as_ref(),
            "rule.*", // all the messages
            lapin::options::QueueBindOptions::default(),
            lapin::types::FieldTable::default(),
        )
        .await?;

    Ok((to_working_queue, all_quarantine))
}

type SocketsConfig = std::collections::HashMap<ConnectionKind, Vec<std::net::SocketAddr>>;
type SocketsBound = std::collections::HashMap<ConnectionKind, Vec<tokio::net::TcpListener>>;

fn vec_to_map<A, B>(input: Vec<(A, B)>) -> std::collections::HashMap<A, Vec<B>>
where
    A: std::hash::Hash + Eq,
{
    let mut out: std::collections::HashMap<A, Vec<B>> = std::collections::HashMap::default();
    for (k, v) in input {
        if let Some(e) = out.get_mut(&k) {
            e.push(v);
        } else {
            out.insert(k, vec![v]);
        }
    }
    out
}

#[tracing::instrument(ret, err)]
async fn bind(config: SocketsConfig) -> Result<SocketsBound, std::io::Error> {
    let futures = config.into_iter().flat_map(|(kind, addr)| {
        addr.into_iter().map(move |addr| {
            tokio::net::TcpListener::bind(addr).map_ok(move |socket| (kind, socket))
        })
    });
    futures_util::future::try_join_all(futures)
        .await
        .map(vec_to_map)
}

/// Builder to separate initialization from the main function.
struct Receiver {
    config: SMTPReceiverConfig,
    conn: std::sync::Arc<lapin::Connection>,
    #[allow(dead_code)]
    channel: lapin::Channel,
    rule_engine_config: std::sync::Arc<
        vsmtp_rule_engine::RuleEngineConfig<
            vsmtp_common::stateful_ctx_received::StatefulCtxReceived,
            vsmtp_receiver::smtp::rules::status::ReceiverStatus,
            vsmtp_receiver::smtp::rules::stages::ReceiverStage,
        >,
    >,
}

#[derive(clap::Parser)]
#[command(author, version, about)]
pub struct Args {
    /// Path to the rhai configuration file.
    #[arg(short, long, default_value_t = String::from("/etc/vsmtp/receiver-smtp/conf.d/config.rhai"))]
    pub config: String,
}

impl Receiver {
    /// Build the configuration, AMQP connections and rule engine for the service.
    async fn build() -> Result<Self, Box<dyn std::error::Error>> {
        let Args { config } = <Args as clap::Parser>::parse();

        let config = SMTPReceiverConfig::from_rhai_file(&config)?;
        let conn = config.broker().connect().await?;
        let conn = std::sync::Arc::new(conn);

        vsmtp_common::init_logs(&conn, config.logs(), "smtp-receiver").await?;

        let channel = conn.create_channel().await?;
        channel
            .confirm_select(lapin::options::ConfirmSelectOptions::default())
            .await?;
        channel
            .basic_qos(1, lapin::options::BasicQosOptions::default())
            .await?;
        let _ = init(&channel).await?;

        let rule_engine_config = std::sync::Arc::new(
            RuleEngineConfigBuilder::default()
                .with_configuration(&config)?
                .with_default_module_resolvers(config.scripts.path.parent().ok_or_else(|| {
                    format!("Invalid script path: {}", config.scripts.path.display())
                })?)
                .with_standard_global_modules()
                .with_global_modules([rhai::packages::Package::as_shared_module(
                    &rhai_rand::RandomPackage::new(),
                )])
                .with_smtp_modules()
                .with_static_modules(
                    [
                        ("code".to_string(), rhai::exported_module!(api::code).into()),
                        (
                            "status".to_string(),
                            rhai::exported_module!(api::status).into(),
                        ),
                    ]
                    .into_iter()
                    .chain(msa_modules())
                    .chain(server_auth())
                    .chain(net_modules())
                    .chain(utils_modules())
                    .chain([
                        vsmtp_rhai_utils::time(),
                        vsmtp_rhai_utils::env(),
                        vsmtp_rhai_utils::process(),
                        vsmtp_rhai_utils::crypto(),
                    ]),
                )
                .with_script_at(
                    &config.scripts.path,
                    include_str!("smtp/rules/defaults/filter.rhai"),
                )?
                .build(),
        );

        Ok(Self {
            config,
            conn,
            channel,
            rule_engine_config,
        })
    }

    /// Run the service.
    async fn run(self) -> Result<(), Box<dyn std::error::Error>> {
        let Self {
            config,
            conn,
            rule_engine_config,
            channel: _,
        } = self;

        let sockets = bind(SocketsConfig::from_iter([
            (ConnectionKind::Relay, config.interfaces.addr.clone()),
            (
                ConnectionKind::Submission,
                config.interfaces.addr_submission.clone(),
            ),
            (
                ConnectionKind::Tunneled,
                config.interfaces.addr_submissions.clone(),
            ),
        ]))
        .await?;

        let config = std::sync::Arc::new(config);
        let rustls_config = if let Some(tls) = &config.tls {
            Some(std::sync::Arc::new(vsmtp_common::tls::get_rustls_config(
                &tls.protocol_version,
                &tls.cipher_suite,
                tls.preempt_cipherlist,
                &config.name,
                tls.root.as_ref(),
                &tls.r#virtual,
            )?))
        } else {
            None
        };

        let server = Server {
            socket: sockets,
            config: config.clone(),
        };

        let on_accept = move |args| async move {
            let channel = conn.create_channel().await.unwrap();
            channel
                .confirm_select(lapin::options::ConfirmSelectOptions::default())
                .await
                .unwrap();
            channel
                .basic_qos(1, lapin::options::BasicQosOptions::default())
                .await
                .unwrap();
            Handler::on_accept(args, rule_engine_config, channel, config, rustls_config)
        };
        tracing::info!("SMTP server is listening");
        server.listen(on_accept).await;
        tracing::info!("SMTP server has stop");

        Ok(())
    }
}

#[tokio::main]
async fn main() {
    let receiver = match Receiver::build().await {
        Ok(receiver) => receiver,
        Err(error) => {
            eprintln!("Failed to boot SMTP Receiver service: {error}");
            return;
        }
    };

    if let Err(e) = receiver.run().await {
        tracing::error!(?e, "Failed to run SMTP receiver");
    }
}
