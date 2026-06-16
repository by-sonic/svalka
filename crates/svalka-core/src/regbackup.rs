use std::path::Path;
use std::process::Command;

/// Export a registry key (e.g. `HKCU\Software\Foo`) to a `.reg` file.
pub fn export_reg(full_key: &str, out_file: &Path) -> Result<(), String> {
    run_reg(&["export", full_key, &out_file.to_string_lossy(), "/y"])
}

/// Re-import a previously exported `.reg` file.
pub fn import_reg(reg_file: &Path) -> Result<(), String> {
    run_reg(&["import", &reg_file.to_string_lossy()])
}

fn run_reg(args: &[&str]) -> Result<(), String> {
    let out = Command::new("reg")
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
