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

use std::sync::Arc;
use vsmtp_common::delivery_attempt::{DeliveryAttempt, LocalInformation, ShouldNotify};
use vsmtp_common::libc::{chown, getpwuid};
use vsmtp_common::{ctx_delivery::CtxDelivery, delivery_route::DeliveryRoute, uuid};
use vsmtp_config::Config;
use vsmtp_delivery::{delivery_main, DeliverySystem};
use vsmtp_protocol::Address;

#[derive(Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
enum UserLookup {
    #[default]
    LocalPart,
    FullAddress,
}

#[derive(Default, serde::Serialize, serde::Deserialize)]
struct Maildir {
    #[serde(skip, default = "default_maildir_hostname")]
    name: String,
    api_version: vsmtp_config::semver::VersionReq,
    #[serde(default, with = "option_group")]
    group_local: Option<uzers::Group>,
    #[serde(default)]
    user_lookup: UserLookup,
    #[serde(default)]
    broker: vsmtp_config::Broker,
    #[serde(default)]
    logs: vsmtp_config::Logs,
    #[serde(skip)]
    path: std::path::PathBuf,
}

fn default_maildir_hostname() -> String {
    "maildir".to_string()
}

mod option_group {

    pub fn deserialize<'de, D>(d: D) -> Result<Option<uzers::Group>, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        match <Option<String> as serde::Deserialize>::deserialize(d)? {
            Some(group_local) => Ok(Some(uzers::get_group_by_name(&group_local).ok_or_else(
                || serde::de::Error::custom(format!("Group '{group_local}' does not exist.")),
            )?)),
            None => Ok(None),
        }
    }

    pub fn serialize<S>(this: &Option<uzers::Group>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match this {
            Some(group) => serializer.serialize_some(&group.name().to_string_lossy()),
            None => serializer.serialize_none(),
        }
    }
}

impl Maildir {
    #[tracing::instrument(name = "create-maildir", fields(folder = ?path.display()))]
    fn create_and_chown(
        path: &std::path::PathBuf,
        user: &uzers::User,
        group_local: &Option<uzers::Group>,
    ) -> std::io::Result<()> {
        if path.exists() {
            tracing::info!("Folder already exists.");
        } else {
            tracing::debug!("Creating folder.");

            std::fs::create_dir_all(path)?;

            tracing::trace!(
                user = user.uid(),
                group = group_local.as_ref().map_or(u32::MAX, uzers::Group::gid),
                "Setting permissions.",
            );

            chown(
                path,
                Some(user.uid()),
                group_local.as_ref().map(uzers::Group::gid),
            )?;
        }

        Ok(())
    }

    fn write(
        &self,
        addr: &Address,
        user: &uzers::User,
        msg_uuid: &uuid::Uuid,
        content: &[u8],
    ) -> std::io::Result<()> {
        let maildir = std::path::PathBuf::from_iter([getpwuid(user.uid())?, "Maildir".into()]);
        Self::create_and_chown(&maildir, user, &self.group_local)?;
        for dir in ["new", "tmp", "cur"] {
            Self::create_and_chown(&maildir.join(dir), user, &self.group_local)?;
        }

        let file_in_maildir_inbox = maildir.join(format!("new/{msg_uuid}.eml"));

        let email = std::fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&file_in_maildir_inbox)?;

        let mut email_buf = std::io::BufWriter::new(email);
        std::io::Write::write_all(&mut email_buf, format!("Delivered-To: {addr}\n").as_bytes())?;
        std::io::Write::write_all(&mut email_buf, content)?;
        std::io::Write::flush(&mut email_buf)?;

        chown(
            &file_in_maildir_inbox,
            Some(user.uid()),
            self.group_local.as_ref().map(uzers::Group::gid),
        )?;

        Ok(())
    }
}

fn get_notification_supported() -> ShouldNotify {
    ShouldNotify::Success | ShouldNotify::Failure | ShouldNotify::Delay
}

#[async_trait::async_trait]
impl DeliverySystem for Maildir {
    fn name(&self) -> &str {
        &self.name
    }

    fn routing_key(&self) -> DeliveryRoute {
        DeliveryRoute::Maildir
    }

    async fn deliver(self: Arc<Self>, ctx: &CtxDelivery) -> Vec<DeliveryAttempt> {
        let content = ctx.mail.read().unwrap().to_string();
        let mut attempt = vec![];

        for i in &ctx.rcpt_to {
            let user = match self.user_lookup {
                UserLookup::LocalPart => i.forward_path.0.local_part(),
                UserLookup::FullAddress => i.forward_path.0.full(),
            };

            match uzers::get_user_by_name(user)
                .map(|user| self.write(&i.forward_path.0, &user, &ctx.uuid, content.as_bytes()))
            {
                None => {
                    tracing::error!(user, "User does not exist, cannot process delivery");
                    attempt.push(DeliveryAttempt::new_local(
                        i.forward_path.clone(),
                        LocalInformation::NotFound,
                        get_notification_supported(),
                    ));
                }
                Some(Err(e)) => {
                    tracing::error!(user, "Error while writing maildir: {}", e);
                    attempt.push(DeliveryAttempt::new_local(
                        i.forward_path.clone(),
                        e.into(),
                        get_notification_supported(),
                    ));
                }
                Some(Ok(())) => {
                    tracing::info!(user, "Message written to maildir successfully");
                    attempt.push(DeliveryAttempt::new_local(
                        i.forward_path.clone(),
                        LocalInformation::Success,
                        get_notification_supported(),
                    ));
                }
            };
        }

        attempt
    }
}

impl Config for Maildir {
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
    #[arg(short, long, default_value_t = String::from("/etc/vsmtp/maildir/conf.d/config.rhai"))]
    pub config: String,
}

#[tokio::main]
async fn main() {
    let Args { config } = <Args as clap::Parser>::parse();

    let system = match Maildir::from_rhai_file(&config) {
        Ok(cfg) => std::sync::Arc::new(cfg),
        Err(error) => {
            eprintln!("Failed to initialize maildir delivery configuration: {error}");
            return;
        }
    };

    if let Err(error) = delivery_main(system).await {
        tracing::error!("Failed to run maildir delivery: {error}");
    }
}
