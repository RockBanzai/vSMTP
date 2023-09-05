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

use r2d2::{self, ManageConnection, PooledConnection};
use rhai::{
    plugin::{mem, Dynamic, FnNamespace, NativeCallContext, PluginFunction, RhaiResult, TypeId},
    FnAccess, Module,
};
use std::io::prelude::*;
use thiserror::Error;
use vsmtp_antivirus::Antivirus;
use vsmtp_antivirus::RhaiAntivirus;
use vsmtp_common::stateful_ctx_received::StatefulCtxReceived;
use vsmtp_rule_engine::api::State;

/// Footer of a INSTREAM command
const FOOTER: &[u8] = &[0; 4];
/// IDSESSION command
const SESSION_CMD: &[u8] = b"zIDSESSION\0";
/// INSTREAM command
const INSTREAM_CMD: &[u8] = b"zINSTREAM\0";
/// Token to find in a scan result (done here via INSTREAM)
const SAFE_TOKEN: &[u8] = b"OK";
/// PING command
const PING_CMD: &[u8] = b"zPING\0";
/// PING command expected answer
const PING_ANSWER: &[u8] = b"PONG";

/// parameter used to configure `Plugin`
#[derive(serde::Serialize, serde::Deserialize)]
pub struct Parameters {
    /// url of clamav service
    url: String,
    /// max number of connection at the same time
    #[serde(default = "Parameters::default_max_connections")]
    max_connections: u32,
}

impl Parameters {
    const fn default_max_connections() -> u32 {
        4
    }
}

/// Plugin managing connections to `ClamAV` service (aka. clamd)
#[derive(Clone)]
pub struct Plugin {
    /// connection pool
    pool: r2d2::Pool<ClamavConnector>,
}

impl Antivirus for Plugin {
    /// Scan a chunk of data for viruses.
    /// It is done via the command INSTREAM of clamd daemon.
    ///
    /// # Args
    ///
    /// * `raw_data` - the data to scan in bytes
    ///
    /// # Return
    ///
    /// True if `raw_data` is contaminated, false otherwise.
    fn scan(&self, raw_data: &[u8]) -> Result<bool, std::io::Error> {
        let mut stream = self.pool.get().map_err(|_err| {
            std::io::Error::new(
                std::io::ErrorKind::Other,
                "failed to retrieve a clamav connection",
            )
        })?;
        let scan_res = Self::inner_scan(&mut stream, raw_data);
        if scan_res.is_err() {
            let error = scan_res.as_ref().err().unwrap();
            stream.last_error = Some(std::io::Error::new(error.kind(), error.to_string()));
        }
        scan_res
    }
}

impl Plugin {
    /// internal implementation of scan function
    /// this was done to catch any IO error to be able to now if the connection is broken?
    ///
    /// # Args
    ///
    /// * `conn` - the current connection
    /// * `raw_data` - the data to scan in bytes
    ///
    /// # Return
    ///
    /// True if `raw_data` is contaminated, false otherwise.
    fn inner_scan(
        conn: &mut PooledConnection<ClamavConnector>,
        raw_data: &[u8],
    ) -> Result<bool, std::io::Error> {
        conn.stream.write_all(INSTREAM_CMD)?;
        for data_chunk in raw_data.chunks(u32::MAX as usize) {
            #[allow(clippy::cast_possible_truncation)]
            let size = data_chunk.len() as u32;
            let msg = [size.to_be_bytes().as_slice(), data_chunk].concat();
            conn.stream.write_all(&msg)?;
        }
        conn.stream.write_all(FOOTER)?;
        let full_buf: Vec<u8> = read_all_buffer(&mut conn.stream, 64)?;
        Ok(!full_buf
            .windows(SAFE_TOKEN.len())
            .any(|window| window == SAFE_TOKEN))
    }
}

/// Encapsulate a `TcpStream` with the last IO error encounter
/// This is used to detect error in `has_broken` method of r2d2
struct TcpStreamWrapper {
    /// Inner TCP stream.
    stream: std::net::TcpStream,
    /// Stored case of broken connection.
    last_error: Option<std::io::Error>,
}

