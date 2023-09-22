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

use crate::mime::parts::MultipartDisplayable;
use crate::ParserError;
use crate::ParserResult;

/// Mime headers definition.
pub mod headers;
pub use headers::Header;

/// Mime parts definition.
pub mod parts;
pub use parts::Multipart;
pub use parts::Part;

pub const CONTENT_TYPE_HEADER: &str = "Content-Type";
pub const CONTENT_DISPOSITION_HEADER: &str = "Content-Disposition";
pub const MIME_VERSION_HEADER: &str = "MIME-Version";

/// <https://www.rfc-editor.org/rfc/rfc2045>
#[derive(Clone, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub struct Mime {
    /// Mime part headers.
    pub headers: Vec<Header>,
    /// Content of the mime part.
    pub part: Part,
}

impl Mime {
    /// Get the given part as a string.
    #[must_use]
    pub fn raw_part(&self) -> String {
        match &self.part {
            Part::Text(content) | Part::Html(content) | Part::Binary(content) => content.join(""),
            Part::Multipart(multipart) => {
                let boundary = self.boundary().unwrap();

                MultipartDisplayable {
                    inner: multipart,
                    boundary,
                    top_level: false,
                }
                .to_string()
            }
            Part::Embedded(mail) => mail.to_string(),
        }
    }

    /// Check of the current mime part is an attachment.
    #[must_use]
    pub fn is_attachment(&self) -> bool {
        match &self.part {
            Part::Text(_) | Part::Html(_) => self
                .headers
                .iter()
                .find(|h| h.name.eq_ignore_ascii_case(CONTENT_DISPOSITION_HEADER))
                .map_or(false, |h| h.body().eq_ignore_ascii_case("attachment")),
            Part::Embedded(_) | Part::Binary(_) => true,
            Part::Multipart(_) => false,
        }
    }

    /// Extract a boundary from the Content-Type header field
    /// if the current mime part is multipart.
    #[must_use]
    pub fn boundary(&self) -> Option<&str> {
        self.headers.iter().find_map(|header| {
            if header.name.eq_ignore_ascii_case(CONTENT_TYPE_HEADER) {
                header.arg("boundary").map(headers::Arg::value)
            } else {
                None
            }
        })
    }
}

/// Cut the mime type of the current section and return the type and subtype.
/// if no Content-Type header is found, will check the parent for a default
/// Content-Type header value.
///
/// see <https://datatracker.ietf.org/doc/html/rfc2045#page-14> for default Content-Type.
/// see <https://datatracker.ietf.org/doc/html/rfc2046#page-26> for digest multipart parent.
pub fn get_mime_type<'a>(
    headers: &'a [Header],
    parent: Option<&'a [Header]>,
) -> ParserResult<(&'a str, &'a str)> {
    match headers
        .iter()
        .find(|h| h.name.eq_ignore_ascii_case(CONTENT_TYPE_HEADER))
    {
        Some(content_type) => match content_type.body().split_once('/') {
            Some((t, subtype)) => Ok((t, subtype)),
            _ => Err(ParserError::InvalidMail(format!(
                "Invalid {} value: {}",
                CONTENT_TYPE_HEADER,
                content_type.body()
            ))),
        },
        None if parent.is_some() => {
            #[allow(clippy::option_if_let_else)]
            match parent
                .unwrap()
                .iter()
                .find(|h| h.name.eq_ignore_ascii_case(CONTENT_TYPE_HEADER))
            {
                Some(content_type)
                    if content_type.body().eq_ignore_ascii_case("multipart/digest") =>
                {
                    Ok(("message", "rfc822"))
                }
                _ => Ok(("text", "plain")),
            }
        }
        _ => Ok(("text", "plain")),
    }
}

impl std::fmt::Display for Mime {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for i in &self.headers {
            write!(f, "{i}")?;
        }

        f.write_str("\r\n")?;

