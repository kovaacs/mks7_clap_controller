use std::env;
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

#[cfg(not(target_os = "macos"))]
compile_error!("xtask currently packages the verified macOS build only");

fn main() -> Result<(), Box<dyn Error>> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
    let release = !env::args().any(|arg| arg == "--debug");
    let target_dir = match env::var_os("CARGO_TARGET_DIR").map(PathBuf::from) {
        Some(path) if path.is_relative() => root.join(path),
        Some(path) => path,
        None => root.join("target"),
    };
    let mut build = Command::new(env::var_os("CARGO").unwrap_or_else(|| "cargo".into()));
    build
        .current_dir(root)
        .env("CARGO_TARGET_DIR", &target_dir)
        .env("MACOSX_DEPLOYMENT_TARGET", "11.0")
        .args([
            "build",
            "--locked",
            "-p",
            "mks7_clap_controller",
            "--target",
            "aarch64-apple-darwin",
        ]);
    if release {
        build.arg("--release");
    }
    if !build.status()?.success() {
        return Err("cargo build failed for aarch64-apple-darwin".into());
    }

    let profile = if release { "release" } else { "debug" };
    let dist = target_dir.join("clap");
    if dist.exists() {
        fs::remove_dir_all(&dist)?;
    }
    fs::create_dir_all(&dist)?;
    fs::write(dist.join("LICENSE"), include_str!("../../LICENSE"))?;
    fs::write(dist.join("README.md"), include_str!("../../README.md"))?;
    fs::copy(
        root.join("THIRD_PARTY_LICENSES.html"),
        dist.join("THIRD_PARTY_LICENSES.html"),
    )?;
    package(&target_dir, profile, &dist)?;
    Ok(())
}

fn package(target_dir: &Path, profile: &str, dist: &Path) -> Result<(), Box<dyn Error>> {
    let bundle = dist.join("MKS-7 Controller.clap");
    let contents = bundle.join("Contents");
    let executable_dir = contents.join("MacOS");
    fs::create_dir_all(&executable_dir)?;
    let executable = executable_dir.join("MKS-7 Controller");
    fs::copy(
        target_dir
            .join("aarch64-apple-darwin")
            .join(profile)
            .join("libmks7_clap_controller.dylib"),
        executable,
    )?;
    let info_plist = include_str!("Info.plist").replace("@VERSION@", env!("CARGO_PKG_VERSION"));
    fs::write(contents.join("Info.plist"), info_plist)?;
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
