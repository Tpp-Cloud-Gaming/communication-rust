use std::io::{Error, ErrorKind};
use std::mem;
use std::process::Command;

use winapi::shared::minwindef::HMODULE;
use winapi::um::processthreadsapi::OpenProcess;
use winapi::um::psapi::{EnumProcessModulesEx, GetModuleFileNameExW};
use winapi::um::winnt::{PROCESS_QUERY_INFORMATION, PROCESS_VM_READ};
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
    if (game_path.ends_with(".exe")) {
        match Command::new(game_path).spawn() {
            Ok(child) => Ok(child.id()),
            Err(_) => Err(Error::new(ErrorKind::Other, "Error initializing game")),
        }
    } else if game_path.ends_with(".lnk") || game_path.ends_with(".url") {
        match Command::new("cmd")
            .args(&["/c", "START", "", game_path])
            .spawn()
        {
            Ok(child) => Ok(child.id()),
            Err(_) => Err(Error::new(ErrorKind::Other, "Error initializing game")),
        }
    } else {
        Err(Error::new(
            ErrorKind::Other,
            "Error type of file not supported",
        ))
    }
}

unsafe extern "system" fn enumerate_callback(hwnd: HWND, lparam: LPARAM) -> BOOL {
    let hwnds: &mut Vec<(HWND, String, String, DWORD, String)> =
        &mut *(lparam as *mut Vec<(HWND, String, String, DWORD, String)>);

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

    let path_name = path_name(process_id);

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
            path_name,
        ));
    }

    TRUE
}

pub fn select_game_window(game_path: &str) -> u64 {
    let mut window_handle = 1000;
    let mut handle_found = false;

    while handle_found == false {
        let mut hwnds: Vec<(HWND, String, String, DWORD, String)> = Vec::new();
        unsafe { EnumWindows(Some(enumerate_callback), &mut hwnds as *mut _ as LPARAM) };

        // Inside a while to wait for the window of the game to start

        for (count, element) in hwnds.iter().enumerate() {
            // println!(
            //     "[{}] PID: {:?}, Class Name:  {}, Window Text: {}, Game Path [{}] [{}]",
            //     count, element.3, element.1, element.2, element.4, game_path
            // );
            if element.4 == game_path {
                window_handle = element.0 as u64;
                handle_found = true;
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

pub fn path_name(pid: DWORD) -> String {
    let mut path_name = String::new();
    let mut cb_needed: DWORD = 0;
    let mut h_mods: [HMODULE; 1024] = [0u8 as HMODULE; 1024];

    // Get a handle to the process.
    let h_process = unsafe {
        OpenProcess(
            PROCESS_QUERY_INFORMATION | PROCESS_VM_READ,
            winapi::shared::minwindef::FALSE,
            pid,
        )
    };

    if h_process == winapi::shared::ntdef::NULL {
        println!("ERROR");
    }

    // Get a list of all the modules in this process.
    if unsafe {
        EnumProcessModulesEx(
            h_process,
            h_mods.as_mut_ptr(),
            mem::size_of_val(&h_mods) as DWORD,
            &mut cb_needed,
            winapi::um::psapi::LIST_MODULES_DEFAULT,
        ) == TRUE
    } {
        let module_count = (cb_needed / mem::size_of::<HMODULE>() as DWORD) as usize;
        for i in 0..module_count {
            let mut sz_mod_name = [0u16; winapi::shared::minwindef::MAX_PATH];

            // Get the full path to the module's file.
            if unsafe {
                GetModuleFileNameExW(
                    h_process,
                    h_mods[i],
                    sz_mod_name.as_mut_ptr(),
                    sz_mod_name.len() as DWORD,
                )
            } != 0
            {
                // Print the module name and handle value.
                let module_name = String::from_utf16(&sz_mod_name).unwrap();
                let trimmed_name = module_name.trim_matches(char::from(0));
                if trimmed_name.ends_with(".exe") || trimmed_name.ends_with(".EXE") {
                    path_name = trimmed_name.to_owned();
                }
            }
        }
    }

    return path_name;
}
