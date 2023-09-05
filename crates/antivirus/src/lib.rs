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

/// A trait to implement an antivirus plugin.
pub trait Antivirus {
    /// Scan bytes for viruses.
    ///
    /// # Return
    ///
    /// True if the data contains a virus, false otherwise.
    ///
    /// # Errors
    ///
    /// Any i/o error. Note: a custom error could be desirable.
    fn scan(&self, bytes: &[u8]) -> Result<bool, std::io::Error>;
}

/// Wrapper used to pass trait objects by parameters of Rhai functions.
/// This was implemented because Rhai does not seem to be able
/// to cast dynamic sized types.
#[derive(Clone)]
pub struct RhaiAntivirus(pub rhai::Shared<dyn Antivirus>);

unsafe impl Send for RhaiAntivirus {}
unsafe impl Sync for RhaiAntivirus {}
