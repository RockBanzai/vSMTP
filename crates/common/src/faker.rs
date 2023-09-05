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

use crate::{delivery_route::DeliveryRoute, Mailbox, NotifyOn, Recipient};
use fake::{
    faker::{
        company::fr_fr::BsAdj,
        internet::fr_fr::{FreeEmailProvider, IPv4, IPv6, IP},
        lorem::en::Words,
        name::fr_fr::{FirstName, LastName},
    },
    Fake,
};
use vsmtp_mail_parser::Mail;
use vsmtp_protocol::{Address, ClientName, Domain, DsnReturn, OriginalRecipient, Reply};

pub struct IpFaker;

impl fake::Dummy<IpFaker> for std::net::SocketAddr {
    fn dummy_with_rng<R: rand::Rng + ?Sized>(_: &IpFaker, rng: &mut R) -> Self {
        let ip: std::net::IpAddr = IP().fake_with_rng(rng);
        let port: u16 = rng.gen_range(0..65535);
        Self::new(ip, port)
    }
}

pub struct NameFaker;

impl fake::Dummy<NameFaker> for Domain {
    fn dummy_with_rng<R: rand::Rng + ?Sized>(_: &NameFaker, rng: &mut R) -> Self {
        let domain: String = FreeEmailProvider().fake_with_rng(rng);
        domain.parse().unwrap()
    }
}

pub struct OptionNameFaker;

impl fake::Dummy<OptionNameFaker> for Option<Domain> {
    fn dummy_with_rng<R: rand::Rng + ?Sized>(_: &OptionNameFaker, rng: &mut R) -> Self {
        rng.gen_bool(0.5).then(|| {
            FreeEmailProvider()
                .fake_with_rng::<String, _>(rng)
                .parse()
                .unwrap()
        })
    }
}

pub struct ClientNameFaker;

impl fake::Dummy<ClientNameFaker> for ClientName {
    fn dummy_with_rng<R: rand::Rng + ?Sized>(_: &ClientNameFaker, rng: &mut R) -> Self {
        match rng.gen_range(0..3) {
            0 => Self::Domain(NameFaker.fake_with_rng(rng)),
            1 => Self::Ip4(IPv4().fake_with_rng(rng)),
            2 => Self::Ip6(IPv6().fake_with_rng(rng)),
            _ => unreachable!(),
        }
    }
}

pub struct MailboxFaker {
    pub domain: Option<Domain>,
}

impl fake::Dummy<MailboxFaker> for Address {
    fn dummy_with_rng<R: rand::Rng + ?Sized>(config: &MailboxFaker, rng: &mut R) -> Self {
        format!(
            "{}.{}@{}",
            FirstName().fake_with_rng::<String, _>(rng),
            LastName().fake_with_rng::<String, _>(rng),
            config
                .domain
                .clone()
                .unwrap_or_else(|| NameFaker.fake_with_rng(rng))
        )
        .parse()
        .unwrap()
    }
}

impl fake::Dummy<fake::Faker> for Recipient {
    fn dummy_with_rng<R: rand::Rng + ?Sized>(config: &fake::Faker, rng: &mut R) -> Self {
        Self {
            forward_path: Mailbox::dummy_with_rng(config, rng),
            original_forward_path: if rng.gen_bool(0.1) {
                Some(OriginalRecipient {
                    addr_type: "rfc822".to_string(),
                    mailbox: MailboxFaker { domain: None }.fake_with_rng(rng),
                })
            } else {
                None
            },
            notify_on: NotifyOnFaker.fake_with_rng(rng),
        }
    }
}

pub struct NotifyOnFaker;
impl fake::Dummy<NotifyOnFaker> for NotifyOn {
    fn dummy_with_rng<R: rand::Rng + ?Sized>(_: &NotifyOnFaker, rng: &mut R) -> Self {
        if rng.gen_bool(0.2) {
            return Self::Never;
        }
        Self::Some {
            success: rng.gen_bool(0.4),
            failure: rng.gen_bool(0.8),
            delay: rng.gen_bool(0.2),
        }
    }
}

pub struct DeliveryRouteFaker {
    pub r#type: Option<i32>,
}
impl fake::Dummy<DeliveryRouteFaker> for DeliveryRoute {
    fn dummy_with_rng<R: rand::Rng + ?Sized>(config: &DeliveryRouteFaker, rng: &mut R) -> Self {
        match config.r#type.unwrap_or_else(|| rng.gen_range(0..=4)) {
            0 => Self::Basic,
            1 => Self::Maildir,
            2 => Self::Mbox,
            3 => Self::Forward {
                service: BsAdj().fake_with_rng(rng),
            },
            4 => Self::Extern {
                name: BsAdj().fake_with_rng(rng),
            },
            _ => unimplemented!(),
        }
    }
}

