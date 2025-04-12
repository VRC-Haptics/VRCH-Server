use std::fs;
use std::path::Path;
use std::process::Command;

macro_rules! p {
    ($($tokens: tt)*) => {
        println!("cargo::warning={}", format!($($tokens)*))
    }
}

fn main() {
    let output_folder = "./sidecars";

    p!("Building proxy sidecar");

    println!("cargo:rerun-if-changed=../src-proxy/Cargo.toml");
    println!("cargo:rerun-if-changed=../src-proxy/src/");
    let status = Command::new("cargo")
        .args(&[
            "build",
            "--release",
            "--manifest-path",
            "../src-proxy/Cargo.toml",
        ])
        .status()
        .expect("failed to build proxy sidecar");
    if !status.success() {
        panic!("Sidecar build failed!");
    }

    p!("Building elevated sidecar");

    println!("cargo:rerun-if-changed=../src-elevated-register/Cargo.toml");
    println!("cargo:rerun-if-changed=../src-elevated-register/src/");
    let status = Command::new("cargo")
        .args(&[
            "build",
            "--release",
            "--manifest-path",
            "../src-elevated-register/Cargo.toml",
        ])
        .status()
        .expect("failed to build reigster sidecar");
    if !status.success() {
        panic!("Sidecar build failed!");
    }

    p!("Building C# VRC sidecar");

    println!("cargo:rerun-if-changed=../src-vrc-oscquery/listen-for-vrc/listen-for-vrc.csproj");
    println!("cargo:rerun-if-changed=../src-vrc-oscquery/listen-for-vrc/Program.cs");
    let status = Command::new("dotnet")
        .args([
            "publish",
            "../src-vrc-oscquery/listen-for-vrc/listen-for-vrc.csproj",
            "-c",
            "Release",// Configuration: Release mode
            "--self-contained",
            "true",                // Publish as self-contained.
            "-p:PublishSingleFile=true",
            "-p:PublishTrimmed=true", // Optional: trims unused code, reducing the binary size.
            "-o",
            output_folder,       // Output directory for the published files
        ])
        .status()
        .expect("Failed to execute dotnet publish.");
    if !status.success() {
        panic!("Sidecar build failed!");
    }


    // Copy Sidecar to sidecars directory of main app
    let source = Path::new("../src-proxy/target/release/BhapticsPlayer.exe");
    let destination = Path::new("sidecars/bHapticsPlayer/BhapticsPlayer.exe");
    fs::copy(source, destination).expect("failed to copy proxy sidecar binary");

    p!("Bhaptics Proxy replaced");

    let source = Path::new("../src-elevated-register/target/release/elevated-register.exe");
    let destination = Path::new("sidecars/elevated-register.exe");
    fs::copy(source, destination).expect("failed to copy elevated sidecar binary");

    p!("elevated sidecar replaced");

    tauri_build::build();
}
