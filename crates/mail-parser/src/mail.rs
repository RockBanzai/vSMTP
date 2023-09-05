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

//! Definition of an email data structure.

use crate::{mail::body::ParsedBody, mime, ParserError};

use self::{
    body::Body,
    headers::{Header, Headers},
};

/// Body definition of an email.
pub mod body;
/// Headers definition of an email.
pub mod headers;

pub const FROM_HEADER: &str = "From";
pub const TO_HEADER: &str = "To";
pub const DATE_HEADER: &str = "Date";

/// Internet Message Format representation, completely deserialize and parsed.
#[derive(Clone, Default, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub struct Mail {
    /// Message headers.
    pub headers: Headers,
    /// Message body content.
    pub body: Body,
}

impl TryFrom<Vec<Vec<u8>>> for Mail {
    type Error = ParserError;

    fn try_from(value: Vec<Vec<u8>>) -> Result<Self, Self::Error> {
        crate::parsing::bytes::Parser::default().parse_headers(value)
    }
}

impl TryFrom<&str> for Mail {
    type Error = ParserError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        let mut bytes = vec![];
        let splitted = value.split("\r\n").collect::<Vec<_>>();
        for (idx, mut line) in splitted.iter().map(|l| l.as_bytes().to_vec()).enumerate() {
            bytes.push({
                if idx != splitted.len() - 1 {
                    line.extend_from_slice(b"\r\n");
                }
                line
            });
        }

        crate::parsing::bytes::Parser::default().parse_headers(bytes)
    }
}

