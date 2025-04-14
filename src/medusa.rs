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
use crate::MEDUSA_TIMEOUT;
use crate::git::GitRepo;
#[derive(Clone, Debug)]
pub enum MedusaState {
    // keep the pid for now, selectively terminate some?
    Running { pid: u32 },
    Stopped { status: ExitStatus },
    Error { message: String },
}

impl std::fmt::Display for MedusaState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MedusaState::Running { pid } => write!(f, "Running (PID: {})", pid),
            MedusaState::Stopped { status } => write!(f, "Stopped (Status: {})", status),
            MedusaState::Error { message } => write!(f, "Error (Message: {})", message),
        }
    }
}

use crate::REPO_DIR;

pub struct MedusaHandler {
    /// List of all active medusa processes and their current state
    process: Arc<Mutex<HashMap<String, MedusaState>>>,
}

impl MedusaHandler {
    pub fn new() -> Self {
        let process = Arc::new(Mutex::new(HashMap::new()));

        Self { process }
    }

    pub async fn start_all(self: &Self, repos: Vec<GitRepo>) -> Result<(), Error> {
        for repo in repos {
            repo.git_sync().await?;
            self.run_medusa(repo.clone()).await?;
        }

        Ok(())
    }

    pub async fn run_medusa(self: &Self, repo: GitRepo) -> Result<u32, Error> {
        let mut child = Command::new("medusa")
            .current_dir(Path::new(REPO_DIR).join(repo.name()))
            .arg("fuzz")
            .arg("--timeout")
            .arg(MEDUSA_TIMEOUT)
            .spawn()
            .map_err(|e| format!("Failed to spawn Medusa: {e}"))?;

        let child_pid = child.id().ok_or("Failed to get child PID")?;

        let process_clone = Arc::clone(&self.process);
        let repo_clone = repo.clone();

        // Add the process to the map, as "running"
        {
            let mut map = self.process.lock().await;
            map.insert(repo.name(), MedusaState::Running { pid: child_pid });
        }

        // spawn a monitoring task, to update the process state (ie err/exit)
        tokio::spawn(async move {
            let status = child.wait().await;
            let mut map = process_clone.lock().await;

            match status {
                Ok(status) => map.insert(repo_clone.name(), MedusaState::Stopped { status }),
                Err(err) => map.insert(
                    repo_clone.name(),
                    MedusaState::Error {
                        message: err.to_string(),
                    },
                ),
            };
        });

        Ok(child_pid)
    }

    pub async fn stop_process(self: &Self, repo: String) -> Result<(), Error> {
        let mut map = self.process.lock().await;

        let state = map.get(&repo).ok_or(format!("Repo {repo} not found"))?;

        match state {
            MedusaState::Running { pid } => {
                Command::new("kill")
                    .arg("-2")
                    .arg(pid.to_string())
                    .spawn()?;
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
