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

use vsmtp_mail_parser::{
    mail::{body::Body, headers::Header},
    Mail,
};

use crate::dkim::{canonicalization::CanonicalizationAlgorithm, HashAlgorithm};

macro_rules! canonicalization_empty_body {
    ($name:ident, $canon:expr, $algo:expr, $expected:expr) => {
        #[test]
        fn $name() {
            assert_eq!(
                base64::Engine::encode(
                    &base64::engine::general_purpose::STANDARD,
                    $algo.hash($canon.canonicalize_body(""))
                ),
                $expected
            );
        }
    };
}

#[cfg(feature = "historic")]
canonicalization_empty_body!(
    simple_empty_body_rsa_sha1,
    CanonicalizationAlgorithm::Simple,
    HashAlgorithm::Sha1,
    "uoq1oCgLlTqpdDX/iUbLy7J1Wic="
);

canonicalization_empty_body!(
    simple_empty_body_rsa_sha256,
    CanonicalizationAlgorithm::Simple,
    HashAlgorithm::Sha256,
    "frcCV1k9oG9oKj3dpUqdJg1PxRT2RSN/XKdLCPjaYaY="
);

#[cfg(feature = "historic")]
canonicalization_empty_body!(
    relaxed_empty_body_rsa_sha1,
    CanonicalizationAlgorithm::Relaxed,
    HashAlgorithm::Sha1,
    "2jmj7l5rSw0yVb/vlWAYkK/YBwk="
);

canonicalization_empty_body!(
    relaxed_empty_body_rsa_sha256,
    CanonicalizationAlgorithm::Relaxed,
    HashAlgorithm::Sha256,
    "47DEQpj8HBSa+/TImW+5JCeuQeRkm5NMpJWZG3hSuFU="
);

#[test]
fn canonicalize_ex1() {
    let msg = Mail {
        headers: vec![
            Header::new_unchecked("A", " X\r\n"),
            Header::new_unchecked("B ", " Y\t\r\n\tZ \r\n"),
        ]
        .into(),
        body: Body::Raw(
            [" C \r\n", "D \t E\r\n", "\r\n", "\r\n"]
                .into_iter()
                .map(str::to_string)
                .collect(),
        ),
    };

    assert_eq!(
        msg.headers
            .iter()
            .map(|header| CanonicalizationAlgorithm::Relaxed
                .canonicalize_header(&header.to_string()))
            .fold(String::new(), |mut acc, s| {
                acc.push_str(&s);
                acc.push_str("\r\n");
                acc
            }),
        concat!("a:X\r\n", "b:Y Z\r\n")
    );

    assert_eq!(
        CanonicalizationAlgorithm::Relaxed.canonicalize_headers(
            &msg.headers
                .iter()
                .map(Header::to_string)
                .collect::<Vec<_>>()
        ),
        concat!("a:X\r\n", "b:Y Z\r\n")
    );

    assert_eq!(
        CanonicalizationAlgorithm::Relaxed.canonicalize_body(&msg.body.to_string()),
        concat!(" C\r\n", "D E\r\n")
    );
}

#[test]
fn canonicalize_ex2() {
    let msg = Mail {
        headers: vec![
            Header::new_unchecked("A", " X\r\n"),
            Header::new_unchecked("B ", " Y\t\r\n\tZ  \r\n"),
        ]
        .into(),
        body: Body::Raw(
            [" C \r\n", "D \t E\r\n", "\r\n", "\r\n"]
                .into_iter()
                .map(str::to_string)
                .collect(),
        ),
    };

    assert_eq!(
        msg.headers
            .iter()
            .map(|header| CanonicalizationAlgorithm::Simple
                .canonicalize_header(&header.to_string()))
            .fold(String::new(), |mut acc, s| {
                acc.push_str(&s);
                acc
            }),
        concat!("A: X\r\n", "B : Y\t\r\n", "\tZ  \r\n")
    );

    assert_eq!(
        CanonicalizationAlgorithm::Simple.canonicalize_headers(
            &msg.headers
                .iter()
                .map(Header::to_string)
                .collect::<Vec<_>>()
        ),
        concat!("A: X\r\n", "B : Y\t\r\n", "\tZ  \r\n")
    );

    assert_eq!(
        CanonicalizationAlgorithm::Simple.canonicalize_body(&msg.body.to_string()),
        concat!(" C \r\n", "D \t E\r\n").to_string()
    );
}

#[test]
fn canonicalize_trailing_newline() {
    let msg = Mail {
        headers: vec![
            Header::new_unchecked("A", " X\r\n"),
            Header::new_unchecked("B ", " Y\t\r\n\tZ \r\n"),
        ]
        .into(),
        body: Body::Raw(
            [" C \r\n", "D \t E\r\n", "\r\n", "\r\nok"]
                .into_iter()
                .map(str::to_string)
                .collect(),
        ),
    };

    assert_eq!(
        CanonicalizationAlgorithm::Relaxed.canonicalize_body(&msg.body.to_string()),
        concat!(" C\r\n", "D E\r\n\r\n\r\nok\r\n")
    );
}
