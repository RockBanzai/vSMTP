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

use crate::mime::{self, Mime};

/// Raw body of an email.
#[derive(Clone, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub enum Body {
    Raw(Vec<String>),
    Parsed(ParsedBody),
    Empty,
}

impl Default for Body {
    fn default() -> Self {
        Self::Raw(vec![])
    }
}

/// see rfc5322 (section 2.1 and 2.3)
#[derive(Clone, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub enum ParsedBody {
    /// Text message body
    Text(Vec<String>),
    /// Mime part.
    Mime(Box<Mime>),
    /// Empty message body
    Empty,
}

impl ParsedBody {
    /// Get references on all attachments marked as binary or embedded.
    pub fn attachments(&self) -> Vec<&mime::Mime> {
        match self {
            ParsedBody::Mime(mime) => Self::get_attachment_from_mime(mime),
            _ => vec![],
        }
    }

    /// Helper to fetch attachments recursively in multipart mime sections.
    fn get_attachment_from_mime(mime: &Mime) -> Vec<&mime::Mime> {
        match &mime.part {
            mime::Part::Multipart(multipart) => {
                let mut attachments = Vec::with_capacity(multipart.parts.len());

                for part in &multipart.parts {
                    attachments.extend(Self::get_attachment_from_mime(part));
                }

                attachments
            }
            _ if mime.is_attachment() => vec![mime],
            _ => vec![],
        }
    }
}

impl std::fmt::Display for Body {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Body::Raw(raw) => write!(f, "{}", raw.join("")),
            Body::Parsed(parsed) => match parsed {
                ParsedBody::Text(content) => {
                    for i in content {
                        if i.starts_with('.') {
                            std::fmt::Write::write_char(f, '.')?;
                        }
                        f.write_str(i)?;
                    }
                    Ok(())
                }
                ParsedBody::Mime(content) => {
                    // Top-level mime headers are used for parsing, and are kept in this
                    // top-level mime instance. but they MUST appear in order
                    // when the email is turned into bytes.
                    //
                    // To prevent writing the top-level mime headers at the end of the
                    // top-level header section all the time (which is not right: mime headers
                    // can be placed between regular header), we simply keep them in the root
                    // mime object but discard them when turning it into bytes.
                    write!(f, "{}", content.to_string_top_level())
                }
                ParsedBody::Empty => Ok(()),
            },
            Body::Empty => Ok(()),
        }
    }
}

impl Body {
    /// Return the body without any attachments.
    pub fn to_string_without_attachments(&self) -> String {
        match self {
            Body::Raw(raw) => raw.join(""),
            Body::Parsed(parsed) => match parsed {
                ParsedBody::Text(content) => content.join(""),
                ParsedBody::Mime(content) => content.to_string_without_attachments_top_level(),
                ParsedBody::Empty => Default::default(),
            },
            Body::Empty => Default::default(),
        }
    }
}
