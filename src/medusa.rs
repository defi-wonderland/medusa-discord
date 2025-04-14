use std::{
    collections::HashMap,
    env::var,
    fs::OpenOptions,
    path::Path,
    process::{ExitStatus, Stdio},
    sync::Arc,
};
use tokio::process::Child;
use tokio::process::Command;
use tokio::sync::Mutex;
use tokio::time::{Duration, timeout};

use crate::Error;

#[derive(Clone, Debug)]
pub enum MedusaState {
    // keep the pid for now, selectively terminate some?
    Running { pid: u32 },
    Stopped { status: ExitStatus },
    Error { message: String },
}

pub struct Medusa {
    /// List of all active medusa processes and their current state
    process: Arc<Mutex<HashMap<String, MedusaState>>>,
}

impl Medusa {
    pub fn new() -> Self {
        Self {
            process: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub async fn run_medusa(self: &Self, repo: String) -> Result<String, Error> {
        let mut child = Command::new("medusa")
            .current_dir(repo.clone())
            .arg("fuzz")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| format!("Failed to spawn Medusa: {e}"))?;

        let child_pid = child.id().ok_or("Failed to get child PID")?;

        let process_clone = Arc::clone(&self.process);
        let repo_clone = repo.clone();

        // Add the process to the map, as "running"
        {
            let mut map = self.process.lock().await;
            map.insert(repo.to_string(), MedusaState::Running { pid: child_pid });
        }

        // spawn a monitoring task, to update the process state (ie err/exit)
        tokio::spawn(async move {
            let status = child.wait().await;
            let mut map = process_clone.lock().await;

            match status {
                Ok(status) => map.insert(repo_clone.to_string(), MedusaState::Stopped { status }),
                Err(err) => map.insert(
                    repo_clone.to_string(),
                    MedusaState::Error {
                        message: err.to_string(),
                    },
                ),
            };
        });

        Ok(format!("Started Medusa for {repo}"))
    }

    pub async fn stop_process(self: &Self, repo: String) -> Result<(), Error> {
        let mut map = self.process.lock().await;

        let state = map.get(&repo).ok_or(format!("Repo {repo} not found"))?;

        match state {
            MedusaState::Running { pid } => {
                Command::new("kill").arg(pid.to_string()).spawn()?;
            }
            _ => return Err(format!("Repo {repo} is not running").into()),
        }

        map.remove(&repo);
        Ok(())
    }

    pub async fn get_process_state(self: &Self, repo: String) -> Result<MedusaState, Error> {
        let map = self.process.lock().await;

        Ok(map
            .get(&repo)
            .cloned()
            .ok_or_else(|| format!("Repo {repo} not found"))?)
    }
}
