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

/// Stage of the step-by-step SMTP transaction
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Stage {
    /// The client has just connected to the server
    Connect,
    /// The client has sent the HELO/EHLO command
    Helo,
    /// The client has sent the MAIL FROM command
    MailFrom,
    /// The client has sent the RCPT TO command
    RcptTo,
    /// The client has sent the complete message
    Finished,
}
