fn main() {
    // Compile resources/style.css + the bundled icon pack into a GResource that
    // gets embedded in the binary (see main.rs).
    glib_build_tools::compile_resources(
        &["resources"],
        "resources/raudio.gresource.xml",
        "raudio.gresource",
    );

    // On Windows, embed the app icon into the .exe so it shows in Explorer,
    // the taskbar and shortcuts. No-op elsewhere.
    #[cfg(windows)]
    {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("assets/icon.ico");
        let _ = res.compile();
    }
}
