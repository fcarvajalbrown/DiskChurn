fn main() {
    // suppress console window in release builds
    if std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default() == "windows" {
        println!("cargo:rustc-link-arg=/SUBSYSTEM:WINDOWS");
        println!("cargo:rustc-link-arg=/ENTRY:mainCRTStartup");
    }
}
