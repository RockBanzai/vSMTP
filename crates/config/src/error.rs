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

#[derive(thiserror::Error, Debug)]
pub enum ConfigError {
    #[error("file `{0}` does not have a valid parent directory for rhai configuration files")]
    InvalidParentDirectory(std::path::PathBuf),
    #[error("failed to create rhai configuration object: `{0}`")]
    Serialize(#[from] serde_json::Error),
    #[error("failed to create rhai configuration object: `{0}`")]
    Deserialize(#[from] serde_path_to_error::Error<serde_json::Error>),
    #[error("failed to open script for configuration at `{0}`: {1}")]
    FileOpen(std::path::PathBuf, std::io::Error),
    #[error("failed to compile a rhai script for configuration: `{0}`")]
    Compilation(#[from] rhai::ParseError),
    #[error("failed to execute a rhai script for configuration: `{0}`")]
    Execution(#[from] Box<rhai::EvalAltResult>),
}
