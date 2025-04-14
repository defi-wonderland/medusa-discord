use crate::git::GitRepo;
use crate::medusa::MedusaState;
use crate::{Context, Error, REPO_DIR};
use std::fs::OpenOptions;
use std::io::Read;
use std::io::Write;
use std::path::Path;
/// Show this help menu
#[poise::command(prefix_command, track_edits, slash_command)]
pub async fn help(
    ctx: Context<'_>,
    #[description = "Specific command to show help about"]
    #[autocomplete = "poise::builtins::autocomplete_command"]
    command: Option<String>,
) -> Result<(), Error> {
    poise::builtins::help(
        ctx,
        command.as_deref(),
        poise::builtins::HelpConfiguration {
            extra_text_at_bottom: "Medusa Discord bot - remotely operate Medusa fuzzing campaigns",
            ..Default::default()
        },
    )
    .await?;
    Ok(())
}

/// Start a new campaign
#[poise::command(slash_command)]
pub async fn start(
    ctx: Context<'_>,
    #[description = "Repo URL"] repo_url: String,
    #[description = "Branch name (optional)"] branch: Option<String>,
) -> Result<(), Error> {
    ctx.defer().await?;

    let mut add_to_list = false;

    let repo_with_branch = if let Some(branch) = branch {
        format!("{}:{}", repo_url, branch.clone())
    } else {
        repo_url.clone()
    };

    let repo = GitRepo::new(repo_with_branch.clone());

    // check if repo is in the vec (scoped mutex lock)
    {
        let mut repos = ctx.data().repos.lock().await;

        if !repos.contains(&repo) {
            repos.push(repo.clone());
            add_to_list = true;
        }
    }

    if add_to_list {
        let mut repos_file = OpenOptions::new()
            .append(true)
            .open(Path::new(REPO_DIR).join("repos.txt"))
            .expect("Failed to open repos.txt");

        repos_file
            .write_all((repo_with_branch + "\n").as_bytes())
            .expect("Failed to write to repos.txt");
    }

    repo.git_sync().await?;

    // check if already running - same scope to avoid race and starting twice if spamming /start
    let response = {
        let medusa = ctx.data().medusa_handler.lock().await;
        medusa.run_medusa(repo.clone()).await?;

        let process_state = medusa.get_process_state(repo.name()).await; // read-only

        match process_state {
            Ok(MedusaState::Running { pid }) => {
                format!(
                    "Fuzzing campaign running for {} (PID: {})",
                    repo.name(),
                    pid
                )
            }
            Ok(_) => {
                format!("Failed to launch {}", repo)
            }
            Err(e) => {
                format!("Error starting fuzzing campaign for {}: {}", repo.name(), e)
            }
        }
    };

    ctx.say(response).await?;
    Ok(())
}

/// Stop a given campaign (stop the fuzz but can be resumed later on using start)
#[poise::command(slash_command)]
pub async fn pause(
    ctx: Context<'_>,
    #[description = "Repo name (see status cmd)"] repo_name: String,
) -> Result<(), Error> {
    // Check if campaign is running
    // if not, return an error
    // if yes, stop it
    // remove from the vec and repos.txt file
    // return the stats
    {
        let medusa = ctx.data().medusa_handler.lock().await;
        medusa.stop_process(repo_name.clone()).await?;
    }

    let response = format!("Paused {}", repo_name);
    ctx.say(response).await?;
    Ok(())
}

/// Return the status of all campaigns
#[poise::command(slash_command)]
pub async fn status(ctx: Context<'_>) -> Result<(), Error> {
    // mutex inside {} avoiding locking in await
    let mut repo_names = Vec::new();
    let mut medusa_status = Vec::new();

    {
        let repos = ctx.data().repos.lock().await;
        let medusa = ctx.data().medusa_handler.lock().await;

        for repo in repos.iter() {
            let repo_status = medusa.get_process_state(repo.name()).await?;

            repo_names.push(repo.clone().name());

            medusa_status.push(format!("{}: {}\n", repo.name(), repo_status));
        }
    }

    let repo_list = format!("Currently {} campaigns: \n", repo_names.len());
    let all_status = medusa_status
        .iter()
        .map(|s| s.as_str())
        .collect::<Vec<&str>>()
        .join(",\n");

    ctx.say(repo_list).await?;
    if !all_status.is_empty() {
        ctx.say(all_status).await?;
    }

    Ok(())
}

/// Archive a campaign (pause, delist and move the corpus to archive)
#[poise::command(slash_command)]
pub async fn stop(
    ctx: Context<'_>,
    #[description = "Repo name (see status cmd)"] repo_name: String,
) -> Result<(), Error> {
    // check if campaign is running
    // if yes, stop it
    // move the corpus to archive/
    // remove from the vec and repos.txt file
    // return the stats
    {
        let medusa = ctx.data().medusa_handler.lock().await;
        medusa.stop_process(repo_name.clone()).await?;
    }

    let repo = {
        let repos = ctx.data().repos.lock().await;
        let repo = repos.iter().find(|r| r.name() == repo_name).unwrap();
        repo.clone()
    };

    let repo_with_branch = if repo.branch().is_some() {
        format!("{}:{}", repo.url(), repo.branch().unwrap())
    } else {
        repo.url()
    };

    // move the repo url from repos.txt to archive/archive.txt
    let archive_dir = Path::new(REPO_DIR).join("archive");
    if !archive_dir.exists() {
        std::fs::create_dir(archive_dir.clone()).expect("Failed to create archive directory");
    }

    let mut archive_file = OpenOptions::new()
        .append(true)
        .create(true)
        .open(Path::new(REPO_DIR).join("archive").join("archive.txt"))
        .expect("Failed to open or create archive.txt");

    archive_file
        .write_all((repo_with_branch.clone() + "\n").as_bytes())
        .expect("Failed to write to archive.txt");

    let path = Path::new(REPO_DIR).join("repos.txt");
    let mut repos_file = OpenOptions::new()
        .read(true)
        .write(true)
        .open(path)
        .expect("Failed to open repos.txt");

    let mut repos_file_content = String::new();
    repos_file
        .read_to_string(&mut repos_file_content)
        .expect("Failed to read repos.txt");

    let mut repos_file_content = repos_file_content.split("\n").collect::<Vec<&str>>();
    repos_file_content.remove(
        repos_file_content
            .iter()
            .position(|r| *r == repo_with_branch)
            .unwrap_or_else(|| panic!("Failed to find repo in repos.txt: {}", &repo_with_branch)),
    );

    repos_file
        .write_all(repos_file_content.join("\n").as_bytes())
        .expect("Failed to write to repos.txt");

    // move the folder to archive
    let repo_dir = Path::new(REPO_DIR).join(repo_name.clone());
    if repo_dir.exists() {
        let new_path = archive_dir.join(repo_name.clone());
        std::fs::rename(repo_dir, new_path).expect("Failed to move repo to archive");
    }

    let response = format!("Archived {}", repo_name);
    ctx.say(response).await?;
    Ok(())
}
