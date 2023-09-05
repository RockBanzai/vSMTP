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

use crate::message::mail::{BodyType, Mail, MailHeaders};
use crate::{MailMimeParser, MailParser};

#[test]
fn white_space_and_comments() {
    let parsed = MailMimeParser::default()
        .parse_sync(
            include_str!("../../mail/rfc5322/A.5.eml")
                .lines()
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
                    (
                        "from",
                        "Pete(A nice \\) chap) <pete(his account)@silly.test(his host)>"
                    ),
                    (
                        "to",
                        concat!(
                            "A Group(Some people)\r\n",
                            "     :Chris Jones <c@(Chris's host.)public.example>,\r\n",
                            "         joe@example.org,\r\n  John <jdoe@one.test> (my dear friend); (the end of the group)",
                        )
                    ),
                    ("cc", "(Empty list)(start)Hidden recipients  :(nobody(that I know))  ;"),
                    (
                        "date",
                        concat!(
                            "Thu,\r\n",
                            "      13\r\n",
                            "        Feb\r\n",
                            "          1969\r\n",
                            "      23:32\r\n",
                            "               -0330 (Newfoundland Time)",
                        ),
                    ),
                    ("message-id", "             <testabcd.1234@silly.test>"),
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
    pretty_assertions::assert_eq!(
        parsed.to_string(),
        include_str!("../../mail/rfc5322/A.5.eml")
            .replace('\n', "\r\n")
            // TODO: verify the rfc
            .replace("To:A Group(Some people)", "To: A Group(Some people)")
            .replace(
                "Cc:(Empty list)(start)Hidden recipients  :(nobody(that I know))  ;",
                "Cc: (Empty list)(start)Hidden recipients  :(nobody(that I know))  ;"
            )
    );
}
