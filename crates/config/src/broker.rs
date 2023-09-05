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

// TODO: This configuration could be parsed from an url.
// See https://github.com/viridIT/rfc/blob/main/text/0008-smtp-receiver/0008-smtp-receiver.md#complete-example
#[derive(Default, Debug, serde::Deserialize, serde::Serialize, PartialEq, Eq)]
pub struct Broker {
    // FIXME: Which default should be used ?
    /// AMQP endpoint.
    pub uri: String,
    #[serde(default)]
    pub scheme: Scheme,
    #[serde(
        default,
        skip_serializing,
        deserialize_with = "crate::deserialize_certificate"
    )]
    pub certificate_chain: Option<String>,
    // TODO: query argument
    // TODO: tls root certificate / validate peer / client certificate
    // TODO: vhost: "/path/to/vhost" | null,
}

#[derive(Default, Debug, serde::Deserialize, serde::Serialize, PartialEq, Eq)]
pub enum Scheme {
    Amqp,
    #[default]
    Amqps,
}
