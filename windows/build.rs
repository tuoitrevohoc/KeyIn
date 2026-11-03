//! Embeds the KeyIn logo (windows/assets/keyin.ico) as icon resource 1 of the DLL,
//! used by the input profile, the taskbar button, the settings window and the installer.
fn main() {
    println!("cargo:rerun-if-changed=keyin.rc");
    println!("cargo:rerun-if-changed=assets/keyin.ico");
    // Resources are compiled with the Windows SDK's rc.exe; cross-checks from other
    // hosts (cargo check --target *-windows-msvc on macOS) build without them.
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") && cfg!(windows) {
        // Linked as an argument so the resource lands in the cdylib itself.
        embed_resource::compile_for_everything("keyin.rc", embed_resource::NONE);
    }
}
