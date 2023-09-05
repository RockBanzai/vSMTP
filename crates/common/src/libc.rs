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

/// Change ownership of a file
///
/// # Errors
///
/// * `@path` cannot be convert to `CString`
/// * see `chown(2)` ERRORS
// NOTE: should use https://docs.rs/rustix/latest/rustix/fs/fn.fchown.html
#[inline]
pub fn chown(path: &std::path::Path, user: Option<u32>, group: Option<u32>) -> std::io::Result<()> {
    let path = std::ffi::CString::new(path.to_string_lossy().as_bytes())?;
    #[allow(unsafe_code)]
    // SAFETY: ffi call
    match unsafe {
        libc::chown(
            path.as_ptr(),
            user.unwrap_or(u32::MAX),
            group.unwrap_or(u32::MAX),
        )
    } {
        0i32 => Ok(()),
        _ => Err(std::io::Error::last_os_error()),
    }
}

/// Get user's home directory
///
/// # Errors
///
/// * see `getpwuid(2)` ERRORS
/// * the file path does not contain valid utf8 data
#[inline]
pub fn getpwuid(uid: libc::uid_t) -> std::io::Result<std::path::PathBuf> {
    #[allow(unsafe_code)]
    // SAFETY: ffi call
    let passwd = unsafe { libc::getpwuid(uid) };
    #[allow(unsafe_code)]
    // SAFETY: `passwd` is a valid pointer
    if passwd.is_null() || unsafe { *passwd }.pw_dir.is_null() {
        return Err(std::io::Error::last_os_error());
    }
    #[allow(unsafe_code)]
    // SAFETY: pointer is not null
    let buffer = unsafe { *passwd }.pw_dir;
    #[allow(unsafe_code)]
    // SAFETY: the foreign allocated is used correctly as specified in `CStr::from_ptr`
    Ok(unsafe { std::ffi::CStr::from_ptr(buffer) }
        .to_str()
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?
        .into())
}
