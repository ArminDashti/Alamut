//! Windows user/system environment variables via the registry, through PowerShell.
//! Name and value travel in env vars, never in the script text (no injection).
//!
//! The API runs as a service (LocalSystem), so `[Environment]::...("User")` would
//! read the service account's profile (`systemprofile`). User scope therefore
//! targets the logged-on user's hive: `HKU\<SID>\Environment`.

use std::collections::BTreeMap;
use std::process::Command;

use alamut_core::types::{EnvVar, EnvVarScope};
use anyhow::{bail, Context, Result};
use base64::{engine::general_purpose::STANDARD, Engine};

// ponytail: one powershell spawn per call (~300 ms). Fine for a settings page;
// upgrade to direct registry calls (Win32 RegOpenKeyEx) if called in a loop.

/// Resolves `$key` to the registry path of the scope's environment hive.
fn key_script(scope: EnvVarScope) -> &'static str {
    match scope {
        EnvVarScope::User => "$sid=(New-Object System.Security.Principal.NTAccount((Get-CimInstance Win32_ComputerSystem).UserName)).Translate([System.Security.Principal.SecurityIdentifier]).Value;$key=\"Registry::HKEY_USERS\\$sid\\Environment\";",
        EnvVarScope::System => "$key='Registry::HKEY_LOCAL_MACHINE\\SYSTEM\\CurrentControlSet\\Control\\Session Manager\\Environment';",
    }
}

fn validate_name(name: &str) -> Result<()> {
    if name.is_empty() || name.contains('=') || name.contains('\0') {
        bail!("invalid variable name: {name:?}");
    }
    Ok(())
}

/// Runs a script; returns stdout (ASCII only: callers base64 any text output).
fn run(script: &str, scope: EnvVarScope, name: &str, value: &str) -> Result<String> {
    let mut cmd = Command::new("powershell");
    crate::win_cmd::hide_console(&mut cmd);
    let out = cmd
        .args(["-NoProfile", "-NonInteractive", "-Command"])
        .arg(format!("$ErrorActionPreference='Stop';{}{script}", key_script(scope)))
        .env("ALAMUT_ENV_NAME", name)
        .env("ALAMUT_ENV_VALUE", value)
        .output()
        .context("failed to start powershell")?;
    if !out.status.success() {
        bail!("{}", String::from_utf8_lossy(&out.stderr).trim());
    }
    Ok(String::from_utf8(out.stdout)?.trim().to_string())
}

pub fn list(scope: EnvVarScope) -> Result<Vec<EnvVar>> {
    let out = run(
        "$k=Get-Item -LiteralPath $key;$m=@{};foreach($n in $k.GetValueNames()){$m[$n]=[string]$k.GetValue($n)};\
         [Convert]::ToBase64String([Text.Encoding]::UTF8.GetBytes(($m|ConvertTo-Json -Compress)))",
        scope,
        "",
        "",
    )?;
    let json = String::from_utf8(STANDARD.decode(out)?)?;
    let map: BTreeMap<String, String> = serde_json::from_str(&json)?;
    Ok(map.into_iter().map(|(name, value)| EnvVar { name, value }).collect())
}

/// Create or update. Empty values are rejected: deleting is a separate action.
pub fn set(scope: EnvVarScope, name: &str, value: &str) -> Result<()> {
    validate_name(name)?;
    if value.is_empty() {
        bail!("value must not be empty; delete the variable instead");
    }
    run(
        "New-ItemProperty -LiteralPath $key -Name $env:ALAMUT_ENV_NAME -Value $env:ALAMUT_ENV_VALUE -PropertyType ExpandString -Force|Out-Null",
        scope,
        name,
        value,
    )?;
    Ok(())
}

pub fn delete(scope: EnvVarScope, name: &str) -> Result<()> {
    validate_name(name)?;
    run(
        "Remove-ItemProperty -LiteralPath $key -Name $env:ALAMUT_ENV_NAME",
        scope,
        name,
        "",
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_bad_names() {
        assert!(validate_name("").is_err());
        assert!(validate_name("A=B").is_err());
        assert!(validate_name("PATH").is_ok());
    }
}
