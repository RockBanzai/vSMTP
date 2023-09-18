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

/// Log emitter triggered when rustls does some stuff.
pub struct TlsLogger;

impl vsmtp_protocol::rustls::KeyLog for TlsLogger {
    fn log(&self, label: &str, client_random: &[u8], secret: &[u8]) {
        tracing::trace!(label, ?client_random, ?secret);
    }
}
