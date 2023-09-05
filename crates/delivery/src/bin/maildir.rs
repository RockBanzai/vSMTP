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
use vsmtp_common::delivery_attempt::{DeliveryAttempt, LocalInformation};
use vsmtp_common::libc::{chown, getpwuid};
use vsmtp_common::{ctx_delivery::CtxDelivery, delivery_route::DeliveryRoute, uuid};
use vsmtp_delivery::{delivery_main, DeliverySystem, ShouldNotify};
use vsmtp_protocol::Address;

#[derive(serde::Deserialize)]
#[serde(rename_all = "snake_case")]
enum UserLookup {
    LocalPart,
    FullAddress,
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Maildir {
    #[serde(default, deserialize_with = "deser_option_group")]
    group_local: Option<users::Group>,
    user_lookup: UserLookup,
}

fn deser_option_group<'de, D>(d: D) -> Result<Option<users::Group>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    match <Option<String> as serde::Deserialize>::deserialize(d)? {
        Some(group_local) => Ok(Some(users::get_group_by_name(&group_local).ok_or_else(
            || serde::de::Error::custom(format!("Group '{group_local}' does not exist.")),
        )?)),
        None => Ok(None),
    }
}

impl Maildir {
    #[tracing::instrument(name = "create-maildir", fields(folder = ?path.display()))]
    fn create_and_chown(
        path: &std::path::PathBuf,
        user: &users::User,
        group_local: &Option<users::Group>,
    ) -> std::io::Result<()> {
        if path.exists() {
            tracing::info!("Folder already exists.");
        } else {
            tracing::debug!("Creating folder.");

            std::fs::create_dir_all(path)?;

            tracing::trace!(
                user = user.uid(),
                group = group_local.as_ref().map_or(u32::MAX, users::Group::gid),
                "Setting permissions.",
            );

            chown(
                path,
                Some(user.uid()),
                group_local.as_ref().map(users::Group::gid),
            )?;
        }

        Ok(())
    }

    fn write(
        &self,
        addr: &Address,
        user: &users::User,
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
            self.group_local.as_ref().map(users::Group::gid),
        )?;

        Ok(())
    }
}

#[async_trait::async_trait]
impl DeliverySystem for Maildir {
    fn routing_key(&self) -> DeliveryRoute {
        DeliveryRoute::Maildir
    }

    fn get_notification_supported() -> ShouldNotify {
        ShouldNotify {
            on_success: true,
            on_failure: true,
            on_delay: true,
        }
    }

    async fn deliver(self: Arc<Self>, ctx: &CtxDelivery) -> Vec<DeliveryAttempt> {
        let content = ctx.mail.read().unwrap().to_string();
        let mut attempt = vec![];

        for i in &ctx.rcpt_to {
            let user = match self.user_lookup {
                UserLookup::LocalPart => i.forward_path.0.local_part(),
                UserLookup::FullAddress => i.forward_path.0.full(),
            };

            match users::get_user_by_name(user)
                .map(|user| self.write(&i.forward_path.0, &user, &ctx.uuid, content.as_bytes()))
            {
                None => {
                    tracing::error!(user, "User does not exist, cannot process delivery");
                    attempt.push(DeliveryAttempt::new_local(
                        i.clone(),
                        LocalInformation::NotFound,
                    ));
                }
                Some(Err(e)) => {
                    tracing::error!(user, "Error while writing maildir: {}", e);
                    attempt.push(DeliveryAttempt::new_local(i.clone(), e.into()));
                }
                Some(Ok(())) => {
                    tracing::info!(user, "Message written to maildir successfully");
                    attempt.push(DeliveryAttempt::new_local(
                        i.clone(),
                        LocalInformation::Success,
                    ));
                }
            };
        }

        attempt
    }
}

#[derive(clap::Parser)]
#[command(author, version, about)]
struct Args {}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    <Args as clap::Parser>::parse();

    let system = std::env::var("SYSTEM").expect("SYSTEM");
    let system = std::sync::Arc::from(serde_json::from_str::<Maildir>(&system)?);

    delivery_main(system).await
}
