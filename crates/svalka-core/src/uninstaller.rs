use crate::programs::Program;
use serde::Serialize;
use std::process::Command;

/// Split an uninstall string into (program, args). Handles a leading quoted
/// path; otherwise takes the whole string up to the first space as the program.
pub fn parse_uninstall_cmd(cmd: &str) -> (String, Vec<String>) {
    let cmd = cmd.trim();
    if let Some(rest) = cmd.strip_prefix('"') {
        if let Some(end) = rest.find('"') {
            let prog = rest[..end].to_string();
            let args = rest[end + 1..]
                .split_whitespace()
                .map(str::to_string)
                .collect();
            return (prog, args);
        }
    }
    let mut parts = cmd.split_whitespace();
    let prog = parts.next().unwrap_or("").to_string();
    (prog, parts.map(str::to_string).collect())
}

#[derive(Debug, Clone, Serialize)]
pub struct UninstallResult {
    pub program: String,
    pub ok: bool,
    pub message: String,
}

/// Run one program's uninstaller (prefers the quiet string). Blocks until it
/// exits and reports the outcome — never panics, never silently swallows.
pub fn uninstall_one(p: &Program) -> UninstallResult {
    let cmd = p.quiet_uninstall.as_ref().or(p.uninstall.as_ref());
    let Some(cmd) = cmd else {
        return UninstallResult {
            program: p.name.clone(),
            ok: false,
            message: "нет строки удаления".into(),
        };
    };
    let (prog, args) = parse_uninstall_cmd(cmd);
    match Command::new(&prog).args(&args).status() {
        Ok(s) if s.success() => UninstallResult {
            program: p.name.clone(),
            ok: true,
            message: "удалено".into(),
        },
        Ok(s) => UninstallResult {
            program: p.name.clone(),
            ok: false,
            message: format!("деинсталлятор завершился с кодом {:?}", s.code()),
        },
        Err(e) => UninstallResult {
            program: p.name.clone(),
            ok: false,
            message: format!("не удалось запустить: {e}"),
        },
    }
}

/// Run a batch sequentially, reporting each result via `on` as it completes.
pub fn uninstall_queue(progs: &[Program], on: impl Fn(&UninstallResult)) -> Vec<UninstallResult> {
    progs
        .iter()
        .map(|p| {
            let r = uninstall_one(p);
            on(&r);
            r
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_quoted_program_with_args() {
        let (prog, args) = parse_uninstall_cmd(r#""C:\App\unins000.exe" /SILENT"#);
        assert_eq!(prog, r"C:\App\unins000.exe");
        assert_eq!(args, vec!["/SILENT"]);
    }

    #[test]
    fn parses_bare_program() {
        let (prog, args) = parse_uninstall_cmd(r"C:\App\setup.exe");
        assert_eq!(prog, r"C:\App\setup.exe");
        assert!(args.is_empty());
    }
}