/// error that can happens in clamav plugin
#[derive(Error, Debug)]
enum ClamavPluginError {
    /// Ping command succeed but did not received a pong response
    #[error("No pong answer received")]
    NoPongError(),
    /// any io error (mostly from socket communication)
    #[error("{0}")]
    IOError(#[from] std::io::Error),
}

/// r2d2 connector
struct ClamavConnector {
    // FIXME: by default, this cannot resolve domain name.
    //        Thus, you cannot connect to clamd at `example.com`,
    //        for example.
    /// Address to clamd.
    address: std::net::SocketAddr,
}

impl ManageConnection for ClamavConnector {
    type Connection = TcpStreamWrapper;
    type Error = ClamavPluginError;

    fn connect(&self) -> Result<Self::Connection, Self::Error> {
        let mut plugin = TcpStreamWrapper {
            stream: std::net::TcpStream::connect(self.address)?,
            last_error: None,
        };
        plugin.stream.write_all(SESSION_CMD)?;
        Ok(plugin)
    }

    fn has_broken(&self, conn: &mut Self::Connection) -> bool {
        conn.last_error.is_some()
    }

    fn is_valid(&self, conn: &mut Self::Connection) -> Result<(), Self::Error> {
        conn.stream.write_all(PING_CMD)?;
        let full_buf: Vec<u8> = read_all_buffer(&mut conn.stream, 12)?;
        if full_buf
            .windows(PING_ANSWER.len())
            .any(|window| window == PING_ANSWER)
        {
            Ok(())
        } else {
            Err(ClamavPluginError::NoPongError())
        }
    }
}

/// Utility function that read a socket buffer until `\0`
///
/// # Args
///
/// * `stream` - the tcp stream to read from.
/// * `buffer_size` - size of the sub-buffer used to fill the final buffer.
///
/// # Return
///
/// * `Result<Vec<u8>, std::io::Error>` - The full buffer, or any IO error encounter during the process
///
fn read_all_buffer(
    stream: &mut std::net::TcpStream,
    buffer_size: usize,
) -> Result<Vec<u8>, std::io::Error> {
    let mut full_buf: Vec<u8> = Vec::new();
    loop {
        let mut buf = vec![0u8; buffer_size];
        let _ = stream.read(&mut buf)?;
        full_buf.append(&mut buf.clone());
        if buf.contains(&b'\0') {
            break;
        }
    }
    Ok(full_buf)
}

#[rhai::plugin::export_module]
pub mod clamav {
    use vsmtp_common::stateful_ctx_received::StateError;

    /// Connect to clamd.
    ///
    /// # Parameters
    ///
    /// a map composed of the following parameters:
    /// `url`         (default: 4)     - url to the clamd instance.
    /// `max_connections` (default: 4) - Number of simultaneous opened connections to clamd. (default: 4)
    ///
    /// # Effective SMTP stages
    ///
    /// from `pre_queue`.
    ///
    /// # Return
    ///
    /// A clamd service object.
    ///
    /// # Errors
    ///
    /// If the service cannot connect to clamd or if the parameters are incorrect,
    /// this function will throw an error.
    ///
    /// # Examples
    ///
    /// ```js
    /// // Import the plugin.
    /// import "plugins/libvsmtp_clamav_plugin" as clamav;
    ///
    /// // Create a connection to clamd.
    /// export const bridge = clamav::connect(#{
    ///     url: "tcp://clamav:3310",
    /// });
    /// ```
    ///
    /// # rhai-autodocs:index:1
    #[rhai_fn(global, return_raw)]
    pub fn connect(params: rhai::Map) -> Result<RhaiAntivirus, Box<rhai::EvalAltResult>> {
        let params = rhai::serde::from_dynamic::<Parameters>(&params.into())?;
        let manager = ClamavConnector {
            address: {
                let mut sockets = url::Url::parse(&params.url)
                    .map_err::<Box<rhai::EvalAltResult>, _>(|err| err.to_string().into())?
                    .socket_addrs(|| None)
                    .map_err::<Box<rhai::EvalAltResult>, _>(|err| err.to_string().into())?;

                sockets
                    .pop()
                    .ok_or_else::<Box<rhai::EvalAltResult>, _>(|| {
                        format!("No ip resolved from '{}'", params.url).into()
                    })?
            },
        };

        Ok(RhaiAntivirus(std::sync::Arc::new(Plugin {
            pool: r2d2::Pool::builder()
                .max_size(params.max_connections)
                .idle_timeout(Some(std::time::Duration::from_secs(30)))
                .max_lifetime(Some(std::time::Duration::from_secs(30)))
                .min_idle(Some(1))
                .build(manager)
                .map_err(|err| err.to_string())?,
        })))
    }

