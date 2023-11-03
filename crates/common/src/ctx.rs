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

use crate::{stateful_ctx_received::StatefulCtxReceived, DeserializeError, SerializeError};

/// Global context that is sent between services.
#[derive(Debug, serde::Serialize, serde::Deserialize, Clone)]
pub struct Ctx<T> {
    /// User defined variables that can be accessed in scripts.
    pub variables: std::collections::HashMap<String, rhai::Dynamic>,
    /// Developer defined variables used to enable "experimental" features.
    /// Those data can be added, subtracted, or modified during non-breaking release
    /// to try new stuff.
    pub internal: std::collections::HashMap<String, rhai::Dynamic>,
    /// Email metadata associated to a service.
    pub metadata: T,
}

impl<'a, T: serde::Deserialize<'a>> Ctx<T> {
    pub fn from_json(bytes: &'a [u8]) -> Result<Self, DeserializeError> {
        match serde_json::from_slice(bytes) {
            Ok(this) => Ok(this),
            Err(err) => Err(DeserializeError::Error(err)),
        }
    }
}

impl<T: serde::Serialize> Ctx<T> {
    pub fn to_json(&self) -> Result<Vec<u8>, SerializeError> {
        match serde_json::to_vec(self) {
            Ok(this) => Ok(this),
            Err(err) => Err(SerializeError::Error(err)),
        }
    }
}

impl Ctx<StatefulCtxReceived> {
    pub fn produce_new(&self) -> Self {
        let mut new_instance = self.clone();
        new_instance.metadata.reset();
        new_instance
    }
}
