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

use mysql::prelude::Queryable;
use rhai::plugin::{
    mem, Dynamic, FnAccess, FnNamespace, ImmutableString, Module, NativeCallContext,
    PluginFunction, RhaiResult, TypeId,
};
// use vsmtp_rule_engine::rhai;

/// Parameters available for the mysql service. Used
/// with serde for easy parsing.
#[derive(Debug, serde::Deserialize)]
struct MySQLDatabaseParameters {
    pub url: String,
    #[serde(default = "default_timeout", with = "humantime_serde")]
    pub timeout: std::time::Duration,
    #[serde(default = "default_connections")]
    pub connections: rhai::INT,
}

const fn default_connections() -> rhai::INT {
    4
}

const fn default_timeout() -> std::time::Duration {
    std::time::Duration::from_secs(30)
}

/// A r2d2 connection manager for mysql.
#[derive(Clone, Debug)]
pub struct ConnectionManager {
    params: mysql::Opts,
}

impl ConnectionManager {
    pub fn new(params: mysql::OptsBuilder) -> Self {
        Self {
            params: mysql::Opts::from(params),
        }
    }
}

impl r2d2::ManageConnection for ConnectionManager {
    type Connection = mysql::Conn;
    type Error = mysql::Error;

    fn connect(&self) -> Result<mysql::Conn, mysql::Error> {
        mysql::Conn::new(self.params.clone())
    }

    fn is_valid(&self, conn: &mut mysql::Conn) -> Result<(), mysql::Error> {
        mysql::prelude::Queryable::query(conn, "SELECT version()").map(|_: Vec<String>| ())
    }

    fn has_broken(&self, conn: &mut mysql::Conn) -> bool {
        self.is_valid(conn).is_err()
    }
}

/// A database connector based on mysql.
#[derive(Debug, Clone)]
pub struct MySQLConnector {
    /// The url to the database.
    pub url: String,
    /// connection pool for the database.
    pub pool: r2d2::Pool<ConnectionManager>,
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("mysql: {0}")]
    MySQL(#[from] mysql::Error),
    #[error("pool of connection: {0}")]
    Pool(#[from] r2d2::Error),
    #[error("the row has no value")]
    RowHasNoValue,
}

impl MySQLConnector {
    pub fn query(&self, query: &str) -> Result<Vec<rhai::Map>, Error> {
        let result = self.pool.get()?.query::<mysql::Row, _>(query)?;

        let mut rows = Vec::with_capacity(result.len());

        for row in &result {
            let mut values = rhai::Map::new();

            for (index, column) in row.columns().iter().enumerate() {
                values.insert(
                    column.name_str().into(),
                    row.as_ref(index)
                        .ok_or_else(|| Error::RowHasNoValue)?
                        .as_sql(false)
                        .into(),
                );
            }

            rows.push(values);
        }

        Ok(rows)
    }
}

/// This plugin exposes methods to open a pool of connections to a MySQL database using
/// Rhai.
#[rhai::plugin::export_module]
pub mod mysql_api {

    pub type MySQL = rhai::Shared<MySQLConnector>;

    /// Open a pool of connections to a MySQL database.
    ///
    /// # Args
    ///
    /// * `parameters` - a map of the following parameters:
    ///     * `url` - a string url to connect to the database.
    ///     * `timeout` - time allowed between each query to the database. (default: `30s`)
    ///     * `connections` - Number of connections to open to the database. (default: 4)
    ///
    /// # Return
    ///
    /// A service used to query the database pointed by the `url` parameter.
    ///
    /// # Error
    ///
    /// * The service failed to connect to the database.
    ///
    /// # Example
    ///
    /// ```text
    /// // Import the plugin stored in the `plugins` directory.
    /// import "plugins/libvsmtp_plugin_mysql" as mysql;
    ///
    /// export const database = mysql::connect(#{
    ///     // Connect to a database on the system with the 'greylist-manager' user and 'my-password' password.
    ///     url: "mysql://localhost/?user=greylist-manager&password=my-password",
    ///     timeout: "1m",
    ///     connections: 1,
    /// });
    /// ```
    ///
    /// # rhai-autodocs:index:1
    #[rhai_fn(global, return_raw)]
    pub fn connect(parameters: rhai::Map) -> Result<MySQL, Box<rhai::EvalAltResult>> {
        let parameters = rhai::serde::from_dynamic::<MySQLDatabaseParameters>(&parameters.into())?;

        let opts = mysql::Opts::from_url(&parameters.url)
            .map_err::<Box<rhai::EvalAltResult>, _>(|err| err.to_string().into())?;
        let builder = mysql::OptsBuilder::from_opts(opts);
        let manager = ConnectionManager::new(builder);

        Ok(rhai::Shared::new(MySQLConnector {
            url: parameters.url,
            pool: r2d2::Pool::builder()
                .max_size(
                    u32::try_from(parameters.connections)
                        .map_err::<Box<rhai::EvalAltResult>, _>(|err| err.to_string().into())?,
                )
                .connection_timeout(parameters.timeout)
                .build(manager)
                .map_err::<Box<rhai::EvalAltResult>, _>(|err| err.to_string().into())?,
        }))
    }

    /// Query the database.
    ///
    /// # Args
    ///
    /// * `query` - The query to execute.
    ///
    /// # Return
    ///
    /// A list of records.
    ///
    /// # Example
    ///
    /// Build a service in `services/database.rhai`;
    ///
    /// ```text
    /// // Import the plugin stored in the `plugins` directory.
    /// import "plugins/libvsmtp_plugin_mysql" as mysql;
    ///
    /// export const database = mysql::connect(#{
    ///     // Connect to a database on the system with the 'greylist-manager' user and 'my-password' password.
    ///     url: "mysql://localhost/?user=greylist-manager&password=my-password",
    ///     timeout: "1m",
    ///     connections: 1,
    /// });
    /// ```
    ///
    /// Query the database during filtering.
    ///
    /// ```text
    /// import "services/database" as srv;
    ///
    /// #{
    ///     connect: [
    ///         action "get records from my database" || {
    ///             // For the sake of this example, we assume that there is a populated
    ///             // table called 'my_table' in the database.
    ///             const records = srv::database.query("SELECT * FROM my_table");
    ///
    ///             // `records` is an array, we can run a for loop and print all records.
    ///             log("info", "fetching mysql records ...");
    ///             for record in records {
    ///                 log("info", ` -> ${record}`);
    ///             }
    ///         }
    ///     ],
    /// }
    /// ```
    ///
    /// # rhai-autodocs:index:2
    #[rhai_fn(global, name = "query", return_raw, pure)]
    pub fn query(
        database: &mut MySQL,
        query: &str,
    ) -> Result<rhai::Array, Box<rhai::EvalAltResult>> {
        database
            .query(query)
            .map(|result| result.into_iter().map(rhai::Dynamic::from).collect())
            .map_err(|e| e.to_string().into())
    }
}
