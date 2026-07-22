fn main() {
    // Compile resources/style.css + the bundled icon pack into a GResource that
    // gets embedded in the binary (see main.rs).
    glib_build_tools::compile_resources(
        &["resources"],
        "resources/raudio.gresource.xml",
        "raudio.gresource",
    );
}
