use windows_sys::Win32::UI::WindowsAndMessaging::{
    IDYES, MB_ICONERROR, MB_ICONINFORMATION, MB_ICONWARNING, MB_OK, MB_YESNO, MessageBoxW,
};

fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(Some(0)).collect()
}

fn message_box(title: &str, message: &str, style: u32) -> i32 {
    let title = wide(title);
    let message = wide(message);
    unsafe {
        MessageBoxW(
            std::ptr::null_mut(),
            message.as_ptr(),
            title.as_ptr(),
            style,
        )
    }
}

pub fn confirm_delete(name: &str) -> bool {
    message_box(
        "Remove saved account",
        &format!(
            "Remove \"{name}\" from Nani Switch?\n\nThis does not delete the Nani account or its server data."
        ),
        MB_YESNO | MB_ICONWARNING,
    ) == IDYES
}

pub fn error(title: &str, message: &str) {
    message_box(title, message, MB_OK | MB_ICONERROR);
}

pub fn info(title: &str, message: &str) {
    message_box(title, message, MB_OK | MB_ICONINFORMATION);
}