impl Mail {
    // TODO: this is pretty much useless, see <https://github.com/viridIT/email/blob/main/src/basic.rs#L55> instead.
    /// Parses an email from a stream of bytes.
    ///
    /// # Args`
    ///
    /// * `stream` - The stream to parse the email from.
    ///
    /// # Errors
    ///
    /// * The input is not compliant
    pub async fn parse_stream<'a>(
        mut stream: impl tokio_stream::Stream<Item = Result<Vec<u8>, ParserError>> + Unpin + Send + 'a,
    ) -> Result<Self, ParserError> {
        let mut buffer = Vec::new();

        while let Some(i) = tokio_stream::StreamExt::try_next(&mut stream).await? {
            buffer.push(i);
        }

        crate::parsing::bytes::Parser::default().parse_headers(buffer)
    }

    /// Get a mutable reference on the body.
    /// If the body has not been parsed yet, parse it.
    /// If it as already been parsed, return a reference to it.
    ///
    /// # Errors
    ///
    /// The body is empty.
    /// Failed to parse the body.
    pub fn body_mut(&mut self) -> Result<&mut ParsedBody, ParserError> {
        crate::parsing::bytes::Parser::default().parse_body_from_mail(self)
    }

    /// Change the "From" header value.
    pub fn rewrite_mail_from(&mut self, value: &str) {
        if let Some(old) = self
            .headers
            .0
            .iter_mut()
            .find(|Header { name, .. }| name.eq_ignore_ascii_case(FROM_HEADER))
        {
            old.body = value.to_string();
        } else {
            self.headers.push(Header::new(FROM_HEADER, value));
        }
    }

    /// Replace a recipient with another.
    pub fn rewrite_rcpt(&mut self, old: &str, new: &str) {
        if let Some(rcpt) = self
            .headers
            .0
            .iter_mut()
            .find(|Header { name, .. }| name.eq_ignore_ascii_case(TO_HEADER))
        {
            rcpt.body = rcpt.body.replace(old, new);
        } else {
            self.headers.push(Header::new(TO_HEADER, new));
        }
    }

    /// Add a recipient to the "To" header.
    pub fn add_rcpt(&mut self, new: &str) {
        if let Some(Header { body, .. }) = self
            .headers
            .0
            .iter_mut()
            .find(|header| header.name.eq_ignore_ascii_case(TO_HEADER))
        {
            // FIXME: newline not handled.
            *body = format!("{body}, {new}");
        } else {
            self.headers.push(Header::new(TO_HEADER, new));
        }
    }

    /// Remove a recipient from the "To" header.
    pub fn remove_rcpt(&mut self, old: &str) {
        self.headers
            .0
            .iter_mut()
            .find(|header| header.name.eq_ignore_ascii_case(TO_HEADER))
            .and_then::<(), _>(|Header { body, .. }| {
                if body.find(old) == Some(0) {
                    *body = body.replace(format!("{old}, ").as_str(), "");
                } else {
                    *body = body.replace(format!(", {old}").as_str(), "");
                }
                None
            });
    }

    /// Set a header with a new value or push it to the header stack.
    pub fn set_header(&mut self, name: &str, value: &str) {
        if let Some(Header { body, .. }) = self
            .headers
            .0
            .iter_mut()
            .find(|header| header.name.eq_ignore_ascii_case(name))
        {
            *body = value.to_string();
        } else {
            self.headers.push(Header::new(name, value));
        }
    }

    // TODO: should this rename all headers with the same name ?
    /// Rename a header.
    pub fn rename_header(&mut self, old: &str, new: &str) {
        if let Some(Header { name, .. }) = self
            .headers
            .0
            .iter_mut()
            .find(|header| header.name.eq_ignore_ascii_case(old))
        {
            *name = new.to_string();
        }
    }

    /// Get the first value of a header which the name matches the argument.
    #[must_use]
    pub fn get_header(&self, name: &str) -> Option<&Header> {
        self.headers
            .0
            .iter()
            .find(|header| header.name.eq_ignore_ascii_case(name))
    }

    /// Get the value of all headers which the name matches the argument.
    pub fn get_headers<'a>(&'a self, name: &'a str) -> impl Iterator<Item = &'a Header> {
        self.headers
            .iter()
            .filter(|header| header.name.eq_ignore_ascii_case(name))
    }

    /// Get the value of a header. The lookup starts from the end of the header list.
    #[must_use]
    pub fn get_header_rev(&self, name: &str) -> Option<&str> {
        self.headers
            .0
            .iter()
            .rev()
            .find(|header| header.name.eq_ignore_ascii_case(name))
            .map(|Header { body, .. }| body.as_str())
    }

    /// Get the "name: body" form of a set of headers matching `named`.
    pub fn get_headers_raw_without_crlf<'a, 'b: 'a>(
        &'b self,
        named: &'a str,
    ) -> impl Iterator<Item = String> + 'a {
        // FIXME: weird, too much allocation
        self.headers
            .iter()
            .filter(|Header { name, .. }| name.eq_ignore_ascii_case(named))
            .map(Header::to_string_without_crlf)
    }

    /// Get the "From" header specified in rfc5322.
    #[must_use]
    pub fn get_rfc5322_from(&self) -> Option<&Header> {
        // FIXME: handle charset encoding
        self.get_headers(FROM_HEADER).next()
    }

    /// Count the number of a header occurrence.
    #[must_use]
    pub fn count_header(&self, name: &str) -> usize {
        self.headers
            .0
            .iter()
            .filter(|header| header.name.eq_ignore_ascii_case(name))
            .count()
    }

    // NOTE: would a double ended queue / linked list interesting in this case ?
    /// Prepend new headers to the email.
    pub fn prepend_headers(&mut self, headers: impl IntoIterator<Item = Header>) {
        self.headers.splice(..0, headers);
    }

    /// Push new headers to header list.
    pub fn append_headers(&mut self, headers: impl IntoIterator<Item = Header>) {
        self.headers.extend(headers);
    }

    /// Remove a header from the list.
    pub fn remove_header(&mut self, name: &str) -> bool {
        if let Some(index) = self
            .headers
            .0
            .iter()
            .position(|header| header.name.eq_ignore_ascii_case(name))
        {
            self.headers.remove(index);
            true
        } else {
            false
        }
    }

    /// Get all attachments from the mail.
    /// This function parses the body if as not been done yet.
    ///
    /// Attachments are:
    /// - content type marked as binary.
    /// - any embedded email.
    /// - any type that as the "Content-Disposition: attachment" property.
    ///
    /// # Errors
    ///
    /// Failed to parse the body.
    pub fn attachments(&mut self) -> Result<Vec<&mime::Mime>, ParserError> {
        self.body_mut().map(|body| body.attachments())
    }

    /// Parse the body and return a reference to it.
    /// If the body is already parsed, return it directly.
    pub fn parse_body(&mut self) -> Result<&mut ParsedBody, ParserError> {
        self.body_mut()
    }
}

impl std::fmt::Display for Mail {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.headers)?;

        if !matches!(self.body, Body::Parsed(ParsedBody::Mime(_))) {
            f.write_str("\r\n")?;
        }

        write!(f, "{}", self.body)
    }
}

impl Mail {
    /// Return the body as a string without it's attachments.
    pub fn to_string_without_attachments(&self) -> String {
        let mut string: String = self.headers.iter().map(Header::to_string).collect();

        if !matches!(self.body, Body::Parsed(ParsedBody::Mime(_))) {
            string.push_str("\r\n");
        }

        string.push_str(&self.body.to_string_without_attachments());

        string
    }
}
