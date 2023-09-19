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

use vsmtp_common::tls::{secret::Secret, CipherSuite, ProtocolVersion};
use vsmtp_config::{logs, semver, Broker, Config, ConfigResult, Logs, Queues};
use vsmtp_protocol::{rustls, Domain};

pub mod cli;
pub const SUBMIT_TO: &str = "working";

/// Configuration for the SMTP receiver.
#[derive(serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SMTPReceiverConfig {
    pub api_version: semver::VersionReq,
    /// Name of the server. Used when identifying itself to the client.
    #[serde(default = "SMTPReceiverConfig::default_name")]
    pub name: String,
    /// Listeners.
    #[serde(default)]
    pub interfaces: Interfaces,
    /// Extension to enable and display on the EHLO command.
    #[serde(default)]
    pub esmtp: Esmtp,
    /// Error counts handling.
    #[serde(default)]
    pub errors: Errors,
    /// Maximum number of clients that can connect at the same time.
    #[serde(default = "SMTPReceiverConfig::default_max_client")]
    pub max_clients: i64,
    /// Maximum size of the message in bytes.
    #[serde(default = "SMTPReceiverConfig::default_message_size_limit")]
    pub message_size_limit: usize,
    /// TLS parameters.
    #[serde(default)]
    pub tls: Option<Tls>,
    /// Queue names to redirect or forward the email.
    #[serde(default = "SMTPReceiverConfig::default_queues")]
    pub queues: Queues,
    /// Filters configuration.
    #[serde(default)]
    pub scripts: Scripts,
    /// Application data location on disk. (quarantine, email write, context dump, etc.)
    #[serde(default = "SMTPReceiverConfig::default_storage")]
    pub storage: std::path::PathBuf,
    /// AMQP client configuration.
    #[serde(default)]
    pub broker: Broker,
    /// logging configuration.
    #[serde(default)]
    pub logs: Logs,
    #[serde(skip)]
    /// Path to the configuration script.
    pub path: std::path::PathBuf,
}

impl SMTPReceiverConfig {
    fn default_name() -> String {
        "vsmtp".to_string()
    }

    /// Unlimited clients by default.
    const fn default_max_client() -> i64 {
        -1
    }

    const fn default_message_size_limit() -> usize {
        20_000_000
    }

    fn default_queues() -> Queues {
        Queues {
            submit: Some(SUBMIT_TO.to_string()),
            ..Default::default()
        }
    }

    fn default_storage() -> std::path::PathBuf {
        "/var/vsmtp/storage".into()
    }
}

impl Default for SMTPReceiverConfig {
    fn default() -> Self {
        Self {
            api_version: semver::VersionReq::default(),
            name: SMTPReceiverConfig::default_name(),
            interfaces: Interfaces::default(),
            esmtp: Esmtp::default(),
            errors: Errors::default(),
            max_clients: SMTPReceiverConfig::default_max_client(),
            message_size_limit: SMTPReceiverConfig::default_message_size_limit(),
            tls: None,
            queues: SMTPReceiverConfig::default_queues(),
            scripts: Scripts::default(),
            storage: SMTPReceiverConfig::default_storage(),
            broker: Broker::default(),
            logs: Logs::default(),
            path: std::path::PathBuf::default(),
        }
    }
}

/// Listeners that receives trafic via SMTP.
#[derive(Default, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Interfaces {
    /// 25.
    #[serde(default)]
    pub addr: Vec<std::net::SocketAddr>,
    /// 587.
    #[serde(default)]
    pub addr_submission: Vec<std::net::SocketAddr>,
    /// 465.
    #[serde(default)]
    pub addr_submissions: Vec<std::net::SocketAddr>,
}

/// Error handling for clients.
#[derive(serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Errors {
    /// Soft error count before dropping the email. -1 to disable.
    #[serde(default = "Errors::default_soft_count")]
    pub soft_count: i64,
    /// hard error count before dropping the email. -1 to disable.
    #[serde(default = "Errors::default_hard_count")]
    pub hard_count: i64,
    /// Delay between errors.
    #[serde(default = "Errors::default_delay", with = "humantime_serde")]
    pub delay: std::time::Duration,
}

impl Errors {
    const fn default_soft_count() -> i64 {
        10
    }

    /// Unlimited clients by default.
    const fn default_hard_count() -> i64 {
        10
    }

