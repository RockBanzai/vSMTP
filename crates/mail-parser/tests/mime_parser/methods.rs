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

use crate::{message::message_body::MessageBody, MailMimeParser};

fn generate_test_bodies() -> (MessageBody, MessageBody) {
    let headers = [
        "From: john <john@example.com>\r\n",
        "To: green@example.com\r\n",
        "Date: tue, 30 nov 2021 20:54:27 +0100\r\n",
        "Content-Language: en-US\r\n",
        "Subject: test message\r\n",
        "Content-Type: text/html; charset=UTF-8\r\n",
        "Content-Transfer-Encoding: 7bit\r\n",
    ];
    let body = r#"<html>
  <head>
    <meta http-equiv="Content-Type" content="text/html; charset=UTF-8">
  </head>
  <body>
    only plain text here<br>
  </body>
</html>
"#;

    let raw = MessageBody::new(
        headers.iter().map(ToString::to_string).collect(),
        body.to_string(),
    );
    let mut parsed = raw.clone();
    parsed.parse::<MailMimeParser>().unwrap();

    (raw, parsed)
}

#[test]
fn test_get_header() {
    use crate::tests::mime_parser::methods::generate_test_bodies;

    let (raw, parsed) = generate_test_bodies();

    assert_eq!(raw.get_header("To"), Some("green@example.com".to_string()));
    assert_eq!(
        parsed.get_header("to"),
        Some("green@example.com".to_string())
    );
}

#[test]
fn test_set_and_append_header() {
    use crate::tests::mime_parser::methods::generate_test_bodies;

    let (mut raw, mut parsed) = generate_test_bodies();

    let new_header = "X-New-Header";
    let new_header_message = "this is a new header!";
    let subject_message = "this is a subject";

    raw.set_header("Subject", subject_message);
    raw.set_header(new_header, new_header_message);
    assert_eq!(raw.get_header("Subject"), Some(subject_message.to_string()));
    assert_eq!(
        raw.get_header(new_header),
        Some(new_header_message.to_string())
    );

    parsed.set_header("subject", subject_message);
    parsed.set_header(new_header, new_header_message);
    assert_eq!(
        parsed.get_header("subject"),
        Some(subject_message.to_string())
    );
    assert_eq!(
        parsed.get_header(new_header),
        Some(new_header_message.to_string())
    );
}
