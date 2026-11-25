//! Update detection and the update command. Free of any GUI dependency so the
//! parsing/command-building logic is unit-testable.

use std::path::PathBuf;
use std::process::Command;

/// Where updates are checked for / applied from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub enum Source {
    Pacman,
    Aur,
    #[default]
    Both,
}

impl Source {
    pub const ALL: [Source; 3] = [Source::Pacman, Source::Aur, Source::Both];
    pub const LABELS: [&'static str; 3] = ["Pacman (official repos)", "AUR", "Pacman + AUR"];

    pub fn index(self) -> usize {
        Self::ALL.iter().position(|s| *s == self).unwrap_or(0)
    }

    pub fn includes_pacman(self) -> bool {
        matches!(self, Source::Pacman | Source::Both)
    }

    pub fn includes_aur(self) -> bool {
        matches!(self, Source::Aur | Source::Both)
    }
}

/// Outcome of one update check.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CheckResult {
    pub pacman: usize,
    pub aur: usize,
    pub errors: Vec<String>,
}

impl CheckResult {
    pub fn total(&self) -> usize {
        self.pacman + self.aur
    }
}

/// Each pending update is one non-empty line of output.
pub fn count_lines(output: &str) -> usize {
    output.lines().filter(|l| !l.trim().is_empty()).count()
}

/// Returns the first of paru/yay found on `PATH`.
pub fn detect_aur_helper() -> Option<&'static str> {
    ["paru", "yay"].into_iter().find(|h| find_in_path(h).is_some())
}

pub fn find_in_path(program: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .map(|dir| dir.join(program))
        .find(|candidate| candidate.is_file())
}

/// Runs a checker command. `Ok(count)` on success; exit code 2 with no output
/// (`checkupdates`) or exit code 1 with no output (`paru/yay -Qua`) mean "none".
fn run_check(program: &str, args: &[&str], empty_code: i32) -> Result<usize, String> {
    let output = Command::new(program)
        .args(args)
        .output()
        .map_err(|e| format!("could not run `{program}`: {e}"))?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    match output.status.code() {
        Some(0) => Ok(count_lines(&stdout)),
        Some(code) if code == empty_code && count_lines(&stdout) == 0 => Ok(0),
        _ => {
            let stderr = String::from_utf8_lossy(&output.stderr);
            Err(format!("`{program}` failed: {}", stderr.trim()))
        }
    }
}

/// Blocking check of the selected sources.
pub fn check(source: Source) -> CheckResult {
    let mut result = CheckResult::default();
    if source.includes_pacman() {
        // `checkupdates` (pacman-contrib) uses a temporary db, so it needs no root
        // and never leaves the system in a partial-upgrade state.
        match run_check("checkupdates", &[], 2) {
            Ok(n) => result.pacman = n,
            Err(e) => result.errors.push(format!("{e} (is pacman-contrib installed?)")),
        }
    }
    if source.includes_aur() {
        match detect_aur_helper() {
            Some(helper) => match run_check(helper, &["-Qua"], 1) {
                Ok(n) => result.aur = n,
                Err(e) => result.errors.push(e),
            },
            None => result
                .errors
                .push("No AUR helper found (install paru or yay)".to_string()),
        }
    }
    result
}

/// Shell command that applies updates for `source`, or `None` if impossible
/// (AUR requested but no helper available).
pub fn update_command(source: Source, helper: Option<&str>) -> Option<String> {
    match (source, helper) {
        (Source::Pacman, _) => Some("sudo pacman -Syu".to_string()),
        (Source::Aur, Some(h)) => Some(format!("{h} -Sua")),
        (Source::Aur, None) => None,
        // AUR helpers upgrade the official repos too.
        (Source::Both, Some(h)) => Some(format!("{h} -Syu")),
        (Source::Both, None) => Some("sudo pacman -Syu".to_string()),
    }
}

/// Terminals we know how to drive: (binary, flag preceding the command).
const TERMINALS: [(&str, Option<&str>); 7] = [
    ("cosmic-term", Some("-e")),
    ("kitty", None),
    ("alacritty", Some("-e")),
    ("foot", None),
    ("konsole", Some("-e")),
    ("gnome-terminal", Some("--")),
    ("xterm", Some("-e")),
];

/// Blocking: runs `command` in a terminal and waits for the window to close.
pub fn run_in_terminal(command: &str) -> Result<(), String> {
    let script = format!("{command}; echo; read -rp 'Finished. Press Enter to close...' _");
    for (term, flag) in TERMINALS {
        if find_in_path(term).is_none() {
            continue;
        }
        let mut cmd = Command::new(term);
        if let Some(flag) = flag {
            cmd.arg(flag);
        }
        cmd.args(["bash", "-c", &script]);
        return cmd
            .status()
            .map(|_| ())
            .map_err(|e| format!("could not launch {term}: {e}"));
    }
    Err("No supported terminal emulator found".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_non_empty_lines() {
        assert_eq!(count_lines(""), 0);
        assert_eq!(count_lines("\n  \n"), 0);
        assert_eq!(count_lines("linux 6.1 -> 6.2\nmesa 1 -> 2\n"), 2);
    }

    #[test]
    fn source_inclusion() {
        assert!(Source::Pacman.includes_pacman() && !Source::Pacman.includes_aur());
        assert!(!Source::Aur.includes_pacman() && Source::Aur.includes_aur());
        assert!(Source::Both.includes_pacman() && Source::Both.includes_aur());
        for s in Source::ALL {
            assert_eq!(Source::ALL[s.index()], s);
        }
    }

    #[test]
    fn update_commands() {
        assert_eq!(update_command(Source::Pacman, Some("yay")).unwrap(), "sudo pacman -Syu");
        assert_eq!(update_command(Source::Aur, Some("yay")).unwrap(), "yay -Sua");
        assert_eq!(update_command(Source::Aur, None), None);
        assert_eq!(update_command(Source::Both, Some("paru")).unwrap(), "paru -Syu");
        assert_eq!(update_command(Source::Both, None).unwrap(), "sudo pacman -Syu");
    }

    #[test]
    fn total_sums_sources() {
        let r = CheckResult { pacman: 3, aur: 2, errors: vec![] };
        assert_eq!(r.total(), 5);
    }
}