    const fn default_delay() -> std::time::Duration {
        std::time::Duration::from_secs(2)
    }
}

impl Default for Errors {
    fn default() -> Self {
        Self {
            soft_count: Self::default_soft_count(),
            hard_count: Self::default_hard_count(),
            delay: Self::default_delay(),
        }
    }
}

/// TLS parameters.
#[derive(serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Tls {
    /// Ignore the client’s ciphersuite order.
    /// Instead, choose the top ciphersuite in the server list which is supported by the client.
    #[serde(default)]
    pub preempt_cipherlist: bool,
    /// Timeout for the TLS handshake. Sending a timeout reply to the client.
    #[serde(default = "Tls::default_handshake_timeout", with = "humantime_serde")]
    pub handshake_timeout: std::time::Duration,
    /// TLS protocol supported.
    #[serde(default)]
    pub protocol_version: Vec<ProtocolVersion>,
    /// TLS cipher suite supported.
    #[serde(default = "Tls::default_cipher_suite")]
    pub cipher_suite: Vec<CipherSuite>,
    /// Certificate used by default if no SNI parameter is provided by the client.
    #[serde(default)]
    pub root: Option<Secret>,
    /// Virtual domain used by the server for Server Name Identification (SNI).
    #[serde(default)]
    pub r#virtual: std::collections::BTreeMap<Domain, Secret>,
}

impl Default for Tls {
    fn default() -> Self {
        Self {
            preempt_cipherlist: Default::default(),
            handshake_timeout: Self::default_handshake_timeout(),
            protocol_version: Vec::default(),
            cipher_suite: Self::default_cipher_suite(),
            root: Option::default(),
            r#virtual: std::collections::BTreeMap::default(),
        }
    }
}

impl Tls {
    pub(crate) fn default_cipher_suite() -> Vec<CipherSuite> {
        [
            // TLS1.3 suites
            rustls::CipherSuite::TLS13_AES_256_GCM_SHA384,
            rustls::CipherSuite::TLS13_AES_128_GCM_SHA256,
            rustls::CipherSuite::TLS13_CHACHA20_POLY1305_SHA256,
            // TLS1.2 suites
            rustls::CipherSuite::TLS_ECDHE_ECDSA_WITH_AES_256_GCM_SHA384,
            rustls::CipherSuite::TLS_ECDHE_ECDSA_WITH_AES_128_GCM_SHA256,
            rustls::CipherSuite::TLS_ECDHE_ECDSA_WITH_CHACHA20_POLY1305_SHA256,
            rustls::CipherSuite::TLS_ECDHE_RSA_WITH_AES_256_GCM_SHA384,
            rustls::CipherSuite::TLS_ECDHE_RSA_WITH_AES_128_GCM_SHA256,
            rustls::CipherSuite::TLS_ECDHE_RSA_WITH_CHACHA20_POLY1305_SHA256,
        ]
        .into_iter()
        .map(CipherSuite)
        .collect::<Vec<_>>()
    }

    pub(crate) const fn default_handshake_timeout() -> std::time::Duration {
        std::time::Duration::from_secs(1)
    }
}

/// Scripts location and parameters.
#[derive(Default, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Scripts {
    #[serde(default = "Scripts::default_script_path")]
    pub path: std::path::PathBuf,
}

impl Scripts {
    fn default_script_path() -> std::path::PathBuf {
        <std::path::PathBuf as std::str::FromStr>::from_str("/etc/vsmtp/receiver-smtp/filter.rhai")
            .expect("infallible")
    }
}

/// Extended Simple Mail Transfer Protocol (ESMTP) options.
#[derive(Debug, serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub struct Esmtp {
    /// Authentication policy.
    #[serde(default = "Esmtp::default_auth")]
    pub auth: Option<Auth>,
    // TODO:
    // /// Enable 8BITMIME.
    // #[serde(default = "Esmtp::default_eightbitmime")]
    // pub eightbitmime: bool,
    // /// Enable SMTPUTF8.
    // #[serde(default = "Esmtp::default_smtputf8")]
    // pub smtputf8: bool,
    /// Enable starttls.
    #[serde(default = "Esmtp::default_starttls")]
    pub starttls: bool,
    /// Enable pipelining.
    #[serde(default = "Esmtp::default_pipelining")]
    pub pipelining: bool,
    // TODO:
    // /// Enable chunking.
    // #[serde(default = "Esmtp::default_chunking")]
    // pub chunking: bool,
    /// Maximum size of the message.
    #[serde(default = "Esmtp::default_size")]
    pub size: usize,
    /// DSN
    #[serde(default = "Esmtp::default_dsn")]
    pub dsn: bool,
}

