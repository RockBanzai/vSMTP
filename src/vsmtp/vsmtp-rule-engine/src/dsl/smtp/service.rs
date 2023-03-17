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

use vsmtp_common::status::SmtpConnection;

/// A service used to communicate with a third-party program using SMTP.
#[derive(Debug, Clone)]
pub struct Smtp {
    /// A transport to handle transactions to the delegate.
    pub delegator: SmtpConnection,
    /// Delegation results address.
    pub receiver: std::net::SocketAddr,
}

impl std::fmt::Display for Smtp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "smtp")
    }
}
