use std::env;

fn main() {
    let target_os = env::var("CARGO_CFG_TARGET_OS").expect("Cargo did not provide the target OS");
    assert!(
        target_os == "macos",
        "mks7_clap_controller currently has a verified MIDI implementation for macOS only"
    );
}
