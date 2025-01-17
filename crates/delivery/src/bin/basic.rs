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

use vsmtp_auth::TlsCertificate;
use vsmtp_common::{
    ctx_delivery::CtxDelivery,
    delivery_attempt::{DeliveryAttempt, RemoteInformation, RemoteMailExchange, ShouldNotify},
    delivery_route::DeliveryRoute,
    dns_resolver::DnsResolver,
    stateful_ctx_received::MailFromProps,
    Recipient,
};
use vsmtp_config::Config;
use vsmtp_delivery::{delivery_main, send, DeliverySystem, Tls};
use vsmtp_protocol::{ClientName, Domain};

/// The [`Basic`] implementation of the delivery system.
///
/// It has been designed to be as simple as possible:
/// * group the recipients by domain (meaning it support multiple domains per message)
/// * for each domain, lookup the MX records and take the MX with the higher priority
/// * make only one attempt to send the message to that MX
#[derive(serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Basic {
    #[serde(default = "default_service_name", skip)]
    name: String,
    api_version: vsmtp_config::semver::VersionReq,
    dns: DnsResolver,
    tls: Tls,
    #[serde(default)]
    broker: vsmtp_config::Broker,
    #[serde(default)]
    logs: vsmtp_config::Logs,
    #[serde(skip)]
    path: std::path::PathBuf,
    extra_root_ca: Option<std::sync::Arc<TlsCertificate>>,
}

fn get_notification_supported() -> ShouldNotify {
    ShouldNotify::Failure | ShouldNotify::Delay
}

impl Basic {
    // TODO: null mx record (with optional fallback on A/AAAA record)
    #[tracing::instrument(
        skip(self, mail_from, rcpt_to, mail),
        fields(rcpt_count = rcpt_to.len())
        ret,
        level = "debug"
    )]
    async fn send_to_one_domain(
        &self,
        domain: Domain,
        mail_from: MailFromProps,
        rcpt_to: Vec<&Recipient>,
        mail: &[u8],
    ) -> DeliveryAttempt {
        let mxs = match self
            .dns
            .resolver
            .mx_lookup::<hickory_resolver::Name>(domain.clone().into())
            .await
        {
            Ok(records) => records,
            Err(e)
                if matches!(
                    e.kind(),
                    hickory_resolver::error::ResolveErrorKind::NoRecordsFound { .. }
                ) =>
            {
                return DeliveryAttempt::new_remote(
                    rcpt_to
                        .into_iter()
                        .map(|i| i.forward_path.clone())
                        .collect::<Vec<_>>(),
                    RemoteInformation::DnsMxLookup { error: e.into() },
                    get_notification_supported(),
                );
            }
            // TODO: handle other dns errors
            Err(e) => todo!("{e:?}"),
        };

        let mut records = mxs.into_iter().collect::<Vec<_>>();
        records.sort_by_key(hickory_resolver::proto::rr::rdata::MX::preference);

        // TODO: null MX

        // NOTE: we know there is at least one MX ??
        let mx = records.first().unwrap();

        let ips = match self.dns.resolver.lookup_ip(mx.exchange().clone()).await {
            Ok(records) => records,
            Err(e) => {
                return DeliveryAttempt::new_remote(
                    rcpt_to
                        .into_iter()
                        .map(|i| i.forward_path.clone())
                        .collect::<Vec<_>>(),
                    RemoteInformation::DnsMxIpLookup {
                        mx: RemoteMailExchange {
                            mx: mx.exchange().clone().into(),
                            mx_priority: mx.preference(),
                        },
                        error: e.into(),
                    },
                    get_notification_supported(),
                );
            }
        };

        // NOTE: we know there is at least one IP ??
        let ip = ips.iter().next().unwrap();

        send(
            std::net::SocketAddr::new(ip, 25),
            domain,
            ClientName::Domain(hostname::get().unwrap().to_string_lossy().parse().unwrap()),
            mail_from.clone(),
            rcpt_to.into_iter().cloned().collect::<Vec<_>>(),
            Some(RemoteMailExchange {
                mx: mx.exchange().clone().into(),
                mx_priority: mx.preference(),
            }),
            mail,
            self.tls.clone(),
            self.extra_root_ca.clone(),
        )
        .await
    }
}

#[async_trait::async_trait]
impl DeliverySystem for Basic {
    fn name(&self) -> &str {
        &self.name
    }

    fn routing_key(&self) -> DeliveryRoute {
        DeliveryRoute::Basic
    }

    async fn deliver(self: std::sync::Arc<Self>, ctx: &CtxDelivery) -> Vec<DeliveryAttempt> {
        let rcpt_to = ctx.get_undelivered_rcpt();

        let mut rcpt_by_domain = std::collections::HashMap::<Domain, Vec<&Recipient>>::new();
        for i in rcpt_to {
            #[allow(clippy::option_if_let_else)]
            if let Some(rcpt) = rcpt_by_domain.get_mut(&i.forward_path.domain()) {
                rcpt.push(i);
            } else {
                rcpt_by_domain.insert(i.forward_path.domain(), vec![i]);
            }
        }

        let mail = ctx.mail.read().unwrap().to_string();

        let deliveries = rcpt_by_domain.into_iter().map(|(domain, rcpt_to)| {
            self.send_to_one_domain(domain, ctx.mail_from.clone(), rcpt_to, mail.as_bytes())
        });

        futures_util::future::join_all(deliveries).await
    }
}

impl Default for Basic {
    fn default() -> Self {
        Self {
            name: default_service_name(),
            dns: DnsResolver::google(),
            api_version: vsmtp_config::semver::VersionReq::default(),
            broker: vsmtp_config::Broker::default(),
            logs: vsmtp_config::Logs::default(),
            path: std::path::PathBuf::default(),
            tls: Tls::default(),
            extra_root_ca: None,
        }
    }
}

fn default_service_name() -> String {
    "sender-basic".to_string()
}

impl Config for Basic {
    fn api_version(&self) -> &vsmtp_config::semver::VersionReq {
        &self.api_version
    }

    fn broker(&self) -> &vsmtp_config::Broker {
        &self.broker
    }

    fn logs(&self) -> &vsmtp_config::logs::Logs {
        &self.logs
    }

    fn path(&self) -> &std::path::Path {
        &self.path
    }
}

#[derive(clap::Parser)]
#[command(author, version, about)]
struct Args {
    /// Path to the rhai configuration file.
    #[arg(short, long, default_value_t = String::from("/etc/vsmtp/basic/conf.d/config.rhai"))]
    pub config: String,
}

#[tokio::main]
async fn main() {
    let Args { config } = <Args as clap::Parser>::parse();

    let system = match Basic::from_rhai_file(&config) {
        Ok(cfg) => std::sync::Arc::new(cfg),
        Err(error) => {
            eprintln!("Failed to initialize basic delivery configuration: {error}");
            return;
        }
    };

    if let Err(error) = delivery_main(system).await {
        tracing::error!("Failed to run basic delivery: {error}");
    }
}
