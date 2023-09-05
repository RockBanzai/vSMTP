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

use serde::{de::Deserializer, ser::Serializer, Deserialize, Serialize};
use std::sync::{Arc, RwLock};

pub mod arc_rwlock {
    pub use super::*;

    pub fn serialize<S, T>(val: &Arc<RwLock<T>>, s: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
        T: Serialize,
    {
        T::serialize(&*val.read().unwrap(), s)
    }

    pub fn deserialize<'de, D, T>(d: D) -> Result<Arc<RwLock<T>>, D::Error>
    where
        D: Deserializer<'de>,
        T: Deserialize<'de>,
    {
        Ok(Arc::new(RwLock::new(T::deserialize(d)?)))
    }
}

pub mod arc_option {
    pub use super::*;

    pub fn serialize<S, T>(val: &Option<Arc<T>>, s: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
        T: Serialize,
    {
        match val {
            Some(val) => T::serialize(val, s),
            None => Option::serialize(&None::<T>, s),
        }
    }

    pub fn deserialize<'de, D, T>(d: D) -> Result<Option<Arc<T>>, D::Error>
    where
        D: Deserializer<'de>,
        T: Deserialize<'de>,
    {
        Option::<T>::deserialize(d)?.map_or_else(|| Ok(None), |val| Ok(Some(Arc::new(val))))
    }
}