pub struct RcptToFaker;

#[allow(clippy::implicit_hasher)]
impl fake::Dummy<RcptToFaker> for std::collections::HashMap<DeliveryRoute, Vec<Recipient>> {
    fn dummy_with_rng<R: rand::Rng + ?Sized>(_: &RcptToFaker, rng: &mut R) -> Self {
        let mut map = Self::new();

        if rng.gen_bool(0.5) {
            map.insert(
                DeliveryRoute::Basic,
                (fake::Faker, 1..10).fake_with_rng::<Vec<Recipient>, _>(rng),
            );
        }
        if rng.gen_bool(0.5) {
            map.insert(
                DeliveryRoute::Forward {
                    service: BsAdj().fake_with_rng(rng),
                },
                (fake::Faker, 1..10).fake_with_rng::<Vec<Recipient>, _>(rng),
            );
        }
        if rng.gen_bool(0.5) {
            map.insert(
                DeliveryRoute::Maildir,
                (fake::Faker, 1..10).fake_with_rng::<Vec<Recipient>, _>(rng),
            );
        }
        if rng.gen_bool(0.5) {
            map.insert(
                DeliveryRoute::Mbox,
                (fake::Faker, 1..10).fake_with_rng::<Vec<Recipient>, _>(rng),
            );
        }
        if rng.gen_bool(0.5) {
            let domain_count: usize = (1..15).fake_with_rng(rng);
            for _ in 0..domain_count {
                let provider: String = FreeEmailProvider().fake_with_rng(rng);

                let rcpt_count: usize = (1..5).fake_with_rng(rng);
                let recipient = (0..rcpt_count)
                    .map(|_| Recipient {
                        forward_path: Mailbox(
                            MailboxFaker {
                                domain: Some(provider.parse().unwrap()),
                            }
                            .fake_with_rng(rng),
                        ),
                        notify_on: NotifyOnFaker.fake_with_rng(rng),
                        original_forward_path: None,
                    })
                    .collect::<Vec<_>>();

                map.insert(
                    DeliveryRoute::Extern {
                        name: format!("send.{provider}"),
                    },
                    recipient,
                );
            }
        }

        map
    }
}

pub struct MailFaker;

impl fake::Dummy<MailFaker> for Mail {
    fn dummy_with_rng<R: rand::Rng + ?Sized>(_: &MailFaker, _rng: &mut R) -> Self {
        let raw = r#"From: <foo@mydomain.tld>
To: <foo@mydomain.tld>
Date: Wed, 31 May 2023 14:29:09 +0200 (CEST)
Message-Id: <14e17.0003.0000@mlala-Nitro-AN515-54>

La de da de da 1.
La de da de da 2.
La de da de da 3.
La de da de da 4.
"#;

        let raw = raw.replace('\n', "\r\n");
        raw.as_str().try_into().unwrap()
    }
}

pub struct ProtocolErrorFaker;
impl fake::Dummy<ProtocolErrorFaker> for Option<vsmtp_protocol::Error> {
    fn dummy_with_rng<R: rand::Rng + ?Sized>(_config: &ProtocolErrorFaker, rng: &mut R) -> Self {
        if rng.gen_bool(0.25) {
            return None;
        }

        let random_kind = rng
            .gen_range(0..=vsmtp_protocol::ErrorKind::Other as i32)
            .try_into()
            .unwrap();
        let kind = <vsmtp_protocol::ErrorKind as strum::IntoEnumIterator>::iter()
            .nth(random_kind)
            .unwrap()
            .to_std();

        Some(std::io::Error::from(kind).into())
    }
}

pub struct ReplyFaker;
impl fake::Dummy<ReplyFaker> for Reply {
    fn dummy_with_rng<R: rand::Rng + ?Sized>(_config: &ReplyFaker, rng: &mut R) -> Self {
        let severity = rng.gen_range(2..=5);
        let category = rng.gen_range(0..=5);
        let details = rng.gen_range(0..=9);

        format!(
            "{}{}{} {}",
            severity,
            category,
            details,
            Words(3..7).fake_with_rng::<Vec<String>, R>(rng).join(" ")
        )
        .parse()
        .unwrap()
    }
}

pub struct DsnReturnFaker;
impl fake::Dummy<DsnReturnFaker> for DsnReturn {
    fn dummy_with_rng<R: rand::Rng + ?Sized>(_: &DsnReturnFaker, rng: &mut R) -> Self {
        if rng.gen_bool(0.5) {
            Self::Headers
        } else {
            Self::Full
        }
    }
}
