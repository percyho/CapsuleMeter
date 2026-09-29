use std::{
    env,
    ffi::OsString,
    path::{Path, PathBuf},
    process::Command,
};

fn find_in_path(path: OsString) -> Option<PathBuf> {
    env::split_paths(&path)
        .map(|directory| directory.join("codex"))
        .find(|candidate| candidate.is_file())
}

#[cfg(target_os = "macos")]
fn find_in_login_shell() -> Option<PathBuf> {
    let mut shells = Vec::new();
    if let Some(shell) = env::var_os("SHELL") {
        shells.push(shell);
    }
    shells.push(OsString::from("/bin/zsh"));

    for shell in shells {
        let Ok(output) = Command::new(shell)
            .args(["-lic", "command -v codex"])
            .output()
        else {
            continue;
        };
        if !output.status.success() {
            continue;
        }

        for line in String::from_utf8_lossy(&output.stdout).lines() {
            let candidate = Path::new(line.trim());
            if candidate.is_absolute() && candidate.is_file() {
                return Some(candidate.to_path_buf());
            }
        }
    }

    None
}

pub fn executable() -> OsString {
    if let Some(path) = env::var_os("PATH").and_then(find_in_path) {
        return path.into_os_string();
    }

    #[cfg(target_os = "macos")]
    if let Some(path) = find_in_login_shell() {
        return path.into_os_string();
    }

    OsString::from("codex")
}
