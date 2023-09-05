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

use rhai::plugin::{
    mem, Dynamic, FnAccess, FnNamespace, ImmutableString, NativeCallContext, PluginFunction,
    RhaiResult, TypeId,
};
use rhai::Module;

/// the access mode to the database.
#[derive(
    Clone, Debug, Default, serde_with::DeserializeFromStr, strum::EnumString, strum::Display,
)]
#[allow(clippy::module_name_repetitions)]
pub enum AccessMode {
    #[strum(serialize = "O_RDONLY")]
    Read,
    #[strum(serialize = "O_WRONLY")]
    Write,
    #[default]
    #[strum(serialize = "O_RDWR")]
    ReadWrite,
}

/// refresh rate of the database.
#[derive(Default, Clone, Debug, serde::Deserialize, strum::Display)]
#[serde(rename_all = "lowercase")]
#[strum(serialize_all = "lowercase")]
pub enum Refresh {
    #[default]
    Always,
    No,
}

/// A database connector based on the csv file format.
#[derive(Debug, Clone)]
pub struct Csv {
    /// A path to the file to open.
    pub path: std::path::PathBuf,
    /// Access mode to the database.
    pub access: AccessMode,
    /// Delimiter character to separate fields in records.
    pub delimiter: char,
    /// Database refresh mode.
    pub refresh: Refresh,
    /// Raw content of the database.
    pub fd: std::sync::Arc<std::fs::File>,
}

impl std::fmt::Display for Csv {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "csv")
    }
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("error occurred while parsing csv file: {0}")]
    Parsing(#[from] csv::Error),
    #[error("error occurred while writing csv file: {error} at {path}")]
    Write {
        error: csv::Error,
        path: std::path::PathBuf,
    },
}

impl Csv {
    /// Query a record matching the first element.
    pub fn query(&self, key: &str) -> Result<Option<csv::StringRecord>, Error> {
        let mut reader = csv::ReaderBuilder::new()
            .has_headers(false)
            .trim(csv::Trim::All)
            .delimiter(self.delimiter as u8)
            .from_reader(&*self.fd);

        for record in reader.records() {
            let record = record?;
            if record.get(0).filter(|fst| *fst == key).is_some() {
                return Ok(Some(record));
            }
        }

        Ok(None)
    }

    /// Add a record.
    pub fn add_record(&self, record: &[String]) -> Result<(), Error> {
        let mut writer = csv::WriterBuilder::new()
            .has_headers(false)
            .delimiter(self.delimiter as u8)
            .from_writer(&*self.fd);

        writer.write_record(record).map_err(|e| Error::Write {
            error: e,
            path: self.path.clone(),
        })?;

        writer.flush().map_err(|e| Error::Write {
            error: e.into(),
            path: self.path.clone(),
        })?;

        Ok(())
    }

    /// Remove a record.
    pub fn remove_record(&self, key: &str) -> Result<(), Error> {
        let content = std::fs::read_to_string(&self.path).map_err(|e| Error::Write {
            error: e.into(),
            path: self.path.clone(),
        })?;

        let mut writer = std::io::BufWriter::new(
            std::fs::OpenOptions::new()
                .write(true)
                .truncate(true)
                .open(&self.path)
                .map_err(|e| Error::Write {
                    error: e.into(),
                    path: self.path.clone(),
                })?,
        );

        for line in content.lines() {
            if !line.starts_with(key) {
                std::io::Write::write_vectored(
                    &mut writer,
                    &[
                        std::io::IoSlice::new(line.as_bytes()),
                        std::io::IoSlice::new(b"\n"),
                    ],
                )
                .map_err(|e| Error::Write {
                    error: e.into(),
                    path: self.path.clone(),
                })?;
            }
        }

        std::io::Write::flush(&mut writer).map_err(|e| Error::Write {
            error: e.into(),
            path: self.path.clone(),
        })?;

        Ok(())
    }
}

#[derive(Debug, serde::Deserialize)]
pub struct CsvDatabaseParameters {
    /// Path to the csv file.
    pub connector: std::path::PathBuf,
    /// Write & read access modes.
    #[serde(default)]
    pub access: AccessMode,
    /// Refresh policy.
    #[serde(default)]
    pub refresh: Refresh,
    /// Delimiter used to separate fields.
    #[serde(default = "default_delimiter")]
    pub delimiter: char,
}

