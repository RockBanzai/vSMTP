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

use vsmtp_mail_parser::parsing::bytes::Parser;

/// Build a complete path from the current cargo manifest files using a relative path.
#[macro_export]
macro_rules! from_manifest_path {
    ($path:expr) => {
        std::path::PathBuf::from_iter([env!("CARGO_MANIFEST_DIR"), $path])
    };
}

#[test]
fn exclude_attachments() {
    let raw = include_str!("mail/mime1.eml").replace('\n', "\r\n");
    let bytes = raw
        .lines()
        .map(|l| {
            let mut l = l.as_bytes().to_vec();
            l.extend(b"\r\n");
            l
        })
        .collect();

    let mut mail = vsmtp_mail_parser::parsing::bytes::Parser::default()
        .parse_headers(bytes)
        .unwrap();

    // Parse the body.
    let _ = mail.parse_body().unwrap();

    let mail = mail.to_string_without_attachments();

    pretty_assertions::assert_eq!(
        mail,
        // Remove the desired attachment.
        raw.replace(
            "SUQsRmlyc3ROYW1lLExhc3ROYW1lLENvdW50cnkKMzQ4LEpvaG4sU3RpbGVzLENhbmFkYQo5MjM4\r\nOSxKaWUsTGl1LENoaW5hCjczNCxTaGlybGV5LFJvZHJpZ3VleixVbml0ZWQgU3RhdGVzCjI4OTMs\r\nQW5heWEsSXllbmdhcixJbmRpYQ==\r\n\r\n", ""
        )
    )
}

#[test]
fn should_produce_same_mail() {
    let raw = include_str!("mail/mime1.eml").replace('\n', "\r\n");
    let bytes = raw
        .lines()
        .map(|l| {
            let mut l = l.as_bytes().to_vec();
            l.extend(b"\r\n");
            l
        })
        .collect();

    let mail = vsmtp_mail_parser::parsing::bytes::Parser::default()
        .parse_headers(bytes)
        .unwrap();

    let mail = mail.to_string();
    pretty_assertions::assert_eq!(raw, mail);
}

fn visit_dirs(
    dir: &std::path::Path,
    cb: &dyn Fn(&std::fs::DirEntry) -> std::io::Result<()>,
) -> std::io::Result<()> {
    if dir.is_dir() {
        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                visit_dirs(&path, cb)?;
            } else {
                cb(&entry)?;
            }
        }
    }
    Ok(())
}

#[test]
fn test_parse_whole_folder() {
    visit_dirs(
        &from_manifest_path!("tests/mail"),
        &|entry| -> std::io::Result<()> {
            println!("reading {entry:?}");
            let raw = std::fs::read_to_string(entry.path())
                .expect("emails in test directory should be readable")
                .replace('\n', "\r\n");

            // FIXME: using `lines` isn't great for the parser, because it can omit information on
            //        the last carriage return:
            //        "The final line ending is optional. A string that ends with a final line ending
            //         will return the same lines as an otherwise identical string without a final line ending."
            let bytes: Vec<Vec<u8>> = raw
                .lines()
                .map(|l| {
                    let mut l = l.as_bytes().to_vec();
                    l.extend(b"\r\n");
                    l
                })
                .collect();

            // Only parse the headers, then the body.
            Parser::default()
                .parse_headers(bytes.clone())
                .map(|mut mail| {
                    pretty_assertions::assert_eq!(mail.to_string(), raw);
                    mail.parse_body().expect("failed to parse body");
                    pretty_assertions::assert_eq!(mail.to_string(), raw);
                })
                .expect("failed to parse email headers");

            // Fully parse the email.
            Parser::default()
                .parse(bytes)
                .map(|mail| pretty_assertions::assert_eq!(mail.to_string(), raw))
                .expect("failed to parse email");

            Ok(())
        },
    )
    .expect("folder contain valid mail");
}
