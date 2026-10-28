//! Operating-system details: local file times, hidden/system attributes, USB flash drives and
//! the Desktop folder. Windows functions come from kernel32 only (already linked by std).

use std::fs::Metadata;
use std::path::PathBuf;

/// FILETIME (100 ns ticks since 1601-01-01 UTC) to a local TDateTime, like the original's
/// `FileTimeToDateTime` (UTC + the current local bias). 0 stays 0 ("no date").
#[cfg(windows)]
pub fn filetime_to_local(ft: u64) -> f64 {
    if ft == 0 {
        return 0.0;
    }
    let utc = win::FileTime { low: ft as u32, high: (ft >> 32) as u32 };
    let mut local = win::FileTime { low: 0, high: 0 };
    // SAFETY: both pointers refer to valid FILETIME structs for the duration of the call.
    let ok = unsafe { win::FileTimeToLocalFileTime(&utc, &mut local) } != 0;
    let ticks = if ok { (u64::from(local.high) << 32) | u64::from(local.low) } else { ft };
    // 109 205 days from 1601-01-01 to 1899-12-30; 864e9 ticks per day.
    ticks as f64 / 864e9 - 109_205.0
}

/// Hidden or System entries are skipped by the folder scan (FL-004).
pub fn is_hidden_or_system(meta: &Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        meta.file_attributes() & (0x2 | 0x4) != 0
    }
    #[cfg(not(windows))]
    {
        let _ = meta;
        false
    }
}

/// Directory test that also follows junctions/symlinks reported by the directory listing.
pub fn is_directory(meta: &Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        meta.is_dir() || meta.file_attributes() & 0x10 != 0
    }
    #[cfg(not(windows))]
    {
        meta.is_dir()
    }
}

/// The user's Desktop folder(s): auto-search skips queued folders inside them (FL-003).
/// Approximation of the shell's CSIDL_DESKTOPDIRECTORY without linking shell32:
/// `%USERPROFILE%\Desktop` and, when OneDrive redirects it, `%OneDrive%\Desktop`.
pub fn desktop_dirs() -> Vec<PathBuf> {
    ["USERPROFILE", "OneDrive"]
        .iter()
        .filter_map(std::env::var_os)
        .filter(|v| !v.is_empty())
        .map(|v| PathBuf::from(v).join("Desktop"))
        .collect()
}

/// Drive roots ("X:\") of valid USB flash drives: removable type, USB bus, ready and non-zero
/// size (`IsValidUSBFlashDrive`).
pub fn usb_flash_drives() -> Vec<String> {
    #[cfg(windows)]
    {
        win::usb_flash_drives()
    }
    #[cfg(not(windows))]
    {
        Vec::new()
    }
}

#[cfg(windows)]
mod win {
    use std::ffi::c_void;

    #[repr(C)]
    pub struct FileTime {
        pub low: u32,
        pub high: u32,
    }

    #[repr(C)]
    struct StoragePropertyQuery {
        property_id: u32,
        query_type: u32,
        additional: [u8; 4],
    }

    const DRIVE_REMOVABLE: u32 = 2;
    const FILE_SHARE_READ_WRITE: u32 = 3;
    const OPEN_EXISTING: u32 = 3;
    const IOCTL_STORAGE_QUERY_PROPERTY: u32 = 0x002D_1400;
    const BUS_TYPE_USB: u32 = 7;
    /// Offset of `BusType` in STORAGE_DEVICE_DESCRIPTOR.
    const BUS_TYPE_OFFSET: usize = 28;
    const SEM_FAILCRITICALERRORS: u32 = 1;

    #[link(name = "kernel32")]
    unsafe extern "system" {
        pub fn FileTimeToLocalFileTime(ft: *const FileTime, local: *mut FileTime) -> i32;
        fn GetLogicalDrives() -> u32;
        fn GetDriveTypeW(root: *const u16) -> u32;
        fn CreateFileW(
            name: *const u16,
            access: u32,
            share: u32,
            security: *mut c_void,
            disposition: u32,
            flags: u32,
            template: *mut c_void,
        ) -> *mut c_void;
        fn DeviceIoControl(
            handle: *mut c_void,
            code: u32,
            input: *const c_void,
            input_size: u32,
            output: *mut c_void,
            output_size: u32,
            returned: *mut u32,
            overlapped: *mut c_void,
        ) -> i32;
        fn CloseHandle(handle: *mut c_void) -> i32;
        fn GetDiskFreeSpaceExW(root: *const u16, avail: *mut u64, total: *mut u64, free: *mut u64) -> i32;
        fn SetThreadErrorMode(mode: u32, old: *mut u32) -> i32;
    }

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    fn is_usb_bus(letter: char) -> bool {
        let device = wide(&format!("\\\\.\\{letter}:"));
        // SAFETY: valid NUL-terminated name; null security/template; the handle is closed below.
        let handle = unsafe {
            CreateFileW(
                device.as_ptr(),
                0,
                FILE_SHARE_READ_WRITE,
                std::ptr::null_mut(),
                OPEN_EXISTING,
                0,
                std::ptr::null_mut(),
            )
        };
        if handle.is_null() || handle as isize == -1 {
            return false;
        }
        let query = StoragePropertyQuery { property_id: 0, query_type: 0, additional: [0; 4] };
        let mut out = [0u8; 1024];
        let mut returned = 0u32;
        // SAFETY: input/output buffers are valid for the given sizes; synchronous call.
        let ok = unsafe {
            DeviceIoControl(
                handle,
                IOCTL_STORAGE_QUERY_PROPERTY,
                (&query as *const StoragePropertyQuery).cast(),
                size_of::<StoragePropertyQuery>() as u32,
                out.as_mut_ptr().cast(),
                out.len() as u32,
                &mut returned,
                std::ptr::null_mut(),
            )
        } != 0;
        // SAFETY: handle came from CreateFileW and is closed once.
        unsafe { CloseHandle(handle) };
        ok && returned as usize >= BUS_TYPE_OFFSET + 4
            && u32::from_le_bytes(out[BUS_TYPE_OFFSET..BUS_TYPE_OFFSET + 4].try_into().unwrap())
                == BUS_TYPE_USB
    }

    fn ready_with_size(root: &[u16]) -> bool {
        let (mut avail, mut total, mut free) = (0u64, 0u64, 0u64);
        // SAFETY: valid NUL-terminated root and out pointers.
        let ok = unsafe { GetDiskFreeSpaceExW(root.as_ptr(), &mut avail, &mut total, &mut free) };
        ok != 0 && total > 0
    }

    pub fn usb_flash_drives() -> Vec<String> {
        let mut old = 0u32;
        // SAFETY: plain flag call; restored below. Avoids "insert a disk" dialogs.
        unsafe { SetThreadErrorMode(SEM_FAILCRITICALERRORS, &mut old) };
        // SAFETY: no arguments.
        let mask = unsafe { GetLogicalDrives() };
        let mut out = Vec::new();
        for (i, letter) in ('A'..='Z').enumerate() {
            if mask & (1 << i) == 0 {
                continue;
            }
            let root_str = format!("{letter}:\\");
            let root = wide(&root_str);
            // SAFETY: valid NUL-terminated root path.
            let kind = unsafe { GetDriveTypeW(root.as_ptr()) };
            if kind == DRIVE_REMOVABLE && is_usb_bus(letter) && ready_with_size(&root) {
                out.push(root_str);
            }
        }
        // SAFETY: restores the previous mode.
        unsafe { SetThreadErrorMode(old, std::ptr::null_mut()) };
        out
    }
}
