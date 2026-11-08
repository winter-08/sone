fn main() {
    // Only the Tauri frontend needs tauri-build (config, icons, capabilities). The `host`
    // build is the backend library under a native UI.
    #[cfg(feature = "tauri-ui")]
    tauri_build::build()
}
