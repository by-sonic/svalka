use std::path::PathBuf;

/// Resolver signature: maps an env var name to its value. Injected so logic
/// is testable without touching the real environment.
pub type EnvFn<'a> = dyn Fn(&str) -> Option<String> + 'a;

pub fn live_env(key: &str) -> Option<String> {
    std::env::var(key).ok()
}

pub fn system_temp_roots(env: &EnvFn) -> Vec<PathBuf> {
    let mut v = Vec::new();
    if let Some(t) = env("TEMP") {
        v.push(PathBuf::from(t));
    }
    if let Some(w) = env("SystemRoot") {
        v.push(PathBuf::from(w).join("Temp"));
    }
    v
}

pub fn browser_cache_roots(env: &EnvFn) -> Vec<PathBuf> {
    let mut v = Vec::new();
    if let Some(local) = env("LOCALAPPDATA") {
        let base = PathBuf::from(&local);
        for sub in [
            r"Google\Chrome\User Data\Default\Cache",
            r"Microsoft\Edge\User Data\Default\Cache",
        ] {
            v.push(base.join(sub));
        }
    }
    v
}

pub fn dev_cache_roots(env: &EnvFn) -> Vec<PathBuf> {
    let mut v = Vec::new();
    if let Some(up) = env("USERPROFILE") {
        let base = PathBuf::from(&up);
        v.push(base.join(r".cargo\registry\cache"));
        v.push(base.join(r".gradle\caches"));
    }
    if let Some(local) = env("LOCALAPPDATA") {
        v.push(PathBuf::from(&local).join(r"pip\Cache"));
        v.push(PathBuf::from(&local).join("go-build"));
    }
    v
}

pub fn thumbnail_cache_roots(env: &EnvFn) -> Vec<PathBuf> {
    env("LOCALAPPDATA")
        .map(|l| vec![PathBuf::from(l).join(r"Microsoft\Windows\Explorer")])
        .unwrap_or_default()
}

pub fn crash_dump_roots(env: &EnvFn) -> Vec<PathBuf> {
    let mut v = Vec::new();
    if let Some(l) = env("LOCALAPPDATA") {
        v.push(PathBuf::from(l).join("CrashDumps"));
    }
    if let Some(w) = env("SystemRoot") {
        v.push(PathBuf::from(w).join("Minidump"));
    }
    v
}

pub fn wer_roots(env: &EnvFn) -> Vec<PathBuf> {
    let mut v = Vec::new();
    if let Some(p) = env("ProgramData") {
        v.push(PathBuf::from(p).join(r"Microsoft\Windows\WER"));
    }
    if let Some(l) = env("LOCALAPPDATA") {
        v.push(PathBuf::from(l).join(r"Microsoft\Windows\WER"));
    }
    v
}

pub fn windows_update_roots(env: &EnvFn) -> Vec<PathBuf> {
    env("SystemRoot")
        .map(|w| vec![PathBuf::from(w).join(r"SoftwareDistribution\Download")])
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn system_temp_roots_resolve_from_env() {
        let env = |k: &str| match k {
            "TEMP" => Some(r"C:\Users\me\AppData\Local\Temp".to_string()),
            "SystemRoot" => Some(r"C:\Windows".to_string()),
            _ => None,
        };
        let roots = system_temp_roots(&env);
        assert!(roots.iter().any(|p| p.ends_with("Temp")));
        assert!(roots.iter().any(|p| p.ends_with(r"Windows\Temp")));
    }

    #[test]
    fn crash_dump_roots_resolve() {
        let env = |k: &str| match k {
            "LOCALAPPDATA" => Some(r"C:\Users\me\AppData\Local".to_string()),
            "SystemRoot" => Some(r"C:\Windows".to_string()),
            _ => None,
        };
        let roots = crash_dump_roots(&env);
        assert!(roots.iter().any(|p| p.ends_with("CrashDumps")));
        assert!(roots.iter().any(|p| p.ends_with("Minidump")));
    }
}
