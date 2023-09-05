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
    collection,
    message::{
        mail::{BodyType, Mail, MailHeaders},
        mime_type::{Mime, MimeBodyType, MimeHeader},
    },
    MailParser,
};

const MAIL: &str = include_str!("../mail/allen-p__discussion_threads__1.eml");

#[test]
fn mime_parser() {
    pretty_assertions::assert_eq!(
        MailMimeParser::default()
            .parse_sync(
                MAIL.lines()
                    .map(|l| l.as_bytes().to_vec())
                    .collect::<Vec<_>>()
            )
            .unwrap()
            .unwrap_right(),
        Mail {
            headers: MailHeaders(
                [
                    (
                        "message-id",
                        "<20379972.1075855673249.JavaMail.evans@thyme>"
                    ),
                    ("date", "Fri, 10 Dec 1999 07:00:00 -0800 (PST)"),
                    ("from", "phillip.allen@enron.com"),
                    ("to", "naomi.johnston@enron.com"),
                    ("subject", ""),
                    ("mime-version", "1.0"),
                    ("x-from", "Phillip K Allen"),
                    ("x-to", "Naomi Johnston"),
                    ("x-cc", ""),
                    ("x-bcc", ""),
                    (
                        "x-folder",
                        "\\Phillip_Allen_Dec2000\\Notes Folders\\Discussion threads"
                    ),
                    ("x-origin", "Allen-P"),
                    ("x-filename", "pallen.nsf"),
                ]
                .into_iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect::<Vec<_>>()
            ),
            body: BodyType::Mime(Box::new(Mime {
                headers: vec![
                    MimeHeader {
                        name: "Content-Type".to_string(),
                        value: "text/plain".to_string(),
                        args: collection! {
                            "charset".to_string() => "us-ascii".to_string()
                        }
                    },
                    MimeHeader {
                        name: "content-transfer-encoding".to_string(),
                        value: "7bit".to_string(),
                        args: collection! {}
                    },
                ],
                content: MimeBodyType::Regular(
                    vec![
                    "Naomi,",
                    "",
                    "The two analysts that I have had contact with are Matt Lenhart  and Vishal ",
                    "Apte.",
                    "Matt will be represented by Jeff Shankman.",
                    "Vishal joined our group in October.  He was in the Power Trading Group for ",
                    "the first 9 months.",
                    "I spoke to Jim Fallon and we agreed that he should be in the excellent ",
                    "category.  I just don't want Vishal ",
                    "to go unrepresented since he changed groups mid year.",
                    "",
                    "Call me with questions.(x37041)",
                    "",
                    "Phillip Allen",
                    "West Gas Trading",
                ]
                    .into_iter()
                    .map(str::to_string)
                    .collect::<Vec<_>>()
                )
            }))
        }
    );
}