        match &self.part {
            Part::Text(content) | Part::Html(content) | Part::Binary(content) => {
                for i in content {
                    write!(f, "{i}")?;
                }
                Ok(())
            }
            Part::Multipart(multipart) => {
                let boundary = self.boundary().unwrap();
                write!(
                    f,
                    "{}",
                    MultipartDisplayable {
                        inner: multipart,
                        boundary,
                        top_level: false,
                    }
                )
            }
            Part::Embedded(mail) => write!(f, "{mail}"),
        }
    }
}

impl Mime {
    /// Return the Mime part without any attachments.
    #[must_use]
    pub fn to_string_without_attachments(&self) -> String {
        let mut f = String::new();

        for i in &self.headers {
            f.push_str(&i.to_string());
        }

        f.push_str("\r\n");

        let is_attachment = self.is_attachment();
        match &self.part {
            Part::Multipart(multipart) => {
                let boundary = self.boundary().unwrap();

                f.push_str(
                    &MultipartDisplayable {
                        inner: multipart,
                        boundary,
                        top_level: false,
                    }
                    .to_string_without_attachments(),
                );
            }
            Part::Text(content) | Part::Html(content) if !is_attachment => {
                f.push_str(&content.join(""));
            }
            _ => {
                // Ignore text/html marked as attachments, binaries and embedded emails.
                String::default();
            }
        };

        f
    }

    /// Return the Mime part without any attachments for the top level mime section.
    #[must_use]
    pub fn to_string_without_attachments_top_level(&self) -> String {
        let mut f = String::new();

        f.push_str("\r\n");

        let is_attachment = self.is_attachment();
        match &self.part {
            Part::Multipart(multipart) => {
                let boundary = self.boundary().unwrap();

                f.push_str(
                    &MultipartDisplayable {
                        inner: multipart,
                        boundary,
                        top_level: true,
                    }
                    .to_string_without_attachments(),
                );
            }
            Part::Text(content) | Part::Html(content) if !is_attachment => {
                f.push_str(&content.join(""));
            }
            _ => {
                // Ignore text/html marked as attachments, binaries and embedded emails.
                String::default();
            }
        };

        f
    }

    /// Return the Mime part without headers.
    #[must_use]
    pub fn to_string_top_level(&self) -> String {
        let mut f = String::new();

        f.push_str("\r\n");

        match &self.part {
            Part::Text(content) | Part::Html(content) | Part::Binary(content) => {
                for i in content {
                    f.push_str(i.as_str());
                }
            }
            Part::Multipart(multipart) => {
                let boundary = self.boundary().unwrap();
                f.push_str(
                    &MultipartDisplayable {
                        inner: multipart,
                        boundary,
                        top_level: true,
                    }
                    .to_string(),
                );
            }
            Part::Embedded(mail) => f.push_str(&mail.to_string()),
        };

        f
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use crate::mime::headers::Arg;

    use super::*;

    #[test]
    fn mime_header() {
        let input = Header::new_unchecked(
            CONTENT_TYPE_HEADER.to_string(),
            " text/plain".to_string(),
            vec![
                Arg::from_str(" charset=us-ascii").unwrap(),
                Arg::from_str(" another=\"argument\"").unwrap(),
            ],
        );

        pretty_assertions::assert_eq!(
            input.arg("charset").unwrap().value(),
            "us-ascii".to_string()
        );
        pretty_assertions::assert_eq!(
            input.arg("another").unwrap().value(),
            "argument".to_string()
        );
        pretty_assertions::assert_eq!(
            input.to_string(),
            "Content-Type: text/plain; charset=us-ascii; another=\"argument\""
        );

        let input = Header::new_unchecked(
            CONTENT_TYPE_HEADER.to_string(),
            " application/foobar".to_string(),
            Vec::default(),
        );

        pretty_assertions::assert_eq!(
            input.to_string(),
            "Content-Type: application/foobar".to_string()
        );
        pretty_assertions::assert_eq!(input.body(), "application/foobar".to_string());
    }
}
