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
    mem, Dynamic, FnAccess, FnNamespace, Module, NativeCallContext, PluginFunction, RhaiResult,
    TypeId,
};

#[derive(Debug, serde::Deserialize)]
struct Args {
    args: Vec<String>,
    user: Option<String>,
    group: Option<String>,
    #[serde(default = "Args::default_timeout", with = "humantime_serde")]
    timeout: std::time::Duration,
}

impl Args {
    const fn default_timeout() -> std::time::Duration {
        std::time::Duration::from_secs(60)
    }
}

#[derive(Debug)]
pub struct ProcessOutput {
    status: std::process::ExitStatus,
    stdout: String,
    stderr: String,
}

fn to_string<T: std::io::Read>(ctx: Option<T>) -> std::io::Result<String> {
    Ok(ctx
        .map(|mut i| {
            let mut buffer = String::new();
            i.read_to_string(&mut buffer)?;
            std::io::Result::Ok(buffer)
        })
        .transpose()?
        .unwrap_or_default())
}

/// Utility functions to interact with the system and execute commands.
///
/// This modules is accessible in filtering AND configuration scripts.
#[rhai::plugin::export_module]
pub mod api {

    /// Run a command and return the result of the execution.
    ///
    /// # Example
    ///
    /// ```js
    /// let result = process::run(#{
    ///     args: ["ls", "-l]
    /// });
    ///
    /// print(result.stdout);
    /// print(result.stderr);
    /// print(result.status);
    /// ```
    ///
    /// # rhai-autodocs:index:1
    #[rhai_fn(global, return_raw)]
    pub fn run(args: rhai::Dynamic) -> Result<ProcessResult, Box<rhai::EvalAltResult>> {
        let Args {
            args,
            user,
            group,
            timeout,
        } = rhai::serde::from_dynamic::<Args>(&args)?;

        let mut command = std::process::Command::new(args.get(0).unwrap());
        command.args(args.iter().skip(1));
        command.stdin(std::process::Stdio::piped());
        command.stdout(std::process::Stdio::piped());
        command.stderr(std::process::Stdio::piped());

        if let Some(user) = user {
            if let Some(user) = uzers::get_user_by_name(&user) {
                std::os::unix::prelude::CommandExt::uid(&mut command, user.uid());
            } else {
                return Err(format!("user not found: '{user}'").into());
            }
        }

        if let Some(group) = group {
            if let Some(group) = uzers::get_group_by_name(&group) {
                std::os::unix::prelude::CommandExt::gid(&mut command, group.gid());
            } else {
                return Err(format!("group not found: '{group}'").into());
            }
        }
        tracing::trace!(?command, "Running command.");

        let mut child = command.spawn().map_err(|e| e.to_string())?;

        let status = match wait_timeout::ChildExt::wait_timeout(&mut child, timeout) {
            Ok(Some(status)) => status,
            Ok(None) => {
                child.kill().map_err(|e| e.to_string())?;
                child.wait().expect("command wasn't running")
            }
            Err(e) => return Err(e.to_string().into()),
        };

        let std::process::Child {
            stdin: _,
            mut stdout,
            mut stderr,
            ..
        } = child;

        Ok(rhai::Shared::new(ProcessOutput {
            status,
            stdout: to_string(stdout.take()).map_err(|e| e.to_string())?,
            stderr: to_string(stderr.take()).map_err(|e| e.to_string())?,
        }))
    }

    /// Result of a command.
    ///
    /// # rhai-autodocs:index:2
    pub type ProcessResult = rhai::Shared<ProcessOutput>;

    /// Get the stderr content of the process result.
    /// # rhai-autodocs:index:3
    #[rhai_fn(global, get = "stderr", pure)]
    pub fn stderr(result: &mut ProcessResult) -> String {
        result.stderr.clone()
    }

    /// Get the stdout content of the process result.
    /// # rhai-autodocs:index:4
    #[rhai_fn(global, get = "stdout", pure)]
    pub fn stdout(result: &mut ProcessResult) -> String {
        result.stdout.clone()
    }

    /// Get the status returned by the command that gave this result.
    /// # rhai-autodocs:index:5
    #[rhai_fn(global, get = "status", pure)]
    pub fn status(result: &mut ProcessResult) -> std::process::ExitStatus {
        result.status
    }

    /// Transform the process result into a debug string.
    /// # rhai-autodocs:index:6
    #[rhai_fn(global, pure)]
    pub fn to_debug(result: &mut ProcessResult) -> String {
        format!("{result:?}",)
    }
}
