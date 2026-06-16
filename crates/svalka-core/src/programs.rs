use serde::Serialize;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize)]
pub enum ProgramSource {
    Registry,
    Uwp,
}

#[derive(Debug, Clone, Serialize)]
pub struct Program {
    pub name: String,
    pub publisher: Option<String>,
    pub install_location: Option<PathBuf>,
    pub uninstall: Option<String>,
    pub quiet_uninstall: Option<String>,
    pub real_size: u64,
    pub source: ProgramSource,
}

#[cfg(windows)]
pub fn read_programs_from(root: &winreg::RegKey) -> Vec<Program> {
    let mut out = Vec::new();
    for name in root.enum_keys().filter_map(Result::ok) {
        let Ok(k) = root.open_subkey(&name) else { continue };
        let display: Option<String> = k.get_value("DisplayName").ok();
        let system: u32 = k.get_value("SystemComponent").unwrap_or(0);
        let Some(display) = display else { continue };
        if system == 1 {
            continue;
        }
        let install_location: Option<String> = k.get_value("InstallLocation").ok();
        out.push(Program {
            name: display,
            publisher: k.get_value("Publisher").ok(),
            install_location: install_location
                .filter(|s| !s.is_empty())
                .map(PathBuf::from),
            uninstall: k.get_value("UninstallString").ok(),
            quiet_uninstall: k.get_value("QuietUninstallString").ok(),
            real_size: 0,
            source: ProgramSource::Registry,
        });
    }
    out
}

#[cfg(windows)]
pub fn list_programs() -> Vec<Program> {
    use winreg::enums::*;
    use winreg::RegKey;

    let sources = [
        (HKEY_LOCAL_MACHINE, r"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall"),
        (HKEY_LOCAL_MACHINE, r"SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall"),
        (HKEY_CURRENT_USER, r"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall"),
    ];

    let mut all: Vec<Program> = Vec::new();
    for (hive, path) in sources {
        if let Ok(root) = RegKey::predef(hive).open_subkey(path) {
            all.extend(read_programs_from(&root));
        }
    }

    // dedupe by (name, publisher)
    all.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    all.dedup_by(|a, b| a.name == b.name && a.publisher == b.publisher);

    // real size from InstallLocation (the whole point — not EstimatedSize)
    for p in &mut all {
        if let Some(loc) = &p.install_location {
            p.real_size = crate::sizer::dir_size(loc);
        }
    }
    all
}

#[cfg(not(windows))]
pub fn list_programs() -> Vec<Program> {
    Vec::new()
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;
    use serial_test::serial;
    use winreg::enums::*;
    use winreg::RegKey;

    #[test]
    #[serial]
    fn reads_program_fields_from_registry() {
        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        let base = r"Software\SvalkaTest\Uninstall";
        let (app, _) = hkcu.create_subkey(format!(r"{base}\MyApp")).unwrap();
        app.set_value("DisplayName", &"My App").unwrap();
        app.set_value("Publisher", &"Acme").unwrap();
        app.set_value("UninstallString", &r"C:\MyApp\uninstall.exe").unwrap();

        let root = hkcu.open_subkey(base).unwrap();
        let progs = read_programs_from(&root);

        hkcu.delete_subkey_all(r"Software\SvalkaTest").unwrap();

        let p = progs.iter().find(|p| p.name == "My App").expect("app present");
        assert_eq!(p.publisher.as_deref(), Some("Acme"));
        assert_eq!(p.uninstall.as_deref(), Some(r"C:\MyApp\uninstall.exe"));
    }
}
