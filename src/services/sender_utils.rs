/// This module provides utility functions for sending data in a game communication system.
/// It includes functions for initializing a game, retrieving the handler of a process, and
/// getting information about running processes.
///
/// ```
///
/// # Note
///
/// This module relies on the `sysinfo` and `winapi` crates for retrieving process information
/// and interacting with the Windows API, respectively.
///
/// The `initialize_game` function can be used to start a game process given its path. It supports
/// both `.exe` files and shortcuts (`.lnk` or `.url` files).
///
/// The `getHandler` function retrieves the handler (HWND) of a running process given its path.
/// It uses the Windows API to enumerate visible windows and match the process based on its PID.
///
/// The `get_processes_info` function retrieves information about all running processes, including
/// their PIDs and executable paths.
///
/// The `enum_windows_proc` function is a callback used by the `EnumWindows` function from the
/// Windows API to enumerate visible windows and match the process based on its PID.
///
/// The `get_hwnd_by_pid` function uses the `EnumWindows` function to find the handler (HWND) of
/// a running process given its PID.
///
use std::io::{Error, ErrorKind};

use std::process::Command;

use std::thread::sleep;
use std::time::Duration;
use winapi::{
    shared::{
        minwindef::{BOOL, DWORD, FALSE, LPARAM, TRUE},
        windef::HWND,
    },
    um::winuser::{
        EnumWindows, GetWindowTextLengthW, GetWindowTextW, GetWindowThreadProcessId,
        IsWindowVisible,
    },
};

use sysinfo::System;

#[derive(Debug)]
struct ProcessInfo {
    pid: u32,
    path: String,
}

struct EnumData {
    target_pid: DWORD,
    hwnd: Option<HWND>,
}

const HANDLER_RETRIES: usize = 8;
const HANDLER_SLEEP: usize = 10000;

pub fn initialize_game(game_path: &str) -> Result<(), Error> {
    if game_path.ends_with(".exe") {
        match Command::new(game_path).spawn() {
            Ok(_child) => Ok(()),
            Err(_) => Err(Error::new(ErrorKind::Other, "Error initializing game")),
        }
    } else {
        Err(Error::new(
            ErrorKind::Other,
            "Error type of file not supported",
        ))
    }
}

pub fn get_handler(target_path: &str) -> Result<(u64, u32), Error> {
    let mut found_process: Option<ProcessInfo> = None;
    let game_path: String = target_path.replace("\\\\", "\\");

    //sleep(Duration::from_millis(20000));
    for i in 0..HANDLER_RETRIES {
        if let Ok(processes) = get_processes_info() {
            for process in processes {
                if process.path == game_path {
                    found_process = Some(process);
                    break;
                }
            }
            if found_process.is_some() {
                break;
            }
        }
        sleep(Duration::from_millis(HANDLER_SLEEP as u64));
        println!("Retrying get info... {}/{}\r", i + 1, HANDLER_RETRIES);
    }

    if found_process.is_none() {
        log::error!("SENDER UTILS | Process not found PID");
        return Err(Error::new(
            ErrorKind::Other,
            "Process PID not found after retries",
        ));
    }
    for i in 0..HANDLER_RETRIES {
        if let Some(process) = &found_process {
            if let Some(hwnd) = get_hwnd_by_pid(process.pid) {
                return Ok((hwnd as u64, process.pid));
            }
        }
        sleep(Duration::from_millis(HANDLER_SLEEP as u64));
        println!("Retrying get hwnd... {}/{}\r", i + 1, HANDLER_RETRIES);
    }
    log::error!("SENDER UTILS | Process not found HWND");
    Err(Error::new(
        ErrorKind::Other,
        "Process HWND not found after retries",
    ))
}

/// Function that retrieves ProcessInfo with PID
///
///
/// # Returns
///
/// * `Result<Vec<ProcessInfo>>` - If the function succeeds, the return value is a ProcessInfo to the window. If no window is associated with the process, the return value is an Error.
fn get_processes_info() -> std::io::Result<Vec<ProcessInfo>> {
    let system = System::new_all();
    let mut processes: Vec<ProcessInfo> = Vec::new();

    for (pid, process) in system.processes() {
        if let Some(path) = process.exe() {
            if let Some(path_str) = path.to_str() {
                processes.push(ProcessInfo {
                    pid: pid.as_u32(),
                    path: path_str.to_string(),
                });
            }
        }
    }

    Ok(processes)
}

/// Unsafe function that is called for each window that belongs to the same thread.
///
/// # Arguments
///
/// * `hwnd` - A handle to a window.
/// * `lparam` - The application-defined value given in EnumWindows.
///
/// # Returns
///
/// * `BOOL` - To continue enumeration, the callback function must return TRUE; to stop enumeration, it must return FALSE.
unsafe extern "system" fn enum_windows_proc(hwnd: HWND, lparam: LPARAM) -> BOOL {
    let data = &mut *(lparam as *mut EnumData);
    let mut pid: DWORD = 0;
    GetWindowThreadProcessId(hwnd, &mut pid);
    if pid == data.target_pid && IsWindowVisible(hwnd) == TRUE {
        let length = GetWindowTextLengthW(hwnd) + 1;
        let mut buffer: Vec<u16> = vec![0; length as usize];
        GetWindowTextW(hwnd, buffer.as_mut_ptr(), length);

        data.hwnd = Some(hwnd);
        return FALSE; // Stop enumeration
    }
    TRUE
}

/// Function that retrieves a handle to a window that belongs to a specified process.
///
/// # Arguments
///
/// * `pid` - The process ID.
///
/// # Returns
///
/// * `Option<HWND>` - If the function succeeds, the return value is a handle to the window. If no window is associated with the process, the return value is None.
fn get_hwnd_by_pid(pid: DWORD) -> Option<HWND> {
    let mut data = EnumData {
        target_pid: pid,
        hwnd: None,
    };
    unsafe {
        EnumWindows(
            Some(enum_windows_proc),
            &mut data as *mut EnumData as LPARAM,
        );
    }
    data.hwnd
}
