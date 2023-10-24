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

use crate::extensions::{self, Extension};
use crate::faker::ReplyFaker;
use crate::transfer_error::Delivery;
use vsmtp_protocol::Reply;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, fake::Dummy)]
pub struct Ehlo {
    #[dummy(faker = "ReplyFaker")]
    reply: Reply,
    server_name: String,
    extensions: Vec<(Extension, String)>,
}

impl Ehlo {
    #[must_use]
    pub fn contains(&self, extension: &Extension) -> bool {
        self.extensions.iter().any(|(e, _)| e == extension)
    }
}

impl TryFrom<Reply> for Ehlo {
    type Error = Delivery;

    fn try_from(reply: Reply) -> Result<Self, Self::Error> {
        let code = reply.code();
        if code.value() != 250 {
            return Err(Delivery::ReplyParsing {
                with_source: Some(format!("expect 250 on EHLO, got {}", code.value())),
            });
        }

        let mut lines = reply.lines();
        let server_name = lines
            .next()
            .ok_or(Delivery::ReplyParsing {
                with_source: Some("servername missing from ehlo response".to_owned()),
            })?
            .clone();

        let extensions = lines
            .map(|l| extensions::from_str(l))
            .map(|(verb, args)| (verb, args.to_owned()))
            .collect::<Vec<_>>();

        Ok(Self {
            reply,
            server_name,
            extensions,
        })
    }
}
