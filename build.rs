fn main() {
    #[cfg(windows)]
    {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("icon.ico");
        res.set("ProductName", "FastCam");
        res.set("FileDescription", "FastCam - tiny, fast webcam app");
        res.set("LegalCopyright", "MIT License");
        res.set("OriginalFilename", "fastcam.exe");
        res.compile().unwrap();
    }
}
