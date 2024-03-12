pub unsafe extern "system" fn _enumerate_callback(hwnd: HWND, lparam: LPARAM) -> BOOL {
    let hwnds: &mut Vec<(HWND, String, String)> =
        &mut *(lparam as *mut Vec<(HWND, String, String)>);

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

    //let a = &lparam as *mut Vec<isize>;
    if IsWindowVisible(hwnd) == TRUE
        && IsWindowEnabled(hwnd) == TRUE
        && !window_text_as_str.is_empty()
    {
        hwnds.push((
            hwnd,
            class_name_as_str.to_string(),
            window_text_as_str.to_string(),
        ));
    }

    TRUE
}