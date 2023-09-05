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

//! A library to parse and modify emails.

// #![doc(html_no_source)]
// #![deny(missing_docs)]
// #![forbid(unsafe_code)]
// //
// #![warn(rust_2018_idioms)]
// #![warn(clippy::all)]
// #![warn(clippy::pedantic)]
// #![warn(clippy::nursery)]
// #![warn(clippy::cargo)]

/// average size of a mail
pub const MAIL_SIZE: usize = 1_000_000; // 1MB

/// Errors raised by the parser.
pub mod errors;
/// Rust representation of an email.
pub mod mail;
/// Facilities used to parse and store MIME components.
pub mod mime;
/// Code to parse and validate an email.
pub mod parsing;

pub use errors::{ParserError, ParserResult};
pub use mail::Mail;
