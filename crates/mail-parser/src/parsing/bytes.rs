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

use std::str::FromStr;

use crate::mail::body::{Body, ParsedBody};
use crate::mail::headers::{read_header, Header, Headers};
use crate::mail::{Mail, DATE_HEADER, FROM_HEADER};
use crate::mime;
use crate::mime::headers::Arg;
use crate::mime::Mime;
use crate::{ParserError, ParserResult};

/// a boundary serves as a delimiter between mime parts in a multipart section.
enum BoundaryType {
    Delimiter,
    End,
    OutOfScope,
}

/// Instance parsing a message body
#[derive(Default)]
pub struct Parser {
    boundary_stack: Vec<String>,
}

impl Parser {
    // PERF: use u8 slices instead of vec.
    /// Parse the header section of an email from lines of bytes.
    /// The body is stored as is.
    ///
    /// To later parse the body, see [`Parser::parse_body_from_mail`].
    pub fn parse_headers(&mut self, bytes: Vec<Vec<u8>>) -> ParserResult<Mail> {
        let bytes = bytes
            .iter()
            .map(|l| std::str::from_utf8(l).unwrap())
            .collect::<Vec<&str>>();

        let mut headers = Headers(Vec::with_capacity(10));
        let bytes = &mut &bytes[..];

        while !bytes.is_empty() {
            match read_header(bytes) {
                Some((name, value)) => {
                    headers.0.push(Header::new_unchecked(name, value));
                }

                None => {
                    // there is an empty lines after headers
                    *bytes = &bytes[1..];

                    if bytes.is_empty() {
                        return Ok(Mail {
                            headers,
                            body: Body::Empty,
                        });
                    }

                    check_mandatory_headers(&headers.0)?;

                    return Ok(Mail {
                        headers,
                        body: Body::Raw(bytes.iter().map(|s| s.to_string()).collect()),
                    });
                }
            };

            *bytes = &bytes[1..];
        }

        Ok(Mail {
            headers,
            body: Body::Empty,
        })
    }

