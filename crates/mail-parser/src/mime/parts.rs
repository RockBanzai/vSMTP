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

use crate::mail::Mail;

use super::Mime;

/// Type of a Mime part.
/// https://www.rfc-editor.org/rfc/rfc2045#section-5
#[derive(Clone, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub enum Part {
    /// Text content type.
    Text(Vec<String>),
    /// HTML content type.
    Html(Vec<String>),
    /// Any other content type that is not text nor HTML.
    Binary(Vec<String>),
    /// Multipart content type.
    Multipart(Multipart),
    /// Embedded mail content type.
    Embedded(Mail),
}

/// Boundary separated parts.
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub struct Multipart {
    /// https://www.rfc-editor.org/rfc/rfc2046#section-5
    pub preamble: String,
    ///
    pub parts: Vec<Mime>,
    /// https://www.rfc-editor.org/rfc/rfc2046#section-5
    pub epilogue: String,
}

pub struct MultipartDisplayable<'a> {
    pub(crate) inner: &'a Multipart,
    pub(crate) boundary: &'a str,
    pub(crate) top_level: bool,
}

impl<'a> std::fmt::Display for MultipartDisplayable<'a> {
    //  preamble
    //  --boundary
    //  *{ headers \n body \n boundary}
    //  epilogue || nothing
    //  --end-boundary--
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if !self.inner.preamble.is_empty() {
            f.write_str(&self.inner.preamble)?;
        }

        for i in &self.inner.parts {
            f.write_fmt(format_args!("--{}\r\n", self.boundary))?;
            f.write_fmt(format_args!("{i}"))?;
        }

        if !self.inner.epilogue.is_empty() {
            f.write_str(&self.inner.epilogue)?;
        }

        if self.top_level {
            f.write_fmt(format_args!("--{}--\r\n", self.boundary))?;
        } else {
            f.write_fmt(format_args!("--{}--\r\n\r\n", self.boundary))?;
        }

        Ok(())
    }
}

impl<'a> MultipartDisplayable<'a> {
    pub fn to_string_without_attachments(&self) -> String {
        let mut f = String::new();
        if !self.inner.preamble.is_empty() {
            f.push_str(&self.inner.preamble);
        }

        for i in &self.inner.parts {
            f.push_str("--");
            f.push_str(self.boundary);
            f.push_str("\r\n");
            f.push_str(&i.to_string_without_attachments());
        }

        if !self.inner.epilogue.is_empty() {
            f.push_str(&self.inner.epilogue);
        }

        f.push_str("--");
        f.push_str(self.boundary);

        if self.top_level {
            f.push_str("--\r\n");
        } else {
            f.push_str("--\r\n\r\n");
        }

        f
    }
}
