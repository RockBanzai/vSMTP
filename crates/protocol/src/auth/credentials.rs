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

use super::Mechanism;

/// The credentials send by the client, not necessarily the right one
#[derive(Clone, PartialEq, Eq, strum::Display, serde::Deserialize)]
#[strum(serialize_all = "PascalCase")]
#[cfg_attr(debug_assertions, derive(Debug, serde::Serialize))]
pub enum Credentials {
    /// the pair will be sent and verified by a third party
    Verify {
        ///
        authid: String,
        ///
        authpass: String,
    },
    /// verify the token send by anonymous mechanism
    AnonymousToken {
        /// [ email / 1*255TCHAR ]
        token: String,
    },
}

#[cfg(not(debug_assertions))]
impl std::fmt::Debug for Credentials {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Credentials::Verify { authid, .. } => f
                .debug_struct("Credentials::Verify")
                .field("authid", authid)
                .field("authpass", &"***")
                .finish(),
            Credentials::AnonymousToken { .. } => f
                .debug_struct("Credentials::AnonymousToken")
                .field("token", &"***")
                .finish(),
        }
    }
}

#[cfg(not(debug_assertions))]
impl serde::Serialize for Credentials {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStructVariant;

        match self {
            Credentials::Verify { .. } => {
                let mut s = serializer.serialize_struct_variant("Credentials", 0, "Verify", 2)?;
                s.serialize_field("authid", "***")?;
                s.serialize_field("authpass", "***")?;
                s.end()
            }
            Credentials::AnonymousToken { .. } => {
                let mut s =
                    serializer.serialize_struct_variant("Credentials", 1, "AnonymousToken", 1)?;
                s.serialize_field("token", "***")?;
                s.end()
            }
        }
    }
}

#[doc(hidden)]
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("field is missing")]
    MissingField,
    #[error("cannot parse utf8")]
    Utf8(std::str::Utf8Error),
    #[error("mechanism not implemented")]
    Unimplemented,
}

impl TryFrom<(&rsasl::callback::SessionData, &rsasl::callback::Context<'_>)> for Credentials {
    type Error = Error;

    #[inline]
    fn try_from(
        value: (&rsasl::callback::SessionData, &rsasl::callback::Context<'_>),
    ) -> Result<Self, Self::Error> {
        let (session_data, context) = value;

        match session_data.mechanism().mechanism {
            mech if mech == Mechanism::Plain.as_ref() || mech == Mechanism::Login.as_ref() => {
                Ok(Self::Verify {
                    authid: context
                        .get_ref::<rsasl::property::AuthId>()
                        .ok_or(Error::MissingField)?
                        .to_owned(),
                    authpass: std::str::from_utf8(
                        context
                            .get_ref::<rsasl::property::Password>()
                            .ok_or(Error::MissingField)?,
                    )
                    .map_err(Error::Utf8)?
                    .to_owned(),
                })
            }
            mech if mech == Mechanism::Anonymous.as_ref() => Ok(Self::AnonymousToken {
                token: context
                    .get_ref::<rsasl::mechanisms::anonymous::AnonymousToken>()
                    .ok_or(Error::MissingField)?
                    .to_owned(),
            }),
            // mech if mech == Mechanism::CramMd5.as_ref() => todo!(),
            _ => Err(Error::Unimplemented),
        }
    }
}
