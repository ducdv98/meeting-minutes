use std::{io, path::Path};

use windows::{
    core::{PCWSTR, PWSTR},
    Win32::{
        Foundation::{
            ERROR_FILE_NOT_FOUND, ERROR_INSUFFICIENT_BUFFER, ERROR_NO_MORE_ITEMS, ERROR_SUCCESS,
        },
        Storage::Packaging::Appx::GetCurrentPackageFamilyName,
        System::Registry::{
            RegCloseKey, RegEnumKeyExW, RegOpenKeyExW, RegQueryValueExW, HKEY, HKEY_CURRENT_USER,
            KEY_READ, REG_QWORD, REG_VALUE_TYPE,
        },
    },
};

use super::MicrophoneApp;

const CONSENT_STORE: &str =
    "Software\\Microsoft\\Windows\\CurrentVersion\\CapabilityAccessManager\\ConsentStore\\microphone";

struct RegistryKey(HKEY);

impl Drop for RegistryKey {
    fn drop(&mut self) {
        let _ = unsafe { RegCloseKey(self.0) };
    }
}

fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(Some(0)).collect()
}

fn open(parent: HKEY, name: &str) -> io::Result<Option<RegistryKey>> {
    let name = wide(name);
    let mut key = HKEY::default();
    let result = unsafe {
        RegOpenKeyExW(
            parent,
            PCWSTR::from_raw(name.as_ptr()),
            None,
            KEY_READ,
            &mut key,
        )
    };
    if result == ERROR_FILE_NOT_FOUND {
        return Ok(None);
    }
    if result != ERROR_SUCCESS {
        return Err(io::Error::from_raw_os_error(result.0 as i32));
    }
    Ok(Some(RegistryKey(key)))
}

fn subkeys(key: HKEY) -> io::Result<Vec<String>> {
    let mut names = Vec::new();
    let mut index = 0;
    loop {
        // Windows registry key names are limited to 255 UTF-16 code units.
        let mut name = [0_u16; 256];
        let mut length = name.len() as u32;
        let result = unsafe {
            RegEnumKeyExW(
                key,
                index,
                Some(PWSTR::from_raw(name.as_mut_ptr())),
                &mut length,
                None,
                None,
                None,
                None,
            )
        };
        if result == ERROR_NO_MORE_ITEMS {
            break;
        }
        if result != ERROR_SUCCESS {
            return Err(io::Error::from_raw_os_error(result.0 as i32));
        }
        names.push(String::from_utf16_lossy(&name[..length as usize]));
        index += 1;
    }
    Ok(names)
}

fn qword(key: HKEY, name: &str) -> Option<u64> {
    let name = wide(name);
    let mut bytes = [0_u8; 8];
    let mut length = bytes.len() as u32;
    let mut kind = REG_VALUE_TYPE::default();
    let result = unsafe {
        RegQueryValueExW(
            key,
            PCWSTR::from_raw(name.as_ptr()),
            None,
            Some(&mut kind),
            Some(bytes.as_mut_ptr()),
            Some(&mut length),
        )
    };
    (result == ERROR_SUCCESS && kind == REG_QWORD && length == 8).then(|| u64::from_le_bytes(bytes))
}

fn in_use(key: HKEY) -> bool {
    // Start must be nonzero: Windows may create a consent entry before first use.
    qword(key, "LastUsedTimeStart").is_some_and(|start| start != 0)
        && qword(key, "LastUsedTimeStop") == Some(0)
}

fn own_package_family() -> Option<String> {
    let mut length = 0;
    if unsafe { GetCurrentPackageFamilyName(&mut length, None) } != ERROR_INSUFFICIENT_BUFFER {
        return None;
    }
    let mut name = vec![0_u16; length as usize];
    if unsafe { GetCurrentPackageFamilyName(&mut length, Some(PWSTR::from_raw(name.as_mut_ptr()))) }
        != ERROR_SUCCESS
    {
        return None;
    }
    Some(String::from_utf16_lossy(&name[..length as usize - 1]).to_lowercase())
}

fn own_executable_key() -> Option<String> {
    std::env::current_exe().ok().map(|path| {
        path.to_string_lossy()
            .trim_start_matches(r"\\?\")
            .replace(['\\', '/'], "#")
            .to_lowercase()
    })
}

pub(super) fn apps_using_microphone() -> io::Result<Vec<MicrophoneApp>> {
    let Some(root) = open(HKEY_CURRENT_USER, CONSENT_STORE)? else {
        return Ok(Vec::new());
    };
    let own_package = own_package_family();
    let own_executable = own_executable_key();
    let mut apps = Vec::new();

    for name in subkeys(root.0)? {
        if name.eq_ignore_ascii_case("NonPackaged") {
            continue;
        }
        let Some(key) = open(root.0, &name)? else {
            continue;
        };
        if !in_use(key.0) || own_package.as_deref() == Some(&name.to_lowercase()) {
            continue;
        }
        let package_name = name.split('_').next().unwrap_or(&name);
        apps.push(MicrophoneApp {
            id: format!("package:{}", name.to_lowercase()),
            display_name: package_name.to_string(),
        });
    }

    if let Some(desktop) = open(root.0, "NonPackaged")? {
        for name in subkeys(desktop.0)? {
            let Some(key) = open(desktop.0, &name)? else {
                continue;
            };
            let id = name.to_lowercase();
            if !in_use(key.0) || own_executable.as_deref() == Some(&id) {
                continue;
            }
            let filename = name.rsplit('#').next().unwrap_or(&name);
            let display_name = Path::new(filename)
                .file_stem()
                .and_then(|stem| stem.to_str())
                .unwrap_or(filename)
                .to_string();
            apps.push(MicrophoneApp {
                id: format!("desktop:{id}"),
                display_name,
            });
        }
    }
    apps.sort_by(|left, right| left.id.cmp(&right.id));
    Ok(apps)
}
