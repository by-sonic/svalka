use std::path::{Path, PathBuf};
use std::process::Command;

/// Allowed hive prefixes for export — guards against a malformed key string
/// reaching reg.exe.
const ALLOWED_HIVES: &[&str] = &[
    "HKLM\\",
    "HKCU\\",
    "HKCR\\",
    "HKU\\",
    "HKCC\\",
    "HKEY_LOCAL_MACHINE\\",
    "HKEY_CURRENT_USER\\",
    "HKEY_CLASSES_ROOT\\",
    "HKEY_USERS\\",
    "HKEY_CURRENT_CONFIG\\",
];

/// Fully-qualified path to reg.exe so PATH cannot be hijacked.
fn reg_exe() -> PathBuf {
    let sysroot = std::env::var("SystemRoot").unwrap_or_else(|_| r"C:\Windows".to_string());
    PathBuf::from(sysroot).join(r"System32\reg.exe")
}

/// Export a registry key (e.g. `HKCU\Software\Foo`) to a `.reg` file.
pub fn export_reg(full_key: &str, out_file: &Path) -> Result<(), String> {
    let upper = full_key.to_uppercase();
    if !ALLOWED_HIVES.iter().any(|h| upper.starts_with(h)) {
        return Err(format!("отклонён ключ реестра: {full_key}"));
    }
    run_reg(&["export", full_key, &out_file.to_string_lossy(), "/y"])
}

/// Re-import a previously exported `.reg` file.
pub fn import_reg(reg_file: &Path) -> Result<(), String> {
    run_reg(&["import", &reg_file.to_string_lossy()])
}

fn run_reg(args: &[&str]) -> Result<(), String> {
    let out = Command::new(reg_exe())
        .args(args)
        .output()
        .map_err(|e| format!("не удалось запустить reg.exe: {e}"))?;
    if out.status.success() {
        Ok(())
    } else {
        Err(String::from_utf8_lossy(&out.stderr).trim().to_string())
    }
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;
    use serial_test::serial;
    use tempfile::tempdir;
    use winreg::enums::*;
    use winreg::RegKey;

    #[test]
    #[serial]
    fn export_then_delete_then_import_restores_value() {
        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        let subkey = r"Software\SvalkaTest\Backup";
        let (key, _) = hkcu.create_subkey(subkey).unwrap();
        key.set_value("Marker", &"hello").unwrap();

        let dir = tempdir().unwrap();
        let reg_file = dir.path().join("backup.reg");
        export_reg(&format!(r"HKCU\{subkey}"), &reg_file).unwrap();

        hkcu.delete_subkey_all(r"Software\SvalkaTest").unwrap();
        assert!(hkcu.open_subkey(subkey).is_err());

        import_reg(&reg_file).unwrap();
        let restored = hkcu.open_subkey(subkey).unwrap();
        let v: String = restored.get_value("Marker").unwrap();
        assert_eq!(v, "hello");

        hkcu.delete_subkey_all(r"Software\SvalkaTest").unwrap();
    }
}
