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

    let output = Command::new("dotnet")
        .args(&[
            "publish",
            "../src-vrc-oscquery/listen-for-vrc/listen-for-vrc.csproj",
            "-c",
            "Release", // Configuration: Release mode
            "--self-contained=true",
            "-p:PublishSingleFile=true",
            "-p:PublishTrimmed=true", // Optional: trims unused code, reducing the binary size.
            "-o",
            output_folder, // Output directory for the published files
        ])
        .output() // Capture the output rather than just the status.
        .expect("Failed to execute dotnet publish.");

    if !output.status.success() {
        // Convert stdout from bytes to a string and print.
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        p!(
            "dotnet publish failed with output:\n{} ERR:{}",
            stdout,
            stderr
        );
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