const fn default_delimiter() -> char {
    ','
}

#[rhai::plugin::export_module]
pub mod csv_api {

    type CsvFile = rhai::Shared<Csv>;

    /// # rhai-autodocs:index:1
    #[rhai_fn(global, return_raw)]
    pub fn file(parameters: rhai::Map) -> Result<CsvFile, Box<rhai::EvalAltResult>> {
        let parameters = rhai::serde::from_dynamic::<CsvDatabaseParameters>(&parameters.into())?;

        let fd = std::sync::Arc::new(
            std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .read(match parameters.access {
                    AccessMode::ReadWrite | AccessMode::Read => true,
                    AccessMode::Write => false,
                })
                .write(match parameters.access {
                    AccessMode::ReadWrite | AccessMode::Write => true,
                    AccessMode::Read => false,
                })
                .open(&parameters.connector)
                .map_err::<rhai::EvalAltResult, _>(|err| err.to_string().into())?,
        );

        Ok(rhai::Shared::new(Csv {
            path: parameters.connector,
            delimiter: parameters.delimiter,
            access: parameters.access,
            refresh: parameters.refresh,
            fd,
        }))
    }

    /// # rhai-autodocs:index:2
    #[rhai_fn(global, pure)]
    pub fn to_string(database: &mut CsvFile) -> String {
        database.to_string()
    }

    /// # rhai-autodocs:index:3
    #[rhai_fn(global, pure)]
    pub fn to_debug(database: &mut CsvFile) -> String {
        format!("{database:#?}")
    }

    /// Add a record.
    /// # rhai-autodocs:index:4
    #[rhai_fn(global, name = "set", return_raw, pure)]
    pub fn database_add(
        database: &mut CsvFile,
        record: rhai::Array,
    ) -> Result<(), Box<rhai::EvalAltResult>> {
        let record = record
            .into_iter()
            .map(rhai::Dynamic::try_cast)
            .collect::<Option<Vec<String>>>()
            .ok_or_else::<Box<rhai::EvalAltResult>, _>(|| {
                "all fields in a record must be strings".into()
            })?;

        database
            .add_record(&record)
            .map_err::<Box<rhai::EvalAltResult>, _>(|err| err.to_string().into())
    }

    /// Remove a record.
    /// # rhai-autodocs:index:5
    #[rhai_fn(global, name = "rm", return_raw, pure)]
    pub fn remove_str(database: &mut CsvFile, key: &str) -> Result<(), Box<rhai::EvalAltResult>> {
        database
            .remove_record(key)
            .map_err::<Box<rhai::EvalAltResult>, _>(|err| err.to_string().into())
    }

    /// Query the database.
    /// # rhai-autodocs:index:6
    #[rhai_fn(global, name = "get", return_raw, pure)]
    pub fn query(
        database: &mut CsvFile,
        key: &str,
    ) -> Result<rhai::Array, Box<rhai::EvalAltResult>> {
        database
            .query(key)
            .map_err::<Box<rhai::EvalAltResult>, _>(|err| err.to_string().into())?
            .map_or_else(
                || Ok(rhai::Array::default()),
                |record| {
                    Ok(record
                        .into_iter()
                        .map(|field| rhai::Dynamic::from(field.to_string()))
                        .collect())
                },
            )
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::api::csv_api;
    use rhai::Engine;

    #[test]
    fn test_opening_file() {
        let mut engine = Engine::new();
        engine.register_type_with_name::<AccessMode>("AccessMode");
        let map = engine.parse_json(
            r#"
                {
                    "connector": "tests/dummy_file.csv",
                    "access": "O_RDONLY",
                }"#,
            true,
        );
        csv_api::file(map.unwrap()).unwrap();
    }

    #[test]
    fn query() {
        let mut engine = Engine::new();
        engine.register_type_with_name::<AccessMode>("AccessMode");
        let map = engine.parse_json(
            r#"
                {
                    "connector": "tests/dummy_file.csv",
                    "access": "O_RDONLY",
                }"#,
            true,
        );
        let mut db = csv_api::file(map.unwrap()).unwrap();
        let expected = vec![rhai::Dynamic::from("id"), rhai::Dynamic::from("1")];
        assert_eq!(
            csv_api::query(&mut db, "id")
                .unwrap()
                .get(0)
                .unwrap()
                .to_string(),
            expected.get(0).unwrap().to_string()
        );
    }
}
