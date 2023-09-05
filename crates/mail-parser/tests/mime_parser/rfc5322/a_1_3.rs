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

use crate::MailMimeParser;
use crate::{
    message::mail::{BodyType, Mail, MailHeaders},
    MailParser,
};

const MAIL: &str = include_str!("../../mail/rfc5322/A.1.3.eml");

#[test]
fn group_addresses() {
    let parsed = MailMimeParser::default()
        .parse_sync(
            MAIL.lines()
                .map(|l| l.as_bytes().to_vec())
                .collect::<Vec<_>>(),
        )
        .unwrap()
        .unwrap_right();
    pretty_assertions::assert_eq!(
        parsed,
        Mail {
            headers: MailHeaders(
                [
                    ("from", "Pete <pete@silly.example>"),
                    (
                        "to",
                        "A Group:Ed Jones <c@a.test>,joe@where.test,John <jdoe@one.test>;"
                    ),
                    ("cc", "Undisclosed recipients:;"),
                    ("date", "Thu, 13 Feb 1969 23:32:54 -0330"),
                    ("message-id", "<testabcd.1234@silly.example>"),
                ]
                .into_iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect::<Vec<_>>()
            ),
            body: BodyType::Regular(
                vec!["Testing."]
                    .into_iter()
                    .map(str::to_string)
                    .collect::<_>()
            )
        }
    );
    pretty_assertions::assert_eq!(parsed.to_string(), MAIL.replace('\n', "\r\n"));
}
