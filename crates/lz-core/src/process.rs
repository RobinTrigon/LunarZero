//! Cross-platform process helpers: which shell runs the agent's commands,
//! how to find a tool on PATH (Windows needs `.exe` / `.cmd` / `.bat`
//! resolution — `CreateProcess` only appends `.exe`), and how to kill a
//! command's whole process tree on timeout.

use std::path::{Path, PathBuf};

/// A shell and how to hand it a command line.
#[derive(Debug, Clone)]
pub struct Shell {
    pub program: PathBuf,
    /// arguments placed before the command text
    pub args: Vec<String>,
    /// what to call it in prompts (`zsh`, `bash`, `PowerShell`)
    pub name: String,
    /// PowerShell rather than a POSIX shell
    pub powershell: bool,
}

impl std::fmt::Display for Shell {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.program.display())
    }
}

impl Shell {
    /// Command to run `command` in this shell.
    pub fn command(&self, command: &str) -> tokio::process::Command {
        let mut c = tokio::process::Command::new(&self.program);
        c.args(&self.args).arg(command);
        c
    }
}

/// Pick the shell for agent commands: the configured one, else `$SHELL`
/// (never fish/nu — their syntax differs), else `/bin/sh`; on Windows
/// PowerShell 7 (`pwsh`), then Windows PowerShell, then Git Bash.
pub fn select_shell(configured: Option<&str>) -> Shell {
    if let Some(c) = configured.filter(|s| !s.is_empty()) {
        return shell_from(PathBuf::from(c));
    }
    if cfg!(windows) {
        for name in ["pwsh", "powershell", "bash", "sh"] {
            if let Some(p) = resolve_bin(name) {
                return shell_from(p);
            }
        }
        return shell_from(PathBuf::from("powershell.exe"));
    }
    let candidate = std::env::var("SHELL")
        .ok()
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "/bin/sh".into());
    let name = Path::new(&candidate)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("");
    if matches!(name, "fish" | "nu" | "nushell") {
        for fallback in ["/bin/zsh", "/bin/bash", "/bin/sh"] {
            if Path::new(fallback).exists() {
                return shell_from(PathBuf::from(fallback));
            }
        }
    }
    shell_from(PathBuf::from(candidate))
}

fn shell_from(program: PathBuf) -> Shell {
    // last segment on either separator, minus `.exe`
    let text = program.to_string_lossy().to_string();
    let last = text.rsplit(['/', '\\']).next().unwrap_or("sh");
    let stem = last
        .strip_suffix(".exe")
        .or_else(|| last.strip_suffix(".EXE"))
        .unwrap_or(last)
        .to_ascii_lowercase();
    let stem = if stem.is_empty() { "sh".to_string() } else { stem };
    let powershell = matches!(stem.as_str(), "pwsh" | "powershell");
    Shell {
        name: if powershell {
            "PowerShell".into()
        } else {
            stem.clone()
        },
        args: if powershell {
            vec![
                "-NoLogo".into(),
                "-NoProfile".into(),
                "-NonInteractive".into(),
                "-Command".into(),
            ]
        } else {
            vec!["-c".into()]
        },
        program,
        powershell,
    }
}

/// Executable extensions Windows resolves implicitly (`PATHEXT`).
#[cfg(windows)]
fn pathext() -> Vec<String> {
    std::env::var("PATHEXT")
        .unwrap_or_else(|_| ".COM;.EXE;.BAT;.CMD".into())
        .split(';')
        .filter(|e| !e.is_empty())
        .map(|e| e.to_ascii_lowercase())
        .collect()
}

/// Full path of `bin` on PATH (or the path itself when absolute), trying
/// the Windows executable extensions.
pub fn resolve_bin(bin: &str) -> Option<PathBuf> {
    let candidates = |dir: &Path| -> Option<PathBuf> {
        let plain = dir.join(bin);
        if plain.is_file() {
            return Some(plain);
        }
        #[cfg(windows)]
        {
            for ext in pathext() {
                let p = dir.join(format!("{bin}{ext}"));
                if p.is_file() {
                    return Some(p);
                }
            }
        }
        None
    };
    let p = Path::new(bin);
    if p.is_absolute() || bin.contains('/') || bin.contains('\\') {
        let dir = p.parent()?;
        let name = p.file_name()?.to_str()?;
        return candidates(dir).or_else(|| {
            let _ = name;
            None
        });
    }
    std::env::var_os("PATH")
        .into_iter()
        .flat_map(|p| std::env::split_paths(&p).collect::<Vec<_>>())
        .find_map(|dir| candidates(&dir))
}

pub fn on_path(bin: &str) -> bool {
    resolve_bin(bin).is_some()
}

/// A command for an external tool. On Windows, `.cmd`/`.bat` launchers
/// (npm, npx, prettier, typescript-language-server…) cannot be spawned
/// directly and go through `cmd /C`.
pub fn command(bin: &str) -> tokio::process::Command {
    let resolved = resolve_bin(bin).unwrap_or_else(|| PathBuf::from(bin));
    let is_script = resolved
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| matches!(e.to_ascii_lowercase().as_str(), "cmd" | "bat"));
    if cfg!(windows) && is_script {
        let mut c = tokio::process::Command::new("cmd");
        c.arg("/C").arg(resolved);
        c
    } else {
        tokio::process::Command::new(resolved)
    }
}

/// Stop a command and everything it started.
pub async fn kill_tree(pid: Option<u32>, child: &mut tokio::process::Child) {
    #[cfg(unix)]
    {
        if let Some(pid) = pid {
            unsafe {
                libc::killpg(pid as i32, libc::SIGTERM);
            }
            let _ = tokio::time::timeout(std::time::Duration::from_secs(3), child.wait()).await;
            unsafe {
                libc::killpg(pid as i32, libc::SIGKILL);
            }
        }
        let _ = child.kill().await;
    }
    #[cfg(not(unix))]
    {
        if let Some(pid) = pid {
            let _ = tokio::process::Command::new("taskkill")
                .args(["/T", "/F", "/PID", &pid.to_string()])
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .status()
                .await;
        }
        let _ = child.kill().await;
    }
}

/// PATH separator for prepending our own binary's directory.
pub fn path_with(dir: &Path) -> String {
    let current = std::env::var_os("PATH").unwrap_or_default();
    let mut parts: Vec<PathBuf> = vec![dir.to_path_buf()];
    parts.extend(std::env::split_paths(&current).filter(|p| p != dir));
    std::env::join_paths(parts)
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|_| current.to_string_lossy().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shell_selection() {
        let sh = select_shell(Some("/bin/bash"));
        assert_eq!(sh.name, "bash");
        assert_eq!(sh.args, vec!["-c"]);
        assert!(!sh.powershell);
        let ps = select_shell(Some("C:\\Program Files\\PowerShell\\7\\pwsh.exe"));
        assert!(ps.powershell);
        assert_eq!(ps.name, "PowerShell");
        assert!(ps.args.contains(&"-Command".to_string()));
        let auto = select_shell(None);
        assert!(!auto.program.as_os_str().is_empty());
    }

    #[test]
    fn resolves_binaries_on_path() {
        assert!(resolve_bin("sh").is_some() || cfg!(windows));
        assert!(resolve_bin("definitely-not-a-binary-xyz").is_none());
        let p = path_with(Path::new("/opt/lz-test"));
        assert!(p.starts_with("/opt/lz-test") || cfg!(windows));
    }
}
