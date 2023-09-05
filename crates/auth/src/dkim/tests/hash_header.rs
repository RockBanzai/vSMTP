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

use crate::dkim::{self, PublicKey, Signature};
use base64::Engine;
use vsmtp_mail_parser::Mail;

#[ignore = "used for debugging with FILE env var as input file"]
#[test_log::test]
fn verify_file() {
    let filepath = option_env!("FILE").unwrap();
    let file_content = std::fs::read_to_string(filepath).unwrap();
    let mail = Mail::try_from(file_content.as_str()).unwrap();

    let signature = <Signature as std::str::FromStr>::from_str(
        &mail.get_header("DKIM-Signature").unwrap().to_string(),
    )
    .unwrap();

    let txt_record = trust_dns_resolver::Resolver::default()
        .unwrap()
        .txt_lookup(dbg!(signature.get_dns_query()))
        .unwrap();

    let keys = txt_record
        .into_iter()
        .map(|i| <PublicKey as std::str::FromStr>::from_str(&i.to_string()));

    let keys = keys
        .collect::<Result<Vec<_>, <PublicKey as std::str::FromStr>::Err>>()
        .unwrap();

    dkim::verify(&signature, &mail, keys.first().unwrap()).unwrap();
}

#[test]
fn mail_5() {
    let mail = Mail::try_from(include_str!("mail_5.eml")).unwrap();

    let signature = <Signature as std::str::FromStr>::from_str(
        &mail.get_header("DKIM-Signature").unwrap().to_string(),
    )
    .unwrap();

    let header = signature.get_header_for_hash(&mail);

    pretty_assertions::assert_eq!(
        header,
        concat!(
            "Date: Wed, 3 Aug 2022 17:48:03 +0200\r\n",
            "To: jdoe@negabit.com\r\nFrom: john <john.doe@example.com>\r\n",
            "Subject: after dns update\r\nDKIM-Signature: v=1; a=rsa-sha256; c=simple/simple; d=example.com; s=mail;\r\n",
            "\tt=1659541683; bh=Touenr7dUe0Mxv9r3OfnQ+GHpFRIdDa3Wa3TWnDOQKs=;\r\n",
            "\th=Date:To:From:Subject:From;\r\n",
            "\tb=\r\n"
        )
    );

    println!(
        "{}",
        base64::engine::general_purpose::STANDARD.encode(signature.get_header_hash(&mail))
    );
}
