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

/// The representation of a header for a DKIM algorithm.
pub trait Header {
    /// Get the name of the header.
    fn field_name(&self) -> String;

    /// Get the *complete* value of a header, name and value and comma, whitespace and comments, as received.
    fn get(&self) -> String;
}

/// The representation of an email for a DKIM algorithm.
pub trait Mail {
    /// The type of the header, respecting the constraints of the trait [`Header`].
    type H: Header;

    /// Get the body of the email.
    fn get_body(&self) -> String;

    /// Get the headers of the email.
    fn get_headers(&self) -> Vec<Self::H>;
}
