//! The only place with `unsafe`: thin wrappers over the Win32 calls the bot
//! needs (send input, read the F12 abort key, identify the foreground game).
#![allow(unsafe_code)]

use std::os::windows::ffi::OsStringExt;

use windows_sys::Win32::Foundation::{CloseHandle, HWND};
use windows_sys::Win32::System::Threading::{
    OpenProcess, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION, QueryFullProcessImageNameW,
};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, INPUT, INPUT_0, INPUT_KEYBOARD, INPUT_MOUSE, KEYBDINPUT, KEYEVENTF_KEYUP,
    KEYEVENTF_SCANCODE, MOUSEEVENTF_MOVE, MOUSEEVENTF_WHEEL, MOUSEINPUT, SendInput, VK_F12,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, GetWindowThreadProcessId};

/// Opaque foreground-window id (compared, never dereferenced).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Window(usize);

pub fn foreground() -> Window {
    // SAFETY: no arguments; returns a handle value we only compare.
    Window(unsafe { GetForegroundWindow() } as usize)
}

/// Full path of the executable that owns `w`, if it can be queried.
pub fn exe_path(w: Window) -> Option<String> {
    let mut pid = 0u32;
    // SAFETY: valid out-pointer; an invalid window just yields pid 0.
    unsafe { GetWindowThreadProcessId(w.0 as HWND, &mut pid) };
    if pid == 0 {
        return None;
    }
    // SAFETY: plain handle open; checked for null and closed below.
    let h = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) };
    if h.is_null() {
        return None;
    }
    let mut buf = [0u16; 1024];
    let mut len = buf.len() as u32;
    // SAFETY: buffer and length describe valid writable memory.
    let ok =
        unsafe { QueryFullProcessImageNameW(h, PROCESS_NAME_WIN32, buf.as_mut_ptr(), &mut len) };
    // SAFETY: `h` came from OpenProcess above.
    unsafe { CloseHandle(h) };
    (ok != 0).then(|| {
        std::ffi::OsString::from_wide(&buf[..len as usize])
            .to_string_lossy()
            .into_owned()
    })
}

pub fn f12_down() -> bool {
    // SAFETY: plain key-state query.
    (unsafe { GetAsyncKeyState(i32::from(VK_F12)) } as u16 & 0x8000) != 0
}

fn send(inputs: &[INPUT]) {
    // SAFETY: slice of fully initialised INPUT structs with the right size.
    unsafe {
        SendInput(
            inputs.len() as u32,
            inputs.as_ptr(),
            std::mem::size_of::<INPUT>() as i32,
        )
    };
}

/// Key press/release by hardware scancode (what games read via raw input).
pub fn key(scancode: u16, down: bool) {
    let flags = KEYEVENTF_SCANCODE | if down { 0 } else { KEYEVENTF_KEYUP };
    send(&[INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: 0,
                wScan: scancode,
                dwFlags: flags,
                time: 0,
                dwExtraInfo: 0,
            },
        },
    }]);
}

fn mouse(dx: i32, dy: i32, data: i32, flags: u32) {
    send(&[INPUT {
        r#type: INPUT_MOUSE,
        Anonymous: INPUT_0 {
            mi: MOUSEINPUT {
                dx,
                dy,
                mouseData: data as u32,
                dwFlags: flags,
                time: 0,
                dwExtraInfo: 0,
            },
        },
    }]);
}

/// Relative mouse move in counts (positive dx = right).
pub fn mouse_move(dx: i32, dy: i32) {
    mouse(dx, dy, 0, MOUSEEVENTF_MOVE);
}

/// Wheel notches: positive = up, negative = down.
pub fn wheel(notches: i32) {
    mouse(0, 0, notches * 120, MOUSEEVENTF_WHEEL);
}
