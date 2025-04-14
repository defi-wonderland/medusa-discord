use crate::{Error, REPO_DIR};
use std::path::Path;
use std::process::Command;

/// Represents a git repository and an optional branch
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitRepo {
    url: String,
    branch: Option<String>,
}

impl std::fmt::Display for GitRepo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name())
    }
}

impl GitRepo {
    pub fn new(url: String) -> Self {
        Self {
            url: extract_dir_from_url(&url).unwrap(),
            branch: extract_branch_from_url(&url).unwrap(),
        }
    }

    /// Extracts the directory name from the URL
    pub fn name(&self) -> String {
        extract_dir_from_url(&self.url).unwrap()
    }

    pub fn url(&self) -> String {
        self.url.clone()
    }

    pub fn branch(&self) -> Option<String> {
        self.branch.clone()
    }

    /// Clones the repository (only clone one branch for performance)
    pub fn git_clone(&self) -> Result<(), Error> {
        let dir = extract_dir_from_url(&self.url).unwrap();
        let path = Path::new(REPO_DIR).join(&dir);

        if path.exists() {
            return Err("Repo already exists".into());
        } else {
            let mut cmd = Command::new("git");
            cmd.current_dir(REPO_DIR).arg("clone");

            if let Some(branch) = self.branch() {
                cmd.arg("--branch").arg(&branch);
            }
            cmd.arg("--single-branch");

            cmd.arg(&self.url);

            let result = cmd
                .status()
                .map_err(|e| format!("Failed to clone repo: {e}"))?;

            if !result.success() {
                return Err("Failed to clone repo".into());
            }
        }

        Ok(())
    }

    /// Pulls the repository in the corresponding directory (based on the name in the url)
    pub fn git_pull(&self) -> Result<(), Error> {
        let dir = extract_dir_from_url(&self.url).unwrap();
        let path = Path::new(REPO_DIR).join(&dir);

        let result = Command::new("git")
            .current_dir(path)
            .arg("pull")
            .status()
            .map_err(|e| format!("Failed to pull repo: {e}"))?;

        if !result.success() {
            return Err("Failed to pull repo".into());
        }

        Ok(())
    }
}

/// Extracts the directory name from the URL
pub fn extract_dir_from_url(url: &str) -> Result<String, Error> {
    let dir = url.split('/').last().ok_or("Wrong URL")?;

    if dir.contains(".git") {
        let dir = dir.split(".git").next().ok_or("Wrong URL")?;
        Ok(dir.to_string())
    } else {
        Ok(dir.to_string())
    }
}

/// Extracts the branch name from the URL
pub fn extract_branch_from_url(url: &str) -> Result<Option<String>, Error> {
    let dir = url.split('/').last().ok_or("Wrong URL")?;

    if dir.contains(":") {
        let parts = dir.split(":").collect::<Vec<&str>>();
        Ok(Some(parts[1].to_string()))
    } else {
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_dir_from_url() {
        let url = "https://github.com/abc/def-ghi5.git";
        let dir = extract_dir_from_url(url).unwrap();
        assert_eq!(dir, "def-ghi5");
    }
}
