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

use super::{Action, Status};

#[derive(Debug, serde::Serialize, serde::Deserialize, fake::Dummy)]
pub enum LocalInformation {
    /// scenario: user does not exist
    NotFound,
    /// scenario: program does not have the permission to write in the mailbox
    PermissionDenied,
    /// scenario: the mail with the same uuid already exists in the mailbox
    AlreadyExists,
    /// scenario: writing to a file which has been deleted by another process
    BrokenPipe,
    /// scenario: the operation took too long
    TimedOut,
    /// scenario: could not write all the data (storage is full ?)
    WriteZero,
    /// scenario: could not allocate memory
    OutOfMemory,
    /// error not related to mail storage system, but might happen (really unlikely)
    OtherError(String),
    Success,
}

impl LocalInformation {
    pub(super) const fn get_action(&self) -> Action {
        match self {
            Self::OtherError(_)
            | Self::NotFound
            | Self::PermissionDenied
            | Self::AlreadyExists
            | Self::BrokenPipe => Action::Failed {
                diagnostic_code: None,
            },
            Self::Success => Action::Delivered,
            Self::TimedOut | Self::WriteZero | Self::OutOfMemory => Action::Delayed {
                diagnostic_code: None,
                will_retry_until: None,
            },
        }
    }
}

impl From<&LocalInformation> for Status {
    fn from(value: &LocalInformation) -> Self {
        match value {
            LocalInformation::NotFound => Self("5.1.1".to_string()),
            LocalInformation::PermissionDenied
            | LocalInformation::AlreadyExists
            | LocalInformation::BrokenPipe => Self("5.0.0".to_string()),
            LocalInformation::TimedOut => Self("4.4.7".to_string()),
            LocalInformation::WriteZero => Self("4.3.1".to_string()),
            LocalInformation::OutOfMemory => Self("4.3.0".to_string()),
            LocalInformation::OtherError(_) => Self("5.3.0".to_string()),
            LocalInformation::Success => Self("2.0.0".to_owned()),
        }
    }
}

impl From<std::io::Error> for LocalInformation {
    fn from(value: std::io::Error) -> Self {
        match value.kind() {
            std::io::ErrorKind::NotFound => Self::NotFound,
            std::io::ErrorKind::PermissionDenied => Self::PermissionDenied,
            std::io::ErrorKind::BrokenPipe => Self::BrokenPipe,
            std::io::ErrorKind::AlreadyExists => Self::AlreadyExists,
            std::io::ErrorKind::TimedOut => Self::TimedOut,
            std::io::ErrorKind::WriteZero => Self::WriteZero,
            std::io::ErrorKind::OutOfMemory => Self::OutOfMemory,

            std::io::ErrorKind::UnexpectedEof
            | std::io::ErrorKind::Interrupted
            | std::io::ErrorKind::InvalidData
            | std::io::ErrorKind::InvalidInput
            | std::io::ErrorKind::Unsupported
            | std::io::ErrorKind::Other => {
                tracing::trace!("Other error: {}", value);
                Self::OtherError(value.to_string())
            }

            std::io::ErrorKind::ConnectionRefused
            | std::io::ErrorKind::WouldBlock
            | std::io::ErrorKind::ConnectionReset
            | std::io::ErrorKind::ConnectionAborted
            | std::io::ErrorKind::NotConnected
            | std::io::ErrorKind::AddrInUse
            | std::io::ErrorKind::AddrNotAvailable => {
                unreachable!(
                    "Assuming network io error will not happen in \
                    the filesystem logics"
                )
            }

            // Unstable error kinds
            //std::io::ErrorKind::HostUnreachable => unimplemented!(),
            //std::io::ErrorKind::NetworkUnreachable => unimplemented!(),
            //std::io::ErrorKind::NetworkDown => unimplemented!(),
            //std::io::ErrorKind::NotADirectory => unimplemented!(),
            //std::io::ErrorKind::IsADirectory => unimplemented!(),
            //std::io::ErrorKind::DirectoryNotEmpty => unimplemented!(),
            //std::io::ErrorKind::ReadOnlyFilesystem => unimplemented!(),
            //std::io::ErrorKind::FilesystemLoop => unimplemented!(),
            //std::io::ErrorKind::StaleNetworkFileHandle => unimplemented!(),
            //std::io::ErrorKind::StorageFull => unimplemented!(),
            //std::io::ErrorKind::NotSeekable => unimplemented!(),
            //std::io::ErrorKind::FilesystemQuotaExceeded => unimplemented!(),
            //std::io::ErrorKind::FileTooLarge => unimplemented!(),
            //std::io::ErrorKind::ResourceBusy => unimplemented!(),
            //std::io::ErrorKind::ExecutableFileBusy => unimplemented!(),
            //std::io::ErrorKind::Deadlock => unimplemented!(),
            //std::io::ErrorKind::CrossesDevices => unimplemented!(),
            //std::io::ErrorKind::TooManyLinks => unimplemented!(),
            //std::io::ErrorKind::InvalidFilename => unimplemented!(),
            //std::io::ErrorKind::ArgumentListTooLong => unimplemented!(),
            //std::io::ErrorKind::Uncategorized => unimplemented!(),

            // non exhaustive
            _ => unimplemented!(),
        }
    }
}
