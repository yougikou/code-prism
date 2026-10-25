use std::{fs::OpenOptions, io::Write, path::PathBuf, process::Command};

pub struct ProcessLock {
    path: PathBuf,
    token: String,
}

impl ProcessLock {
    pub fn acquire(config_path: &str) -> anyhow::Result<Self> {
        let config_path = std::path::Path::new(config_path);
        let config = match std::fs::canonicalize(config_path) {
            Ok(path) => path,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                let absolute = if config_path.is_absolute() {
                    config_path.to_path_buf()
                } else {
                    std::env::current_dir()?.join(config_path)
                };
                let parent = absolute
                    .parent()
                    .filter(|path| !path.as_os_str().is_empty())
                    .unwrap_or_else(|| std::path::Path::new("."));
                std::fs::canonicalize(parent)?.join(
                    absolute
                        .file_name()
                        .ok_or_else(|| anyhow::anyhow!("invalid configuration path"))?,
                )
            }
            Err(error) => return Err(error.into()),
        };
        let parent = config
            .parent()
            .filter(|path| !path.as_os_str().is_empty())
            .unwrap_or_else(|| std::path::Path::new("."));
        let path = parent.join(".codeprism-write.lock");
        for _ in 0..3 {
            let token = format!("{} {}", std::process::id(), uuid::Uuid::new_v4());
            match OpenOptions::new().write(true).create_new(true).open(&path) {
                Ok(mut file) => {
                    if let Err(error) = file
                        .write_all(token.as_bytes())
                        .and_then(|_| file.sync_all())
                    {
                        drop(file);
                        let _ = std::fs::remove_file(&path);
                        return Err(error.into());
                    }
                    return Ok(Self { path, token });
                }
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                    if std::fs::symlink_metadata(&path)?.file_type().is_symlink() {
                        anyhow::bail!("refusing to follow symlink lock file {}", path.display());
                    }
                    let existing = std::fs::read_to_string(&path).unwrap_or_default();
                    let pid = existing
                        .split_whitespace()
                        .next()
                        .and_then(|value| value.parse::<u32>().ok());
                    if pid.is_some_and(process_is_running) {
                        anyhow::bail!("another CodePrism process holds {}", path.display());
                    }
                    std::fs::remove_file(&path)?;
                }
                Err(error) => return Err(error.into()),
            }
        }
        anyhow::bail!("could not acquire CodePrism data directory lock")
    }
}

impl Drop for ProcessLock {
    fn drop(&mut self) {
        if std::fs::read_to_string(&self.path).ok().as_deref() == Some(&self.token) {
            let _ = std::fs::remove_file(&self.path);
        }
    }
}

fn process_is_running(pid: u32) -> bool {
    #[cfg(unix)]
    {
        Command::new("kill")
            .args(["-0", &pid.to_string()])
            .status()
            .is_ok_and(|status| status.success())
    }
    #[cfg(windows)]
    {
        Command::new("tasklist")
            .args(["/FI", &format!("PID eq {pid}"), "/NH"])
            .output()
            .is_ok_and(|output| String::from_utf8_lossy(&output.stdout).contains(&pid.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lock_is_exclusive_and_released_on_drop() {
        let dir = tempfile::tempdir().unwrap();
        let config = dir.path().join("config.yaml");
        std::fs::write(&config, "database_url: sqlite:test.db").unwrap();
        let first = ProcessLock::acquire(config.to_str().unwrap()).unwrap();
        assert!(ProcessLock::acquire(config.to_str().unwrap()).is_err());
        drop(first);
        assert!(ProcessLock::acquire(config.to_str().unwrap()).is_ok());
    }
}
