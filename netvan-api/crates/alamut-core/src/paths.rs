use directories::ProjectDirs;
use std::path::PathBuf;

/// Separate from desktop Alamut (`com.Alamut.Alamut`) so both can coexist.
pub fn project_dirs() -> Option<ProjectDirs> {
    ProjectDirs::from("com", "Alamut", "AlamutApi")
}

pub fn data_dir() -> PathBuf {
    // Prefer ProgramData-style path when available; directories crate uses Roaming AppData on Windows.
    // Override with ALAMUT_API_DATA_DIR for service installs.
    if let Ok(custom) = std::env::var("ALAMUT_API_DATA_DIR") {
        return PathBuf::from(custom);
    }
    let program_data = std::env::var_os("ProgramData")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(r"C:\ProgramData"));
    let dir = program_data.join("Alamut").join("AlamutApi");
    if dir.exists() || std::fs::create_dir_all(&dir).is_ok() {
        return dir;
    }
    project_dirs()
        .map(|p| p.data_dir().to_path_buf())
        .unwrap_or_else(|| PathBuf::from(".").join("alamut-api-data"))
}

pub fn db_path() -> PathBuf {
    data_dir().join("alamut-web.db")
}

pub fn ensure_data_dir() -> std::io::Result<PathBuf> {
    let dir = data_dir();
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

pub fn webui_dir() -> Option<PathBuf> {
    if let Ok(custom) = std::env::var("ALAMUT_WEBUI_DIR") {
        let p = PathBuf::from(custom);
        if p.exists() {
            return Some(p);
        }
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(exe_dir) = exe.parent() {
            let alongside = exe_dir.join("webui");
            if alongside.exists() {
                return Some(alongside);
            }
        }
    }
    let manifest_dir = option_env!("CARGO_MANIFEST_DIR").map(PathBuf::from);
    if let Some(manifest) = manifest_dir {
        let in_repo = manifest
            .join("../../..")
            .join("alamut-webui")
            .join("dist");
        if in_repo.exists() {
            return Some(in_repo);
        }
        let in_repo_root = manifest.join("../../../alamut-webui/dist");
        if in_repo_root.exists() {
            return Some(in_repo_root);
        }
    }
    None
}

pub const DEFAULT_BIND: &str = "127.0.0.1:28437";

