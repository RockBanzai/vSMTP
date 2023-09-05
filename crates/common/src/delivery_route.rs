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

#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    Hash,
    serde_with::DeserializeFromStr,
    serde_with::SerializeDisplay,
    strum::IntoStaticStr,
)]
#[strum(serialize_all = "snake_case")]
pub enum DeliveryRoute {
    // maildir delivery (IMAP)
    Maildir,
    // mbox delivery (POP3)
    Mbox,
    // delivery to a predefined service over SMTP
    Forward {
        // TODO: must be one word, should not contain dots
        service: String,
    },
    // basic MTA delivery, DNS MX lookup + SMTP
    Basic,
    // extended implementer-defined delivery
    Extern {
        // TODO: must be one word, should not contain dots
        name: String,
    },
}

impl DeliveryRoute {
    #[must_use]
    pub fn matches(&self, other: &Self) -> bool {
        self.matches_sided(other) || other.matches_sided(self)
    }

    fn matches_sided(&self, other: &Self) -> bool {
        if self == other {
            return true;
        }

        if let (
            Self::Extern { name: self_name } | Self::Forward { service: self_name },
            Self::Extern { name: other_name }
            | Self::Forward {
                service: other_name,
            },
        ) = (self, other)
        {
            // TODO: handle the '*'

            if let Some(self_prefix) = self_name.strip_suffix('#') {
                if other_name.strip_prefix(self_prefix).is_some() {
                    return true;
                }
            }
        }

        false
    }
}

impl std::fmt::Display for DeliveryRoute {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self {
            Self::Extern { name } => {
                write!(f, "ext.{name}")
            }
            Self::Forward { service } => {
                write!(f, "forward.{service}")
            }
            otherwise => write!(f, "{}", Into::<&'static str>::into(*otherwise)),
        }
    }
}

pub struct DeliveryRouteParseError;

impl std::fmt::Display for DeliveryRouteParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "invalid delivery route")
    }
}

impl std::str::FromStr for DeliveryRoute {
    type Err = DeliveryRouteParseError;

    #[allow(clippy::option_if_let_else)] // it is more readable this way
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if let Some(name) = s.strip_prefix("ext.") {
            Ok(Self::Extern {
                name: name.to_string(),
            })
        } else if let Some(service) = s.strip_prefix("forward.") {
            Ok(Self::Forward {
                service: service.to_string(),
            })
        } else {
            [Self::Basic, Self::Maildir, Self::Mbox]
                .into_iter()
                .find_map(|i| (s == Into::<&'static str>::into(i.clone())).then_some(i))
                .ok_or(DeliveryRouteParseError)
        }
    }
}
