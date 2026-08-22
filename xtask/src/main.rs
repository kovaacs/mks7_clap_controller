use std::env;
use std::error::Error;
use std::fs;
use std::path::Path;
use std::process::Command;

#[cfg(not(target_os = "macos"))]
compile_error!("xtask currently packages the verified macOS build only");

fn main() -> Result<(), Box<dyn Error>> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
    let release = !env::args().any(|arg| arg == "--debug");
    let mut build = Command::new(env::var_os("CARGO").unwrap_or_else(|| "cargo".into()));
    build
        .current_dir(root)
        .args(["build", "-p", "mks7_clap_controller"]);
    if release {
        build.arg("--release");
    }
    if !build.status()?.success() {
        return Err("cargo build failed".into());
    }

    let profile = if release { "release" } else { "debug" };
    let target = root.join("target").join(profile);
    let dist = root.join("target").join("clap");
    fs::create_dir_all(&dist)?;
    package(&target, &dist)?;
    Ok(())
}

fn package(target: &Path, dist: &Path) -> Result<(), Box<dyn Error>> {
    let bundle = dist.join("MKS-7 CLAP Controller.clap");
    if bundle.exists() {
        fs::remove_dir_all(&bundle)?;
    }
    let contents = bundle.join("Contents");
    let executable_dir = contents.join("MacOS");
    fs::create_dir_all(&executable_dir)?;
    fs::copy(
        target.join("libmks7_clap_controller.dylib"),
        executable_dir.join("MKS-7 CLAP Controller"),
    )?;
    fs::write(contents.join("Info.plist"), include_str!("Info.plist"))?;
    let status = Command::new("codesign")
        .args(["--force", "--deep", "--sign", "-"])
        .arg(&bundle)
        .status()?;
    if !status.success() {
        return Err("codesign failed".into());
    }
    println!("Packaged {}", bundle.display());
    Ok(())
}
