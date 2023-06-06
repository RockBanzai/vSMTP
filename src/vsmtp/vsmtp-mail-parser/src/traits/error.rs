/*
 * vSMTP mail transfer agent
 * Copyright (C) 2022 viridIT SAS
 *
 * This program is free software: you can redistribute it and/or modify it under
 * the terms of the GNU General Public License as published by the Free Software
 * Foundation, either version 3 of the License, or any later version.
 *
 * This program is distributed in the hope that it will be useful, but WITHOUT
 * ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
 * FOR A PARTICULAR PURPOSE.  See the GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License along with
 * this program. If not, see https://www.gnu.org/licenses/.
 *
*/
// NOTE: should be improved

///
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
    /// The email size exceeds the SIZE EHLO extension.
    #[error("mail is not supposed to be bigger than {expected} bytes but was {got} bytes long")]
    MailSizeExceeded {
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
    #[error("Boundary not found in content-type header parameters, {0}")]
    BoundaryNotFound(String),
    ///
    #[error("Misplaced boundary in mime message, {0}")]
    MisplacedBoundary(String),
}

///
pub type ParserResult<T> = Result<T, ParserError>;
