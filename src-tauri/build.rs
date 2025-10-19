use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn should_skip_copy(src: &fs::Metadata, dst: &fs::Metadata) -> bool {
    if src.len() != dst.len() {
        return false;
    }

    match (src.modified(), dst.modified()) {
        (Ok(src_time), Ok(dst_time)) => src_time <= dst_time,
        _ => false,
    }
}

fn copy_if_different(src: &Path, dst: &Path) -> std::io::Result<()> {
    if let (Ok(src_meta), Ok(dst_meta)) = (fs::metadata(src), fs::metadata(dst)) {
        if should_skip_copy(&src_meta, &dst_meta) {
            return Ok(());
        }
    }

    if let Some(parent) = dst.parent() {
        fs::create_dir_all(parent)?;
    }

    fs::copy(src, dst)?;
    Ok(())
}

macro_rules! p {
    ($($tokens: tt)*) => {
        println!("cargo::warning={}", format!($($tokens)*))
    }
}

fn main() {
    p!("starting build script");
    println!("cargo::rerun-if-changed=build.rs");
    println!("cargo::rerun-if-changed=../src-proxy/src");
    println!("cargo::rerun-if-changed=../src-proxy/Cargo.toml");
    println!("cargo::rerun-if-changed=../src-elevated-register/src");
    println!("cargo::rerun-if-changed=../src-elevated-register/Cargo.toml");
    println!("cargo::rerun-if-changed=../src-vrc-oscquery/listen-for-vrc");

    p!("Finsihed rerun stuff");
    let workspace_sidecars = PathBuf::from("../target/sidecars");
    let output_folder = PathBuf::from("./sidecars");

    fs::create_dir_all(&workspace_sidecars)
        .expect("Failed to create workspace sidecars directory");
    fs::create_dir_all(&output_folder).expect("Failed to create sidecars directory");
    fs::create_dir_all(output_folder.join("bHapticsPlayer"))
        .expect("Failed to create bHapticsPlayer directory");
    p!("created directory");

    let publish_output_dir = workspace_sidecars.join("listen-for-vrc");
    if publish_output_dir.exists() {
        let _ = fs::remove_dir_all(&publish_output_dir);
    }
    fs::create_dir_all(&publish_output_dir).expect("Failed to create listen-for-vrc output");
    let proxy_build_dir = workspace_sidecars.join("proxy-target");
    let register_build_dir = workspace_sidecars.join("register-target");

    p!("Building proxy sidecar");

    let status = Command::new("cargo")
        .args(&[
            "build",
            "--release",
            "--manifest-path",
            "../src-proxy/Cargo.toml",
            "--target-dir",
            proxy_build_dir.to_str().expect("proxy target dir not utf8"),
        ])
        .status()
        .expect("failed to build proxy sidecar");
    if !status.success() {
        panic!("Sidecar build failed!");
    }

    p!("Building elevated sidecar");

    let status = Command::new("cargo")
        .args(&[
            "build",
            "--release",
            "--manifest-path",
            "../src-elevated-register/Cargo.toml",
            "--target-dir",
            register_build_dir.to_str().expect("register target dir not utf8"),
        ])
        .status()
        .expect("failed to build reigster sidecar");
    if !status.success() {
        panic!("Sidecar build failed!");
    }

    p!("Building C# VRC native library");

    let mut dotnet = Command::new("dotnet");
    dotnet.args(&[
        "publish",
        "../src-vrc-oscquery/listen-for-vrc/listen-for-vrc.csproj",
        "-c", "Release",
        "-r", "win-x64",
        "-p:PublishAot=true",
        "-p:NativeLib=Shared",
        "-p:SelfContained=true",
        "-p:StripSymbols=true",
        "-p:IlcGenerateDebugInfo=false",
        "-p:IlcGenerateCompletePdb=false",
        "-o", publish_output_dir.to_str().expect("non utf8 path"),
    ]);

    dotnet.env("DOTNET_CLI_TELEMETRY_OPTOUT", "1");
    dotnet.env("DOTNET_NOLOGO", "1");
    dotnet.env("DOTNET_SKIP_FIRST_TIME_EXPERIENCE", "1");

    let status = dotnet.status().expect("Failed to execute dotnet publish.");
    if !status.success() {
        panic!("dotnet publish failed with status: {:?}", status);
    }

    let proxy_src = proxy_build_dir.join("release/BhapticsPlayer.exe");
    let proxy_dst = output_folder.join("bHapticsPlayer/BhapticsPlayer.exe");
    copy_if_different(&proxy_src, &proxy_dst).expect("failed to copy proxy sidecar binary");

    p!("Bhaptics Proxy replaced");

    let register_src = register_build_dir.join("release/elevated-register.exe");
    let register_dst = output_folder.join("elevated-register.exe");
    copy_if_different(&register_src, &register_dst).expect("failed to copy elevated sidecar binary");

    p!("elevated sidecar replaced");

    let dll_src = publish_output_dir.join("listen-for-vrc.dll");
    let dll_dst = output_folder.join("listen-for-vrc.dll");

    copy_if_different(&dll_src, &dll_dst).expect("failed to copy listen-for-vrc dll");

    tauri_build::build();
}