impl Esmtp {
    pub(crate) const fn default_auth() -> Option<Auth> {
        None
    }

    // pub(crate) const fn default_eightbitmime() -> bool {
    //     true
    // }

    // pub(crate) const fn default_smtputf8() -> bool {
    //     true
    // }

    pub(crate) const fn default_starttls() -> bool {
        true
    }

    pub(crate) const fn default_pipelining() -> bool {
        true
    }

    // pub(crate) const fn default_chunking() -> bool {
    //     false
    // }

    pub(crate) const fn default_size() -> usize {
        20_000_000
    }

    pub(crate) const fn default_dsn() -> bool {
        true
    }
}

impl Default for Esmtp {
    fn default() -> Self {
        Self {
            auth: Self::default_auth(),
            starttls: Self::default_starttls(),
            pipelining: Self::default_pipelining(),
            size: Self::default_size(),
            dsn: Self::default_dsn(),
        }
    }
}

/// Authentication options.
#[derive(Debug, serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub struct Auth {
    /// Some mechanisms are considered unsecure under non-TLS connections.
    /// If `false`, the server will allow to use them even on clair connections.
    ///
    /// `false` by default.
    #[serde(default = "Auth::default_enable_dangerous_mechanism_in_clair")]
    pub enable_dangerous_mechanism_in_clair: bool,
    /// List of mechanisms supported by the server.
    #[serde(default = "Auth::default_mechanisms")]
    pub mechanisms: Vec<Mechanism>,
    /// If the AUTH exchange is canceled, the server will not consider the connection as closing,
    /// increasing the number of attempt failed, until `attempt_count_max`, producing an error.
    #[serde(default = "Auth::default_attempt_count_max")]
    pub attempt_count_max: i64,
}

impl Auth {
    pub(crate) const fn default_enable_dangerous_mechanism_in_clair() -> bool {
        false
    }

    /// Return all the supported SASL mechanisms
    #[must_use]
    pub fn default_mechanisms() -> Vec<Mechanism> {
        vec![Mechanism::Plain, Mechanism::Login, Mechanism::CramMd5]
    }

    pub(crate) const fn default_attempt_count_max() -> i64 {
        -1
    }
}

/// Available mechanisms for authentication.
#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub enum Mechanism {
    /// Common, but for interoperability
    Plain,
    /// Obsolete
    Login,
    /// Limited
    CramMd5,
    /// Common
    /// See <https://datatracker.ietf.org/doc/html/rfc4505>
    Anonymous,
    /*
    - EXTERNAL
    - SECURID
    - DIGEST-MD5
    - SCRAM-SHA-1
    - SCRAM-SHA-1-PLUS
    - SCRAM-SHA-256
    - SCRAM-SHA-256-PLUS
    - SAML20
    - OPENID20
    - GSSAPI
    - GS2-KRB5
    - XOAUTH-2
    */
}

impl std::fmt::Display for Mechanism {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Self::Plain => "PLAIN",
                Self::Login => "LOGIN",
                Self::CramMd5 => "CRAM-MD5",
                Self::Anonymous => "ANONYMOUS",
            }
        )
    }
}

impl Config for SMTPReceiverConfig {
    #[allow(clippy::field_reassign_with_default)]
    fn with_path(path: &impl AsRef<std::path::Path>) -> ConfigResult<Self>
    where
        Self: Config + serde::de::DeserializeOwned + serde::Serialize,
    {
        let mut config = Self::default();
        config.path = path.as_ref().into();

        Ok(config)
    }

    fn api_version(&self) -> &semver::VersionReq {
        &self.api_version
    }

    fn broker(&self) -> &Broker {
        &self.broker
    }

    fn queues(&self) -> &Queues {
        &self.queues
    }

    fn logs(&self) -> &logs::Logs {
        &self.logs
    }

    fn path(&self) -> &std::path::Path {
        &self.path
    }
}
