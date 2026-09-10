//! Windows UAC elevation helpers — used for TUN mode, which needs Administrator.

#[cfg(windows)]
use std::ffi::c_void;

#[cfg(windows)]
#[link(name = "advapi32")]
extern "system" {
    fn OpenProcessToken(process: *mut c_void, desired: u32, token: *mut *mut c_void) -> i32;
    fn GetTokenInformation(
        token: *mut c_void,
        class: i32,
        info: *mut c_void,
        len: u32,
        ret_len: *mut u32,
    ) -> i32;
}

#[cfg(windows)]
#[link(name = "kernel32")]
extern "system" {
    fn GetCurrentProcess() -> *mut c_void;
    fn CloseHandle(h: *mut c_void) -> i32;
}

#[cfg(windows)]
#[link(name = "shell32")]
extern "system" {
    fn ShellExecuteW(
        hwnd: *mut c_void,
        op: *const u16,
        file: *const u16,
        params: *const u16,
        dir: *const u16,
        show: i32,
    ) -> isize;
}

/// Is the current process running with an elevated (Administrator) token?
#[cfg(windows)]
pub fn is_elevated() -> bool {
    const TOKEN_QUERY: u32 = 0x0008;
    const TOKEN_ELEVATION: i32 = 20;

    unsafe {
        let mut token: *mut c_void = std::ptr::null_mut();
        if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) == 0 {
            return false;
        }
        let mut is_elevated: u32 = 0;
        let mut ret_len: u32 = 0;
        let ok = GetTokenInformation(
            token,
            TOKEN_ELEVATION,
            &mut is_elevated as *mut u32 as *mut c_void,
            std::mem::size_of::<u32>() as u32,
            &mut ret_len,
        );
        CloseHandle(token);
        ok != 0 && is_elevated != 0
    }
}

/// Relaunch this same executable through the UAC "runas" verb. Returns true if the
/// elevated process was started (the caller should then exit); false if the user
/// dismissed the prompt or it failed.
#[cfg(windows)]
pub fn relaunch_elevated(extra_args: &[&str]) -> bool {
    use std::os::windows::ffi::OsStrExt;

    let exe = match std::env::current_exe() {
        Ok(p) => p,
        Err(_) => return false,
    };

    let to_wide = |s: &str| s.encode_utf16().chain(std::iter::once(0)).collect::<Vec<u16>>();
    let verb = to_wide("runas");
    let file: Vec<u16> = exe
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    let params = to_wide(&extra_args.join(" "));

    unsafe {
        let r = ShellExecuteW(
            std::ptr::null_mut(),
            verb.as_ptr(),
            file.as_ptr(),
            if extra_args.is_empty() {
                std::ptr::null()
            } else {
                params.as_ptr()
            },
            std::ptr::null(),
            1, // SW_SHOWNORMAL
        );
        r > 32
    }
}

#[cfg(not(windows))]
pub fn is_elevated() -> bool {
    true
}
#[cfg(not(windows))]
pub fn relaunch_elevated(_extra_args: &[&str]) -> bool {
    false
}
