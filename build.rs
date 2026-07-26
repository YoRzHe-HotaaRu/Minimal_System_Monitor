fn main() {
    #[cfg(windows)]
    {
        let mut res = winres::WindowsResource::new();
        res.set_icon("assets/pulse.ico");
        res.set("ProductName", "PULSE System Monitor");
        res.set("FileDescription", "PULSE — minimal Windows system monitor");
        res.set("CompanyName", "PULSE");
        res.set("LegalCopyright", "Copyright © PULSE");
        res.set("OriginalFilename", "Pulse.exe");
        res.set("ProductVersion", "0.1.0");
        res.set("FileVersion", "0.1.0");
        // Avoid console flicker metadata; still a console app for the TUI.
        if let Err(e) = res.compile() {
            eprintln!("winres warning: {e}");
            // Don't fail the build if rc tooling is missing on some setups.
        }
    }
}