    /// Scan an email for viruses.
    ///
    /// # Parameters
    ///
    /// `mail` - The mail to scan.
    ///
    /// # Effective SMTP stages
    ///
    /// from `pre_queue`.
    ///
    /// # Return
    ///
    /// A ```boolean```, true if the mail contains a virus, false otherwise.
    ///
    /// # Errors
    ///
    /// If clamd returns an error or the connection hung up, this function
    /// will throw an error.
    ///
    /// # Examples
    ///
    /// An example for the working service:
    ///
    /// ```js
    /// import "services/antivirus.rhai" as antivirus;
    ///
    /// fn on_post_queue(ctx) {
    ///     if antivirus::bridge.scan(ctx) {
    ///         // Store the email in quarantine if a virus is found.
    ///         status::quarantine("virus")
    ///     } else {
    ///         // Good to deliver!
    ///         status::success()
    ///     }
    /// }
    /// ```
    ///
    /// # rhai-autodocs:index:2
    #[rhai_fn(global, pure, return_raw)]
    pub fn scan(
        plugin: &mut RhaiAntivirus,
        // NOTE(ltabis): Only the email is used in this method, but
        // I decided to pass the whole context anyways in case we need additional
        // data from it later.
        ctx: State<StatefulCtxReceived>,
    ) -> Result<bool, Box<rhai::EvalAltResult>> {
        ctx.read(|ctx| {
            ctx.get_mail(ToString::to_string)
                .map(|mail| plugin.0.scan(mail.as_bytes()))
        })
        .map_err::<Box<rhai::EvalAltResult>, _>(StateError::into)?
        .map_err(|err| err.to_string().into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{IpAddr, Ipv4Addr, SocketAddr};
    use std::{thread, time::Duration};

    #[test]
    fn basic_scan() -> Result<(), String> {
        let manager = ClamavConnector {
            address: SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 13310),
        };

        let plugin = Plugin {
            pool: r2d2::Pool::builder()
                .idle_timeout(Some(std::time::Duration::from_secs(1)))
                .max_lifetime(Some(std::time::Duration::from_secs(1)))
                .max_size(1)
                .build(manager)
                .map_err(|err| err.to_string())?,
        };
        let result = plugin.scan(b"this is a test");
        assert!(result.unwrap());
        let result =
            plugin.scan(b"X5O!P%@AP[4\\PZX54(P^)7CC)7}$EICAR-STANDARD-ANTIVIRUS-TEST-FILE!$H+H*");
        assert!(!result.unwrap());
        Ok(())
    }

    #[ignore]
    #[test]
    fn reconnection() -> Result<(), String> {
        const CLAMAV_SERVICE_ID: &str = "c47844407ebd"; // id of the docker container // TODO: programmatically find it
        let manager = ClamavConnector {
            address: SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 13310),
        };

        let plugin = Plugin {
            pool: r2d2::Pool::builder()
                .idle_timeout(Some(std::time::Duration::from_secs(1)))
                .max_lifetime(Some(std::time::Duration::from_secs(1)))
                .max_size(1)
                .build(manager)
                .map_err(|err| err.to_string())?,
        };
        let result = plugin.scan(b"this is a test");
        assert!(result.unwrap());
        let docker_restart = std::process::Command::new("docker")
            .args(["restart", CLAMAV_SERVICE_ID])
            .status();
        if docker_restart.is_ok() {
            // wait more for clamav service to fully restart
            thread::sleep(Duration::from_secs(5));
            let result = plugin.scan(b"this is a test");
            assert!(result.unwrap());
        } else {
            println!("{}", docker_restart.err().unwrap());
            panic!("Failed to restart clamav docker instance");
        }
        Ok(())
    }
}
