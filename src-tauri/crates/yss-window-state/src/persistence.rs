use crate::WindowStateError;
use serde::Serialize;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

pub(super) fn write_json_atomically<T>(
    destination: &Path,
    snapshot: &T,
) -> Result<(), WindowStateError>
where
    T: Serialize,
{
    let parent = destination
        .parent()
        .ok_or(WindowStateError::MissingParentDirectory)?;
    fs::create_dir_all(parent).map_err(WindowStateError::CreateDirectory)?;
    let json = serde_json::to_vec_pretty(snapshot).map_err(WindowStateError::Serialize)?;
    let (temporary_path, mut temporary_file) =
        reserve_temporary_file(destination).map_err(WindowStateError::ReserveTemporaryFile)?;

    let write_result = temporary_file
        .write_all(&json)
        .and_then(|()| temporary_file.sync_all());
    drop(temporary_file);
    let persist_result = write_result.and_then(|()| atomic_replace(&temporary_path, destination));
    if let Err(source) = persist_result {
        return match fs::remove_file(&temporary_path) {
            Ok(()) => Err(WindowStateError::Persist(source)),
            Err(cleanup) => Err(WindowStateError::PersistAndCleanup { source, cleanup }),
        };
    }
    Ok(())
}

fn reserve_temporary_file(destination: &Path) -> io::Result<(PathBuf, fs::File)> {
    let parent = destination.parent().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "window state path has no parent directory",
        )
    })?;
    let file_name = destination.file_name().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "window state path has no file name",
        )
    })?;

    for _ in 0..8 {
        let temporary_path = parent.join(format!(
            ".{}.{}.tmp",
            file_name.to_string_lossy(),
            uuid::Uuid::new_v4()
        ));
        match fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary_path)
        {
            Ok(file) => return Ok((temporary_path, file)),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error),
        }
    }

    Err(io::Error::new(
        io::ErrorKind::AlreadyExists,
        "unable to reserve a unique window state temporary file",
    ))
}

#[cfg(not(windows))]
fn atomic_replace(temporary: &Path, destination: &Path) -> io::Result<()> {
    fs::rename(temporary, destination)
}

#[cfg(windows)]
fn atomic_replace(temporary: &Path, destination: &Path) -> io::Result<()> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::{
        MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH, MoveFileExW,
    };

    let source = temporary
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect::<Vec<_>>();
    let target = destination
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect::<Vec<_>>();
    let replaced = unsafe {
        MoveFileExW(
            source.as_ptr(),
            target.as_ptr(),
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
        )
    };
    if replaced == 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}
