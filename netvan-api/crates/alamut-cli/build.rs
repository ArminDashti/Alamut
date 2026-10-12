fn main() {
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
    res.set("FileDescription", "Alamut interactive terminal dashboard");
    res.set(
        "LegalCopyright",
        "Copyright (c) Dashti Technologies LLC. All rights reserved.",
    );
    res.set("OriginalFilename", "alamut-cli.exe");
    res.set("InternalName", "alamut-cli.exe");
    if let Err(e) = res.compile() {
        println!("cargo:warning=version resource compile failed ({e}); continuing without it");
    }
}
