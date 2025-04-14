use crate::git::{GitRepoBuilder, extract_dir_from_url};
use crate::medusa::MedusaState;
use crate::{Context, Error, REPO_DIR};
use std::fs::OpenOptions;
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

    let repo = GitRepoBuilder::new(repo_url.clone())
        .branch(branch.clone())
        .build();

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

        let repo_with_branch = if branch.is_some() {
            format!("{}:{}", repo_url, branch.clone().unwrap())
        } else {
            repo_url.clone()
        };

        repos_file
            .write_all(repo_with_branch.as_bytes())
            .expect("Failed to write to repos.txt");
    }

    let dir = Path::new(REPO_DIR).join(repo.name());

    if dir.exists() {
        repo.git_pull()?;
    } else {
        repo.git_clone()?;
    }

    // check if already running - same scope to avoid race and starting twice if spamming /start
    let response = {
        let medusa = ctx.data().medusa_handler.lock().await;
        let process_state = medusa.get_process_state(repo.name()).await; // read-only

        match process_state {
            Ok(MedusaState::Running { pid }) => {
                format!(
                    "Fuzzing campaign already running for {} (PID: {})",
                    repo.name(),
                    pid
                )
            }
            Ok(_) => {
                let medusa = ctx.data().medusa_handler.lock().await;
                medusa.run_medusa(repo.name()).await?;
                format!("Fuzzing campaign started for {}", repo.name())
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

    let response = format!("Ok");
    ctx.say(response).await?;
    Ok(())
}

/// Return the status of all campaigns
#[poise::command(slash_command)]
pub async fn status(ctx: Context<'_>) -> Result<(), Error> {
    // mutex inside {} avoiding locking in await
    let response = {
        let repos = ctx.data().repos.lock().await;
        format!(
            "Currently {} campaigns: \n{}",
            repos.len(),
            repos
                .iter()
                .map(|r| r.name())
                .collect::<Vec<String>>()
                .join(",\n")
        )
    };

    // loop over all campaigns
    // for each, collect the stats

    ctx.say(response).await?;
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

    let response = format!("Ok");
    ctx.say(response).await?;
    Ok(())
}
