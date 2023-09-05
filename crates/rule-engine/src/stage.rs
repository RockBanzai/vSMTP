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

/// "Hooks" used to identify when to run a batch of [`Directives`].
pub trait Stage:
    std::fmt::Debug + Copy + Clone + Ord + std::fmt::Display + std::str::FromStr + Send + Sync
{
    /// Return the name of the rhai function that will be executed when the stage is reached.
    fn hook(&self) -> &'static str;

    /// Return all stages as strings.
    fn stages() -> &'static [&'static str];
}
