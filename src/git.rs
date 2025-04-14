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
            url: extract_url_without_branch(&url).unwrap(),
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

    /// Clones or pull the repository (only one branch for performance)
    pub async fn git_sync(&self) -> Result<(), Error> {
        let dir = extract_dir_from_url(&self.url).unwrap();
        let path = Path::new(REPO_DIR).join(&dir);

        if path.exists() {
            let result = Command::new("git")
                .current_dir(&path)
                .arg("pull")
                .status()
                .map_err(|e| format!("Failed to pull repo: {e}"))?;

            if !result.success() {
                return Err(format!("Failed to pull repo with err status {result}").into());
            }
        } else {
            let mut cmd = Command::new("git");
            cmd.current_dir(REPO_DIR).arg("clone");

            if let Some(branch) = self.branch() {
                cmd.arg("--branch").arg(&branch);
            }
            cmd.arg("--single-branch");

            cmd.arg(&self.url);

            let status = cmd
                .status()
                .map_err(|e| format!("Failed to clone repo: {e}"))?;

            if !status.success() {
                return Err(format!("Failed to clone repo with err status {status}").into());
            }
        }

        if path.join("package.json").exists() {
            let _ = Command::new("yarn")
                .current_dir(&path)
                .arg("install")
                .status()
                .map_err(|e| format!("Failed to install npm: {e}"))?;
        }

        let _ = Command::new("forge")
            .current_dir(&path)
            .arg("install")
            .status()
            .map_err(|e| format!("Failed to install npm: {e}"))?;

        Ok(())
    }
}

/// Extracts the directory name from the URL
pub fn extract_dir_from_url(url: &str) -> Result<String, Error> {
    let dir = url.split('/').last().ok_or("Wrong URL")?;

    if dir.contains(".git") {
        let dir: &str = dir.split(".git").next().ok_or("Wrong URL")?;
        Ok(dir.to_owned())
    } else {
        Ok(dir.to_owned())
    }
}

/// Extracts the branch name from the URL
pub fn extract_branch_from_url(url: &str) -> Result<Option<String>, Error> {
    let dir = url.split('/').last().ok_or("Wrong URL")?;

    if dir.contains(":") {
        let parts = dir.split(":").collect::<Vec<&str>>();
        Ok(Some(parts[1].to_owned()))
    } else {
        Ok(None)
    }
}

/// Extracts the URL without the branch
pub fn extract_url_without_branch(url: &str) -> Result<String, Error> {
    // only keep what comes after the last column if there are 2

    let parts = url.split(':').collect::<Vec<&str>>();

    if parts.len() == 3 {
        Ok(format!("{}:{}", parts[0], parts[1]))
    } else if parts.len() == 2 {
        Ok(url.to_owned())
    } else {
        Err("Wrong URL".into())
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

    #[test]
    fn test_extract_branch_from_url() {
        let url = "https://github.com/abc/def-ghi5.git:abcdef";
        let branch = extract_branch_from_url(url).unwrap();
        assert_eq!(branch, Some("abcdef".to_string()));
    }

    #[test]
    fn test_extract_url_without_branch() {
        let url = "https://github.com/abc/def-ghi5.git";
        let url_without_branch = extract_url_without_branch(url).unwrap();
        assert_eq!(url_without_branch, "https://github.com/abc/def-ghi5.git");

        let url = "https://github.com/abc/def-ghi5.git:abcdef";
        let url_without_branch = extract_url_without_branch(url).unwrap();
        assert_eq!(url_without_branch, "https://github.com/abc/def-ghi5.git");
    }
}
