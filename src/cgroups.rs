use nix::unistd::Pid;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};

pub struct CGroupConfig {
    // The kernel will transform values like 1k, 1m, 1g, ... to
    // bytes by itself, but we should take in bytes directly
    pub mem: String,
}

pub struct CGroup {
    config: CGroupConfig,
    path: PathBuf,
    limits_written: bool,
}

impl CGroup {
    pub fn new(base_cgroup: &Path, config: CGroupConfig) -> Result<Self, std::io::Error> {
        let mut path = base_cgroup.to_path_buf();
        path.push(format!("dabba-{}", nix::unistd::getpid()));

        log::info!("Using cgroup path {path:?}");
        std::fs::create_dir(&path)?;

        Ok(Self {
            config,
            path,
            limits_written: false,
        })
    }

    /// Write the limits from CGroupConfig
    /// cgroup.subtree_control of the parent cgroup must have
    /// the relevant controllers enabled
    fn write_limits(&mut self) -> Result<(), std::io::Error> {
        if self.limits_written {
            return Ok(());
        }

        let mut file = OpenOptions::new()
            .write(true)
            .open(self.path.join("memory.max"))?;
        file.write_all(self.config.mem.as_bytes())?;

        // We only need to write the limits once
        self.limits_written = true;
        Ok(())
    }

    /// Enforce the limits for a given PID
    pub fn enforce(&mut self, pid: Pid) -> Result<(), std::io::Error> {
        self.write_limits()?;

        let mut file = OpenOptions::new()
            .write(true)
            .open(self.path.join("cgroup.procs"))?;
        file.write_all(pid.to_string().as_bytes())?;

        Ok(())
    }
}

impl Drop for CGroup {
    /// Ensure that we clean up after ourselves automatically
    fn drop(&mut self) {
        if let Err(err) = std::fs::remove_dir(&self.path) {
            log::warn!("Failed to cleanup cgroup dir: {err}");
        }
    }
}
