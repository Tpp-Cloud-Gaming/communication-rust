use std::io::{Error, ErrorKind};
use std::process::Command;

use winapi::{
    shared::{
        minwindef::{BOOL, DWORD, LPARAM, TRUE},
        windef::HWND,
    },
    um::winuser::{
        EnumWindows, GetClassNameW, GetWindowTextW, GetWindowThreadProcessId, IsWindowEnabled,
        IsWindowVisible,
    },
};

pub fn initialize_game(game_path: &str) -> Result<u32, Error> {
    // TODO: Check tokio option. Handle the error non generically
    match Command::new(game_path).spawn() {
        Ok(child) => Ok(child.id()),
        Err(_) => Err(Error::new(ErrorKind::Other, "Error initializing game")),
    }
}

unsafe extern "system" fn enumerate_callback(hwnd: HWND, lparam: LPARAM) -> BOOL {
    let hwnds: &mut Vec<(HWND, String, String, DWORD)> =
        &mut *(lparam as *mut Vec<(HWND, String, String, DWORD)>);

    let mut class_name = [0u16; 256];
    let mut window_text = [0u16; 256];

    // Get class name of the window
    GetClassNameW(hwnd, class_name.as_mut_ptr(), 256);

    // Get window text
    GetWindowTextW(hwnd, window_text.as_mut_ptr(), 256);

    // Convert window text and class name to Rust strings
    let binding = String::from_utf16(&window_text).unwrap();
    let window_text_as_str = binding.trim_matches(char::from(0));
    let binding = String::from_utf16(&class_name).unwrap();
    let class_name_as_str = binding.trim_matches(char::from(0));

    let mut process_id: DWORD = 0;
    GetWindowThreadProcessId(hwnd, &mut process_id);

    //let a = &lparam as *mut Vec<isize>;
    if IsWindowVisible(hwnd) == TRUE
        && IsWindowEnabled(hwnd) == TRUE
        && !window_text_as_str.is_empty()
    {
        hwnds.push((
            hwnd,
            class_name_as_str.to_string(),
            window_text_as_str.to_string(),
            process_id,
        ));
    }

    TRUE
}

pub fn select_game_window(pid: u32) -> usize {
    let mut hwnds: Vec<(HWND, String, String, DWORD)> = Vec::new();
    unsafe { EnumWindows(Some(enumerate_callback), &mut hwnds as *mut _ as LPARAM) };

    let mut window_handle = 1000;
    // Inside a while to wait for the window of the game to start
    while window_handle == 1000 {
        for (count, element) in hwnds.iter().enumerate() {
            println!(
                "[{}] PID: {:?}, Class Name:  {}, Window Text: {}",
                count, element.3, element.1, element.2
            );
            if element.3 == pid {
                window_handle = count;
                log::info!(
                    "SENDER | Found game window | [{}] PID: {:?}, Class Name:  {}, Window Text: {}",
                    count,
                    element.3,
                    element.1,
                    element.2
                );
            }
        }
    }
    window_handle
}
