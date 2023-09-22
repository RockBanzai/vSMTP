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

/// Header of a mime section.
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub struct Header {
    ///
    pub name: String,
    ///
    body: String,
    /// parameter ordering does not matter.
    args: Vec<Arg>,
}

/// Argument of an header.
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub struct Arg {
    /// Raw name of the parameter.
    name: String,
    /// Value of the argument, that can be wrapped in quotes.
    value: String,
    /// Stores the start of the value without the non-desired character, like quotes.
    value_start: usize,
    /// Stores the end of the value without the non-desired character, like quotes and CRLF.
    value_end: usize,
}

impl From<&crate::mail::headers::Header> for Header {
    fn from(value: &crate::mail::headers::Header) -> Self {
        crate::parsing::bytes::get_mime_header(&value.name, &value.body)
    }
}

impl Header {
    /// Create a new header, but without adding a newline to the body
    /// and folding it automatically.
    pub fn new_unchecked(name: impl Into<String>, body: impl Into<String>, args: Vec<Arg>) -> Self {
        Self {
            name: name.into(),
            body: body.into(),
            args,
        }
    }

    /// Get the body of the trimmed header.
    #[must_use]
    pub fn body(&self) -> &str {
        self.body.trim()
    }

    /// Find an argument in the current header.
    #[must_use]
    pub fn arg(&self, needle: &str) -> Option<&Arg> {
        self.args
            .iter()
            .find(|arg| arg.name().eq_ignore_ascii_case(needle))
    }
}

// TODO: handle folding here
impl std::fmt::Display for Header {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.name)?;
        f.write_str(":")?;
        f.write_str(&self.body)?;

        for arg in &self.args {
            f.write_fmt(format_args!(";{}={}", arg.name, arg.raw_value()))?;
        }

        Ok(())
    }
}

impl std::str::FromStr for Arg {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let Some((name, value)) = s.split_once('=') else {
            return Err(());
        };

        let name = name.to_string();
        let value = value.to_string();

        let mut value_start = 0;
        let mut value_end = value.len();

        // Guaranty access to the argument value without any quotes,
        // if there are any.
        // We can't use a simple `trim` getter like the `Arg::name()`
        // method because quotes are not WSPs.
        match (value.find('"'), value.rfind('"')) {
            (Some(start), Some(end)) if start < end => {
                value_start = start + 1;
                value_end = end;
            }
            _ => {
                // If there are no quotes, we still need to trim any CRLF.
                if let Some(end) = value.rfind("\r\n") {
                    value_end = end;
                }
            }
        };

        Ok(Self {
            name,
            value,
            value_start,
            value_end,
        })
    }
}

impl Arg {
    /// Get the trimmed name of the argument.
    #[must_use]
    pub fn name(&self) -> &str {
        self.name.trim()
    }

    /// Get the trimmed value of the argument.
    #[must_use]
    pub fn value(&self) -> &str {
        &self.value[self.value_start..self.value_end]
    }

    /// Get the full value of the argument, with quotes and other characters.
    #[must_use]
    pub fn raw_value(&self) -> &str {
        &self.value
    }

    /// Get the full mutable value of the argument, with quotes and other characters.
    pub fn mut_value(&mut self) -> &mut String {
        &mut self.value
    }
}
