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

// FIXME: Probably useless to expose those configurations. The user will probably never touch them.
//        Are there any use cases where the user would want to change those ?
//        e. g. setup multiple quarantine queues specific to some processes ?
#[derive(Debug, serde::Deserialize, serde::Serialize, PartialEq, Eq)]
pub struct Queues {
    // NOTE: those fields will probably never be touched by the user.
    /// Name of the quarantine queue.
    #[serde(default = "Queues::default_quarantine")]
    pub quarantine: String,
    /// Name of the no-route queue.
    #[serde(default = "Queues::default_no_route")]
    pub no_route: String,
    /// Name of the dead queue.
    #[serde(default = "Queues::default_dead")]
    pub dead: String,
    /// Name of the next queue in the mail pipeline.
    /// If set to None, the email is at the end of the pipeline:
    /// It has been sent or delivered locally.
    #[serde(default = "Queues::default_submit")]
    pub submit: Option<String>,
}

impl Queues {
    fn default_quarantine() -> String {
        "quarantine".to_string()
    }

    fn default_no_route() -> String {
        "no-route".to_string()
    }

    fn default_dead() -> String {
        "dead".to_string()
    }

    const fn default_submit() -> Option<String> {
        None
    }
}

impl Default for Queues {
    fn default() -> Self {
        Self {
            quarantine: Self::default_quarantine(),
            no_route: Self::default_no_route(),
            dead: Self::default_dead(),
            submit: Self::default_submit(),
        }
    }
}
