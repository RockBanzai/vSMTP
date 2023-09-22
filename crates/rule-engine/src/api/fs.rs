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

use super::Result;
use crate::api::docs::Ctx;
use rhai::plugin::{
    mem, Dynamic, FnAccess, FnNamespace, ImmutableString, Module, NativeCallContext,
    PluginFunction, RhaiResult, TypeId,
};

pub use fs::*;

#[rhai::export_module]
/// APIs to interact with the file system.
mod fs {
    use vsmtp_common::stateful_ctx_received::StateError;

    // TODO: handle canonicalization.
    // TODO: use config store path ?
    /// Export the current raw message to a file as an `eml` file.
    /// The message id of the email is used to name the file.
    ///
    /// # Args
    ///
    /// * `dir` - the directory where to store the email. Relative to the
    /// application path.
    ///
    /// # Effective smtp stage
    ///
    /// `preq` and onwards.
    ///
    /// # Examples
    ///
    /// ```
    /// # let dir = tempfile::tempdir().expect("fs api: failed to create tmpdir");
    /// # let mut config = vsmtp_test::config::local_test();
    /// # config.app.dirpath = dir.path().into();
    /// # let rules = r#"
    /// #{
    ///     preq: [
    ///        action "write to file" || fs::write("archives"),
    ///     ]
    /// }
    /// # "#;
    /// # let states = vsmtp_test::rhai::run_with_msg_and_config(|builder| Ok(builder
    /// #   .add_root_filter_rules("#{}")?
    /// #      .add_domain_rules("testserver.com".parse().unwrap())
    /// #        .with_incoming(rules)?
    /// #        .with_outgoing(rules)?
    /// #        .with_internal(rules)?
    /// #      .build()
    /// #   .build()), None, config);
    /// # eprintln!("{:?}", dir.path());
    /// # assert!(std::path::PathBuf::from_iter([
    /// #     dir.path(),
    /// #     &std::path::Path::new("archives")
    /// # ]).exists());
    /// ```
    ///
    /// # rhai-autodocs:index:1
    #[rhai_fn(return_raw)]
    pub fn write(ctx: &mut Ctx, dir: &str) -> Result<()> {
        ctx.read(|ctx| {
            ctx.get_mail(|mail| {
                let mut dir = std::path::PathBuf::from(dir);
                let message_id = ctx.get_mail_from().map(|mf| mf.message_uuid)?;

                std::fs::create_dir_all(&dir).map_err::<Box<rhai::EvalAltResult>, _>(|err| {
                    format!("failed to write email at {}: {err}", dir.display()).into()
                })?;
                dir.push(format!("{message_id}.eml"));

                std::fs::OpenOptions::new()
                    .create(true)
                    .write(true)
                    .open(&dir)
                    .and_then(|mut file| {
                        std::io::Write::write_all(&mut file, mail.to_string().as_bytes())
                    })
                    .map_err::<Box<rhai::EvalAltResult>, _>(|err| {
                        format!("failed to write email at {}: {err}", dir.display()).into()
                    })
            })
        })
        .map_err::<Box<rhai::EvalAltResult>, _>(StateError::into)?
    }

    // TODO: use config store path ?
    /// Write the content of the current email with it's metadata in a json file.
    /// The message id of the email is used to name the file.
    ///
    /// # Args
    ///
    /// * `dir` - the directory where to store the email. Relative to the
    /// application path.
    ///
    /// # Effective smtp stage
    ///
    /// `connect` and onwards.
    ///
    /// # Examples
    ///
    /// ```
    /// # let dir = tempfile::tempdir().expect("fs api: failed to create tmpdir");
    /// # let mut config = vsmtp_test::config::local_test();
    /// # config.app.dirpath = dir.path().into();
    ///
    /// # let rules = r#"
    /// #{
    ///     preq: [
    ///        action "write to file" || fs::dump("metadata"),
    ///     ]
    /// }
    /// # "#;
    /// # let states = vsmtp_test::rhai::run_with_msg_and_config(|builder| Ok(builder
    /// #   .add_root_filter_rules("#{}")?
    /// #      .add_domain_rules("testserver.com".parse().unwrap())
    /// #        .with_incoming(rules)?
    /// #        .with_outgoing(rules)?
    /// #        .with_internal(rules)?
    /// #      .build()
    /// #   .build()), None, config);
    /// # eprintln!("{:?}", dir.path());
    /// # assert!(std::path::PathBuf::from_iter([
    /// #     dir.path(),
    /// #     &std::path::Path::new("metadata")
    /// # ]).exists());
    /// ```
    ///
    /// # rhai-autodocs:index:2
    #[rhai_fn(return_raw)]
    pub fn dump(ctx: &mut Ctx, dir: &str) -> Result<()> {
        ctx.read(|ctx| {
            let mut dir = std::path::PathBuf::from(dir);
            let message_id = ctx.get_mail_from().map(|mf| mf.message_uuid)?;

            std::fs::create_dir_all(&dir).map_err::<Box<rhai::EvalAltResult>, _>(|err| {
                format!("failed to write email at {}: {err}", dir.display()).into()
            })?;
            dir.push(format!("{message_id}.json"));

            std::fs::OpenOptions::new()
                .create(true)
                .write(true)
                .open(&dir)
                .and_then(|mut file| {
                    std::io::Write::write_all(
                        &mut file,
                        serde_json::to_string_pretty(&ctx)
                            .map_err::<Box<rhai::EvalAltResult>, _>(|err| {
                                format!("failed to dump email at {dir:?}: {err}").into()
                            })
                            .unwrap()
                            .as_bytes(),
                    )
                })
                .map_err::<Box<rhai::EvalAltResult>, _>(|err| {
                    format!("failed to write email at {}: {err}", dir.display()).into()
                })
        })
    }
}
