use std::{io, path::Path};

/// Moves a file or folder to the Windows Recycle Bin so it can be restored.
#[cfg(windows)]
pub fn move_to_recycle_bin(path: &Path) -> io::Result<()> {
    use std::os::windows::ffi::OsStrExt;
    use windows::{
        core::PCWSTR,
        Win32::UI::Shell::{
            SHFileOperationW, FOF_ALLOWUNDO, FOF_NOCONFIRMATION, FOF_NOERRORUI, FOF_SILENT,
            FO_DELETE, SHFILEOPSTRUCTW,
        },
    };

    let absolute = std::path::absolute(path)?;
    let mut source: Vec<u16> = absolute.as_os_str().encode_wide().collect();
    source.extend([0, 0]);
    let mut operation = SHFILEOPSTRUCTW {
        wFunc: FO_DELETE,
        pFrom: PCWSTR::from_raw(source.as_ptr()),
        fFlags: (FOF_ALLOWUNDO.0 | FOF_NOCONFIRMATION.0 | FOF_NOERRORUI.0 | FOF_SILENT.0) as u16,
        ..Default::default()
    };
    let code = unsafe { SHFileOperationW(&mut operation) };
    if code != 0 || operation.fAnyOperationsAborted.as_bool() {
        return Err(io::Error::other(format!(
            "Could not move the item to the Recycle Bin (code {code})"
        )));
    }
    Ok(())
}

#[cfg(not(windows))]
pub fn move_to_recycle_bin(_path: &Path) -> io::Result<()> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "The Recycle Bin is only available on Windows",
    ))
}
