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

/// Errors raised by the parser.
#[allow(clippy::module_name_repetitions)]
#[derive(Debug, thiserror::Error)]
pub enum ParserError {
    ///
    #[error("{0}")]
    Io(#[from] std::io::Error),
    /// The buffer is longer than expected.
    #[error("buffer is not supposed to be longer than {expected} bytes but got {got}")]
    BufferTooLong {
        /// Maximum size expected.
        expected: usize,
        /// Actual size.
        got: usize,
    },
    ///
    #[error("parsing email failed: {0}")]
    InvalidMail(String),
    ///
    #[error("Mandatory header '{0}' not found")]
    MandatoryHeadersNotFound(String),
    ///
    #[error("Boundary not found in Content-Type header parameters, {0}")]
    BoundaryNotFound(String),
    ///
    #[error("Misplaced boundary in mime message, {0}")]
    MisplacedBoundary(String),
}

/// Result emitted by the parser.
pub type ParserResult<T> = Result<T, ParserError>;
