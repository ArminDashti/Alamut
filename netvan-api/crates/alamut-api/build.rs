use std::env;
use std::path::PathBuf;
use std::process::Command;

fn set_version_resource(bin_name: &str, file_desc: &str) {
    if std::env::var("CARGO_CFG_WINDOWS").is_err() {
        return;
    }
    let version = std::env::var("CARGO_PKG_VERSION").unwrap_or_else(|_| "0.2.0".to_string());
    let mut parts = version
        .split('.')
        .map(|p| p.parse::<u64>().unwrap_or(0))
        .collect::<Vec<_>>();
    while parts.len() < 4 {
        parts.push(0);
    }
    let (a, b, c, d) = (parts[0], parts[1], parts[2], parts[3]);

    let mut res = winres::WindowsResource::new();
    res.set_version_info(winres::VersionInfo::FILEVERSION, (a << 48) | (b << 32) | (c << 16) | d);
    res.set_version_info(winres::VersionInfo::PRODUCTVERSION, (a << 48) | (b << 32) | (c << 16) | d);
    res.set("CompanyName", "Dashti Technologies LLC");
    res.set("ProductName", "Alamut");
    res.set("FileDescription", file_desc);
    res.set(
        "LegalCopyright",
        "Copyright (c) Dashti Technologies LLC. All rights reserved.",
    );
    res.set("OriginalFilename", bin_name);
    res.set("InternalName", bin_name);
    if let Err(e) = res.compile() {
        println!("cargo:warning=version resource compile failed ({e}); continuing without it");
    }
}

fn main() {
    set_version_resource(
        "alamut-api.exe",
        "Alamut Windows service — collectors + localhost API",
    );

    println!("cargo:rerun-if-changed=../../tools/alamut-hwmon/Program.cs");
    println!("cargo:rerun-if-changed=../../tools/alamut-hwmon/AlamutHwmon.csproj");

    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let proj = manifest_dir.join("../../tools/alamut-hwmon/AlamutHwmon.csproj");
    if !proj.is_file() {
        println!("cargo:warning=alamut-hwmon project missing; thermal helper will not be built");
        return;
    }

    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    let Some(profile_dir) = out_dir.ancestors().nth(3) else {
        println!("cargo:warning=could not resolve cargo profile dir for alamut-hwmon");
        return;
    };

    let status = Command::new("dotnet")
        .args([
            "publish",
            proj.to_str().unwrap_or_default(),
            "-c",
            "Release",
            "-r",
            "win-x64",
            "--self-contained",
            "false",
            "-o",
            profile_dir.to_str().unwrap_or_default(),
        ])
        .status();

    match status {
        Ok(s) if s.success() => {}
        Ok(s) => println!("cargo:warning=dotnet publish alamut-hwmon exited {s}"),
        Err(e) => println!("cargo:warning=dotnet not available ({e}); thermal helper not published"),
    }
}