    /// Parse the raw body of an email that does not have it's body parsed yet.
    /// If the email is already fully parsed, it is directly returned.
    ///
    /// # Return
    ///
    /// * A mutable reference on the parsed body of the email.
    pub fn parse_body_from_mail<'m>(
        &mut self,
        mail: &'m mut Mail,
    ) -> ParserResult<&'m mut ParsedBody> {
        let mut body = std::mem::take(&mut mail.body);
        match &mut body {
            Body::Raw(raw) => {
                if Self::has_mime_version(&mail.headers) {
                    let mime_headers = mail
                        .headers
                        .0
                        .iter()
                        .filter(|header| is_mime_header(&header.name));

                    let mime = Box::new(self.as_mime_body(
                        &mut &raw[..],
                        mime_headers.into_iter().map(mime::Header::from).collect(),
                        None,
                    )?);

                    mail.body = Body::Parsed(ParsedBody::Mime(mime));
                } else {
                    mail.body = Body::Parsed(ParsedBody::Text(self.as_regular_body(&mut &raw[..])?))
                };

                match &mut mail.body {
                    Body::Parsed(parsed) => Ok(parsed),
                    _ => unreachable!("body as been parsed above"),
                }
            }
            Body::Parsed(..) => match &mut mail.body {
                Body::Parsed(parsed) => Ok(parsed),
                _ => unreachable!("body as been parsed above"),
            },
            Body::Empty => Err(ParserError::InvalidMail(
                "cannot parse the body of an empty email".to_string(),
            )),
        }
    }

    // PERF: use u8 slices instead of vec.
    /// Parse the entire content an email from lines of bytes.
    /// To only parse the header section, see [`Parser::parse_headers`].
    pub fn parse(&mut self, bytes: Vec<Vec<u8>>) -> ParserResult<Mail> {
        let bytes = bytes
            .iter()
            .map(|l| std::str::from_utf8(l).unwrap())
            .collect::<Vec<&str>>();

        self.parse_inner(&mut &bytes[..])
    }

    fn parse_inner<C: AsRef<str>>(&mut self, bytes: &mut &[C]) -> ParserResult<Mail> {
        let mut headers = Headers(Vec::with_capacity(10));
        let bytes = &mut &bytes[..];

        while !bytes.is_empty() {
            match read_header(bytes) {
                Some((name, body)) => {
                    headers.0.push(Header { name, body });
                }

                None => {
                    // there is an empty lines after headers
                    *bytes = &bytes[1..];

                    if bytes.is_empty() {
                        return Ok(Mail {
                            headers,
                            body: Body::Empty,
                        });
                    }

                    check_mandatory_headers(&headers.0)?;

                    return Ok(Mail {
                        body: if Self::has_mime_version(&headers.0) {
                            let mime_headers = headers
                                .0
                                .iter()
                                .filter(|header| is_mime_header(&header.name));

                            let mime = Box::new(self.as_mime_body(
                                bytes,
                                mime_headers.into_iter().map(mime::Header::from).collect(),
                                None,
                            )?);

                            Body::Parsed(ParsedBody::Mime(mime))
                        } else {
                            Body::Parsed(ParsedBody::Text(self.as_regular_body(bytes)?))
                        },
                        headers,
                    });
                }
            };

            *bytes = &bytes[1..];
        }

        Ok(Mail {
            headers,
            body: Body::Empty,
        })
    }

    /// Check if the "top-level" header section contains the 'MIME-Version' header.
    fn has_mime_version(headers: &[Header]) -> bool {
        headers
            .iter()
            .any(|Header { name, .. }| name.eq_ignore_ascii_case(mime::MIME_VERSION_HEADER))
    }

    fn check_boundary(&self, line: &str) -> Option<BoundaryType> {
        // we start by checking if the stack as any boundary.
        self.boundary_stack.last().and_then(|b| {
            get_boundary_type(line, b).map_or_else(
                || {
                    if self.boundary_stack[..self.boundary_stack.len() - 1]
                        .iter()
                        .any(|b| get_boundary_type(line, b).is_some())
                    {
                        Some(BoundaryType::OutOfScope)
                    } else {
                        None
                    }
                },
                Some,
            )
        })
    }

    pub(crate) fn as_regular_body<C: AsRef<str>>(
        &self,
        content: &mut &[C],
    ) -> ParserResult<Vec<String>> {
        let mut body = Vec::with_capacity(100);

        while !content.is_empty() {
            match self.check_boundary(content[0].as_ref()) {
                // the current mail ils probably embedded.
                // we can stop parsing the mail and return it.
                Some(BoundaryType::Delimiter | BoundaryType::End) => {
                    *content = &content[1..];
                    return Ok(body);
                }

                Some(BoundaryType::OutOfScope) => {
                    return Err(ParserError::MisplacedBoundary(format!(
                        "'{}' boundary is out of scope.",
                        &content[0].as_ref(),
                    )));
                }

                // we just skip the line & push the content in the body.
                None => body.push(content[0].as_ref().to_string()),
            };
            *content = &content[1..];
        }

        // EOF reached.
        Ok(body)
    }

    // TODO: merge with @as_regular_body
    fn parse_regular_mime_body<C: AsRef<str>>(
        &self,
        content: &mut &[C],
    ) -> ParserResult<Vec<String>> {
        let mut body = Vec::new();

        while !content.is_empty() {
            match self.check_boundary(content[0].as_ref()) {
                Some(BoundaryType::Delimiter | BoundaryType::End) => {
                    return Ok(body);
                }

                Some(BoundaryType::OutOfScope) => {
                    return Err(ParserError::MisplacedBoundary(format!(
                        "'{}' boundary is out of scope.",
                        &content[0].as_ref(),
                    )));
                }

                None => {
                    // We skip the header & body separation line.
                    if !(body.is_empty() && content[0].as_ref().is_empty()) {
                        body.push(content[0].as_ref().to_string());
                    }
                }
            };
            *content = &content[1..];
        }

        Ok(body)
    }

    pub(crate) fn as_mime_body<C: AsRef<str>>(
        &mut self,
        content: &mut &[C],
        headers: Vec<mime::Header>,
        parent: Option<&[mime::Header]>,
    ) -> ParserResult<Mime> {
        match mime::get_mime_type(&headers, parent)? {
            ("text", "plain") => Ok(Mime {
                headers,
                part: mime::Part::Text(self.parse_regular_mime_body(content)?),
            }),
            ("text", "html") => Ok(Mime {
                headers,
                part: mime::Part::Html(self.parse_regular_mime_body(content)?),
            }),
            ("message", _) => Ok(Mime {
                headers,
                part: mime::Part::Embedded(self.parse_inner(content)?),
            }),
            ("multipart", _) => Ok(Mime {
                headers: headers.clone(),
                part: mime::Part::Multipart(self.parse_multipart(&headers, content)?),
            }),
            _ => Ok(Mime {
                headers,
                part: mime::Part::Binary(self.parse_regular_mime_body(content)?),
            }),
        }
    }

    fn parse_mime<C: AsRef<str>>(
        &mut self,
        content: &mut &[C],
        parent: Option<&[mime::Header]>,
    ) -> ParserResult<Mime> {
        let mut headers = Vec::new();

        while content.len() > 1 {
            if let Some((name, value)) = read_header(content) {
                headers.push(get_mime_header(&name, &value));
            } else {
                // Skip the mime body separation CRLF.
                *content = &content[1..];
                break;
            };
            *content = &content[1..];
        }

        self.as_mime_body(content, headers, parent)
    }

    fn parse_preamble<'a, C: AsRef<str>>(
        &self,
        content: &'a mut &[C],
    ) -> ParserResult<Vec<&'a str>> {
        let mut preamble = Vec::new();

        while content.len() > 1 {
            match self.check_boundary(content[0].as_ref()) {
                Some(BoundaryType::Delimiter) => {
                    return Ok(preamble);
                }
                Some(BoundaryType::End) => {
                    return Err(ParserError::MisplacedBoundary(
                        "their should not be a end boundary in the preamble".to_string(),
                    ));
                }
                Some(BoundaryType::OutOfScope) => {
                    return Err(ParserError::MisplacedBoundary(format!(
                        "'{}' boundary is out of scope.",
                        &content[0].as_ref(),
                    )));
                }
                None => preamble.push(content[0].as_ref()),
            };

            *content = &content[1..];
        }

        Err(ParserError::BoundaryNotFound(
            "boundary not found after mime part preamble".to_string(),
        ))
    }

    fn parse_epilogue<'a, C: AsRef<str>>(
        &self,
        content: &'a mut &[C],
    ) -> ParserResult<Vec<&'a str>> {
        let mut epilogue = Vec::new();

        while content.len() > 1 {
            match self.check_boundary(content[0].as_ref()) {
                // there could be an ending or delimiting boundary,
                // meaning that the next lines will be part of another mime part.
                Some(BoundaryType::Delimiter | BoundaryType::End) => {
                    break;
                }
                Some(BoundaryType::OutOfScope) => {
                    return Err(ParserError::MisplacedBoundary(format!(
                        "'{}' boundary is out of scope.",
                        &content[0].as_ref(),
                    )));
                }
                None => epilogue.push(content[0].as_ref()),
            };
            *content = &content[1..];
        }

        Ok(epilogue)
    }

    #[allow(clippy::cognitive_complexity)]
    fn parse_multipart<C: AsRef<str>>(
        &mut self,
        headers: &[mime::Header],
        content: &mut &[C],
    ) -> ParserResult<mime::Multipart> {
        let content_type = headers
            .iter()
            .find(|h| h.name.eq_ignore_ascii_case(mime::CONTENT_TYPE_HEADER))
            .unwrap();

        match content_type.arg("boundary") {
            Some(arg) => self.boundary_stack.push(arg.value().to_string()),
            None => {
                return Err(ParserError::BoundaryNotFound(
                    "boundary parameter not found in Content-Type header for a multipart."
                        .to_string(),
                ))
            }
        };

        let mut multi_parts = mime::Multipart {
            preamble: self
                .parse_preamble(content)?
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(""),
            parts: Vec::new(),
            epilogue: String::new(),
        };

        while content.len() > 1 {
            match self.check_boundary(content[0].as_ref()) {
                Some(BoundaryType::Delimiter) => {
                    *content = &content[1..];

                    multi_parts
                        .parts
                        .push(self.parse_mime(content, Some(headers))?);
                }

                Some(BoundaryType::End) => {
                    self.boundary_stack.pop();
                    *content = &content[2..];
                    multi_parts.epilogue = self
                        .parse_epilogue(content)?
                        .iter()
                        .map(ToString::to_string)
                        .collect::<Vec<_>>()
                        .join("");
                    return Ok(multi_parts);
                }

                Some(BoundaryType::OutOfScope) => {
                    return Err(ParserError::MisplacedBoundary(format!(
                        "'{}' boundary is out of scope.",
                        &content[0].as_ref(),
                    )));
                }

                None => return Ok(multi_parts),
            };
        }

        Ok(multi_parts)
    }
}

