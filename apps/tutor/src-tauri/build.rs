fn main() {
    // tauri-build only watches tauri.conf.json: without this a new icon is not embedded until something else changes
    println!("cargo:rerun-if-changed=icons/icon.ico");
    tauri_build::build()
}
