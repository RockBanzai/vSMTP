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

use vsmtp_rule_engine::Stage;

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum ReceiverStage {
    Connect,
    Helo,
    Authenticate,
    MailFrom,
    RcptTo,
    Data,
    PreQueue,
}

impl Stage for ReceiverStage {
    fn hook(&self) -> &'static str {
        match self {
            Self::Connect => "on_connect",
            Self::Helo => "on_helo",
            Self::Authenticate => "on_auth",
            Self::MailFrom => "on_mail_from",
            Self::RcptTo => "on_rcpt_to",
            Self::Data => "on_data",
            Self::PreQueue => "on_pre_queue",
        }
    }

    fn stages() -> &'static [&'static str] {
        &[
            "connect",
            "helo",
            "auth",
            "mail_from",
            "rcpt_to",
            "data",
            "pre_queue",
        ]
    }
}

impl std::str::FromStr for ReceiverStage {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "connect" => Ok(Self::Connect),
            "helo" => Ok(Self::Helo),
            "auth" => Ok(Self::Authenticate),
            "mail_from" => Ok(Self::MailFrom),
            "rcpt_to" => Ok(Self::RcptTo),
            "data" => Ok(Self::Data),
            "pre_queue" => Ok(Self::PreQueue),
            _ => Err(()),
        }
    }
}

impl std::fmt::Display for ReceiverStage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Self::Connect => "connect",
                Self::Helo => "helo",
                Self::Authenticate => "auth",
                Self::MailFrom => "mail_from",
                Self::RcptTo => "rcpt_to",
                Self::Data => "data",
                Self::PreQueue => "pre_queue",
            }
        )
    }
}
