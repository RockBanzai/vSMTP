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

/// Build a complete path from the current cargo manifest files using a relative path.
#[macro_export]
macro_rules! from_manifest_path {
    ($path:expr) => {
        std::path::PathBuf::from_iter([env!("CARGO_MANIFEST_DIR"), $path])
    };
}
