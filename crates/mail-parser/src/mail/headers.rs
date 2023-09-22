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

use std::ops::{Deref, DerefMut};

/// Header of an email.
/// <https://www.rfc-editor.org/rfc/rfc2822#section-2.2>
#[derive(Debug, Default, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub struct Header {
    pub name: String,
    pub body: String,
}

impl Header {
    // TODO: handle folding here.
    /// Create a new header.
    /// This method will add the `\r\n` directly at the end of the value
    /// field.
    pub fn new(name: impl Into<String>, body: impl AsRef<str>) -> Self {
        Self {
            name: name.into(),
            body: format!(" {}\r\n", body.as_ref()),
        }
    }

    /// Create a new header, but without adding a newline to the body
    /// and folding it automatically.
    pub fn new_unchecked(name: impl Into<String>, body: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            body: body.into(),
        }
    }

    /// Return the header as a string "name: body" without the CRLF bit.
    #[must_use]
    pub fn to_string_without_crlf(&self) -> String {
        format!(
            "{}:{}",
            self.name,
            self.body.strip_suffix("\r\n").unwrap_or(&self.body)
        )
    }
}

impl std::fmt::Display for Header {
    // NOTE: simplified version for rhai apis.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}", self.name, self.body)
    }
}

/// List of top-level headers.
/// We use `Vec` instead of a `HashMap` because header ordering is mandatory.
/// <https://www.rfc-editor.org/rfc/rfc2822#section-3.6>
#[derive(Debug, Default, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub struct Headers(pub Vec<Header>);

impl From<Vec<Header>> for Headers {
    fn from(value: Vec<Header>) -> Self {
        Self(value)
    }
}

impl Deref for Headers {
    type Target = Vec<Header>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for Headers {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl std::fmt::Display for Headers {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for h in &self.0 {
            write!(f, "{}:{}", h.name, h.body)?;
        }
        Ok(())
    }
}

#[derive(Debug)]
struct HeaderFoldable<'a>(&'a Header);

impl<'a> std::fmt::Display for HeaderFoldable<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let key = convert_case::Casing::to_case(&self.0.name, convert_case::Case::Train)
            .replace("Id", "ID")
            .replace("Mime-Version", "MIME-Version")
            .replace("Dkim", "DKIM")
            .replace("Arc", "ARC")
            .replace("Spf", "SPF")
            .replace("X-Ms", "X-MS")
            .replace("X-Vr", "X-VR");

        f.write_str(&key)?;
        f.write_str(": ")?;

        let mut byte_writable = self.0.body.as_str();
        if byte_writable.is_empty() {
            return f.write_str("\r\n");
        }

        let mut prev = key.len() + 2;

        // FIXME: we can fold at 78 chars for simple sentence.
        // but must write a continuous string for base64 encoded values (like dkim)
        while !byte_writable.is_empty() {
            let (left, right) = if byte_writable.len() + prev > 998 {
                byte_writable[..998 - prev]
                    .rfind(char::is_whitespace)
                    .map(|idx| (&byte_writable[..idx], &byte_writable[idx..]))
            } else {
                None
            }
            .unwrap_or((byte_writable, ""));

            f.write_str(left)?;
            f.write_str("\r\n")?;

            byte_writable = right;
            if !byte_writable.is_empty() {
                std::fmt::Write::write_char(f, '\t')?;
                prev = 1;
            }
        }
        Ok(())
    }
}

/// Read the current line or folded content and extracts a header if there is any.
///
/// # Arguments
///
/// * `content` - The buffer of lines to parse. this function has the right
///               to iterate through the buffer because it can parse folded
///               headers.
///
/// # Return
///
/// * `Option<(String, String)>` - An option containing two strings,
///                                the name and value of the header parsed
pub fn read_header<C: AsRef<str>>(content: &mut &[C]) -> Option<(String, String)> {
    let mut split = content[0].as_ref().splitn(2, ':');

    match (split.next(), split.next()) {
        (Some(header), Some(body)) => {
            let folded_body = content[1..]
                .iter()
                .take_while(|line| line.as_ref().starts_with(|c| c == ' ' || c == '\t'))
                .map(|line| {
                    *content = &content[1..];
                    line.as_ref()
                })
                .collect::<Vec<&str>>()
                .join("");

            Some((
                // FIXME: headers must not be modified.
                // FIXME: headers must not contain WSP.
                header.trim().into(),
                if folded_body.is_empty() {
                    body.to_string()
                } else {
                    format!("{body}{folded_body}")
                },
            ))
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_read_header() {
        let input = [
            "User-Agent: Mozilla/5.0 (X11; Linux x86_64; rv:78.0) Gecko/20100101\r\n",
            " Thunderbird/78.8.1\r\n",
        ];
        assert_eq!(
            read_header(&mut (&input[..])),
            Some((
                "User-Agent".to_string(),
                " Mozilla/5.0 (X11; Linux x86_64; rv:78.0) Gecko/20100101\r\n Thunderbird/78.8.1\r\n"
                    .to_string()
            ))
        );
    }

    #[test]
    fn test_read_header_with_extra_wsp() {
        let input = [
            "User-Agent:    \t Mozilla/5.0 (X11; Linux x86_64; rv:78.0) Gecko/20100101\r\n",
            " Thunderbird/78.8.1\r\n",
        ];
        assert_eq!(
            read_header(&mut (&input[..])),
            Some((
                "User-Agent".to_string(),
                "    \t Mozilla/5.0 (X11; Linux x86_64; rv:78.0) Gecko/20100101\r\n Thunderbird/78.8.1\r\n"
                    .to_string()
            ))
        );
    }

    #[test]
    fn test_read_long_header() {
        let input = ["User-Agent: Mozilla/5.0 (X11; Linux x86_64; rv:78.0) Gecko/20100101 (this is a comment witch makes the header body longer than it should)"];
        assert_eq!(
            read_header(&mut (&input[..])),
            Some((
                "User-Agent".to_string(),
                " Mozilla/5.0 (X11; Linux x86_64; rv:78.0) Gecko/20100101 (this is a comment witch makes the header body longer than it should)"
                    .to_string()
            ))
        );
    }
}
