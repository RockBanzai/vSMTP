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

use std::{collections::HashMap, str::FromStr};

#[derive(Debug, serde::Deserialize, serde::Serialize, PartialEq, Eq)]
pub struct Logs {
    #[serde(default = "Logs::default_queue")]
    pub queue: String,
    #[serde(default)]
    pub facility: LogsFacility,
    // FIXME: probably unnecessary because of the log-dispatcher crate, which already handles level filters.
    //        should it be up to the service or the log-dispatcher to handle this ?
    /// Customize the log level of the different part of the program.
    ///
    /// See <https://docs.rs/tracing-subscriber/0.3.15/tracing_subscriber/filter/struct.EnvFilter.html>
    #[serde(
        default = "Logs::default_levels",
        serialize_with = "Logs::serialize_levels",
        deserialize_with = "Logs::deserialize_levels"
    )]
    pub levels: HashMap<String, tracing_subscriber::filter::LevelFilter>,
}

impl Logs {
    fn default_queue() -> String {
        "log".to_string()
    }

    fn default_levels() -> HashMap<String, tracing_subscriber::filter::LevelFilter> {
        HashMap::from([(
            "default".to_string(),
            tracing_subscriber::filter::LevelFilter::WARN,
        )])
    }

    fn serialize_levels<S: serde::Serializer>(
        value: &HashMap<String, tracing_subscriber::filter::LevelFilter>,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        let mut x = serializer.serialize_map(Some(value.len()))?;
        for i in value {
            serde::ser::SerializeMap::serialize_entry(&mut x, &i.0, &i.1.to_string())?;
        }
        serde::ser::SerializeMap::end(x)
    }

    fn deserialize_levels<'de, D>(
        deserializer: D,
    ) -> Result<HashMap<String, tracing_subscriber::filter::LevelFilter>, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = <rhai::Map as serde::Deserialize>::deserialize(deserializer)?;
        value
            .into_iter()
            .map(|key| {
                tracing_subscriber::filter::LevelFilter::from_str(key.1.to_string().as_str())
                    .map(|level| (key.0.to_string(), level))
                    .map_err(|e| {
                        serde::de::Error::custom(format!("Failed to parse log level: `{e}`"))
                    })
            })
            .collect::<Result<HashMap<String, tracing_subscriber::filter::LevelFilter>, _>>()
    }
}

#[derive(Default, Debug, serde::Deserialize, serde::Serialize, PartialEq, Eq)]
pub enum LogsFacility {
    Syslog(SyslogTransport),
    File(std::path::PathBuf),
    #[default]
    Console, // stream ?
}

#[derive(Debug, serde::Deserialize, serde::Serialize, PartialEq, Eq)]
pub enum SyslogTransport {
    Udp { socket: std::net::SocketAddr },
    Tcp { socket: std::net::SocketAddr },
    Unix { path: std::path::PathBuf },
}

impl Default for Logs {
    fn default() -> Self {
        Self {
            queue: Self::default_queue(),
            facility: LogsFacility::default(),
            levels: Self::default_levels(),
        }
    }
}
