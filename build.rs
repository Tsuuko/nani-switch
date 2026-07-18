fn main() {
    if cfg!(target_os = "windows") {
        let mut resource = winresource::WindowsResource::new();
        resource
            .set_icon("assets/tray.ico")
            .set("ProductName", "Nani Switch")
            .set("FileDescription", "Nani account switcher")
            .set("LegalCopyright", "")
            .compile()
            .expect("failed to compile Windows resources");
    }
}