pub(crate) fn check_mandatory_headers(headers: &[Header]) -> ParserResult<()> {
    /// rfc822 headers that requires to be specified.
    /// ? does they require ONLY to be at the root message ? (in case of embedded messages)
    const MANDATORY_HEADERS: [&str; 2] = [FROM_HEADER, DATE_HEADER];

    for mh in MANDATORY_HEADERS {
        if !headers.iter().any(|h| h.name.eq_ignore_ascii_case(mh)) {
            return Err(ParserError::MandatoryHeadersNotFound(mh.to_string()));
        }
    }

    Ok(())
}

/// take the name and value of a header and parses those to create
/// a `MimeHeader` struct.
///
/// # Arguments
///
/// * `name` - the name of the header.
/// * `value` - the value of the header (with all params, folded included if any).
#[must_use]
pub fn get_mime_header(name: &str, value: &str) -> mime::Header {
    // Cut the current line using the ";" separator into a vector of "arg=value" strings.
    let args = value.split(';').collect::<Vec<&str>>();
    let mut args_iter = args.iter();

    let body = args_iter.next().unwrap_or(&"").to_string();
    let mut parsed_args = args_iter
        .filter_map(|arg| Arg::from_str(arg).ok())
        .collect::<Vec<_>>();

    // In case there is a trailing ';' with a CRLF, we append it to the last
    // parsed args.
    //
    // For example:
    //
    // Content-Disposition: attachment;filename="customers.txt"; creation-date="Sat, 05 Aug 2017 19:35:36 GMT";
    //
    // There is a trailing ';' here, but because of the above `split`, the last element of the split
    // would be `\r\n`, which is not a valid argument. But we want to keep the `\r\n` because if it is
    // not here when we transform the email in bytes, the email would change compare to the original. (thus
    // breaking DKIM, DMARC, and all other protocols that check the integrity of the email)
    if let Some(last) = args.last() {
        if *last == "\r\n" {
            if let Some(last) = parsed_args.last_mut() {
                *last.mut_value() = format!("{};\r\n", last.raw_value());
            }
        }
    }

    mime::Header::new_unchecked(name.trim().to_string(), body, parsed_args)
}

// check rfc2045 p.9. Additional MIME Header Fields.
#[inline]
pub(crate) fn is_mime_header(name: &str) -> bool {
    const MIME_HEADER_START: &str = "Content-";
    name.get(0..MIME_HEADER_START.len())
        .map_or(false, |name| name.eq_ignore_ascii_case(MIME_HEADER_START))
}

// is used to deduce the boundary type.
// ! this method is called too many times, causing slow downs.
#[inline]
fn get_boundary_type(line: &str, boundary: &str) -> Option<BoundaryType> {
    match (
        // TODO: can be optimized.
        line.starts_with("--") && !line.starts_with(boundary),
        (line.ends_with("--") || line.ends_with("--\r\n")) && !line.ends_with(boundary),
        line.contains(boundary),
    ) {
        (true, false, true) => Some(BoundaryType::Delimiter),
        (true, true, true) => Some(BoundaryType::End),
        _ => None,
    }
}
