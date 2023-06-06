/*
 * vSMTP mail transfer agent
 * Copyright (C) 2022 viridIT SAS
 *
 * This program is free software: you can redistribute it and/or modify it under
 * the terms of the GNU General Public License as published by the Free Software
 * Foundation, either version 3 of the License, or any later version.
 *
 * This program is distributed in the hope that it will be useful, but WITHOUT
 * ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
 * FOR A PARTICULAR PURPOSE.  See the GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License along with
 * this program. If not, see https://www.gnu.org/licenses/.
 *
*/
use crate::{
    config::field::{FieldQueueDelivery, FieldQueueWorking},
    Config,
};
use vsmtp_common::{collection, Stage};

#[test]
fn parse() {
    let path_to_config = std::path::PathBuf::from_iter([
        env!("CARGO_MANIFEST_DIR"),
        "../../../examples/config/secured.vsl",
    ]);

    pretty_assertions::assert_eq!(
        Config::from_vsl_file(&path_to_config).unwrap(),
        Config::builder()
            .with_version_str(&format!(">={}, <3.0.0", env!("CARGO_PKG_VERSION")))
            .unwrap()
            .with_path(path_to_config)
            .with_hostname_and_client_count_max(8)
            .with_default_user_and_thread_pool(
                std::num::NonZeroUsize::new(3).unwrap(),
                std::num::NonZeroUsize::new(3).unwrap(),
                std::num::NonZeroUsize::new(3).unwrap()
            )
            .with_ipv4_localhost()
            .with_default_logs_settings()
            .with_spool_dir_and_queues(
                "/var/spool/vsmtp",
                FieldQueueWorking { channel_size: 16 },
                FieldQueueDelivery {
                    channel_size: 16,
                    deferred_retry_max: 10,
                    deferred_retry_period: std::time::Duration::from_secs(600)
                }
            )
            .without_tls_support()
            .with_rcpt_count_and_default(25)
            .with_error_handler_and_timeout(
                5,
                10,
                std::time::Duration::from_millis(50_000),
                &collection! {
                    Stage::Connect => std::time::Duration::from_millis(50),
                    Stage::Helo => std::time::Duration::from_millis(100),
                    Stage::MailFrom => std::time::Duration::from_millis(200),
                    Stage::RcptTo => std::time::Duration::from_millis(400),
                }
            )
            .with_default_extensions()
            .with_default_app()
            .with_default_vsl_settings()
            .with_default_app_logs()
            .with_dns(
                {
                    let mut cfg = trust_dns_resolver::config::ResolverConfig::new();

                    cfg.set_domain(
                        <trust_dns_resolver::Name as std::str::FromStr>::from_str(
                            "example.dns.com",
                        )
                        .unwrap(),
                    );

                    cfg
                },
                crate::field::ResolverOptsWrapper::default()
            )
            .without_virtual_entries()
            .validate()
    );
}
