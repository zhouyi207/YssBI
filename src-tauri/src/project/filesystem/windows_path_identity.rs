use std::ffi::OsStr;
use std::io;
use std::os::windows::ffi::OsStrExt;
use windows_sys::Win32::Globalization::{LCMAP_UPPERCASE, LCMapStringEx, LOCALE_NAME_INVARIANT};

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(super) struct WindowsPathIdentity(Vec<u16>);

impl WindowsPathIdentity {
    pub(super) fn from_os_str(path: &OsStr) -> io::Result<Self> {
        let wide = path.encode_wide().collect::<Vec<_>>();
        let mut identity = Vec::with_capacity(wide.len());
        let mut valid_start = 0;
        let mut index = 0;

        while index < wide.len() {
            let unit = wide[index];
            let valid_pair = (0xd800..=0xdbff).contains(&unit)
                && wide
                    .get(index + 1)
                    .is_some_and(|next| (0xdc00..=0xdfff).contains(next));
            if valid_pair {
                index += 2;
                continue;
            }
            if (0xd800..=0xdfff).contains(&unit) {
                append_uppercase(&wide[valid_start..index], &mut identity)?;
                identity.push(unit);
                index += 1;
                valid_start = index;
                continue;
            }
            index += 1;
        }
        append_uppercase(&wide[valid_start..], &mut identity)?;
        Ok(Self(identity))
    }
}

fn append_uppercase(source: &[u16], destination: &mut Vec<u16>) -> io::Result<()> {
    if source.is_empty() {
        return Ok(());
    }
    let source_len = i32::try_from(source.len())
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "path is too long"))?;

    // Windows' invariant native mapping supplies the same stable, one-platform
    // identity without converting the path through UTF-8.
    let required = unsafe {
        LCMapStringEx(
            LOCALE_NAME_INVARIANT,
            LCMAP_UPPERCASE,
            source.as_ptr(),
            source_len,
            std::ptr::null_mut(),
            0,
            std::ptr::null(),
            std::ptr::null(),
            0,
        )
    };
    if required == 0 {
        return Err(io::Error::last_os_error());
    }

    let offset = destination.len();
    destination.resize(offset + required as usize, 0);
    let written = unsafe {
        LCMapStringEx(
            LOCALE_NAME_INVARIANT,
            LCMAP_UPPERCASE,
            source.as_ptr(),
            source_len,
            destination[offset..].as_mut_ptr(),
            required,
            std::ptr::null(),
            std::ptr::null(),
            0,
        )
    };
    if written == 0 {
        destination.truncate(offset);
        return Err(io::Error::last_os_error());
    }
    destination.truncate(offset + written as usize);
    Ok(())
}
