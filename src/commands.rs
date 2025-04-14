use crate::git::GitRepo;
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
                medusa.run_medusa(repo.clone()).await?;
                format!("Fuzzing campaign started for {}", repo)
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
        medusa.stop_process(repo_name).await?;
    }

    let response = format!("Ok");
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

    ctx.say(repo_list).await?;
    ctx.say(
        medusa_status
            .iter()
            .map(|s| s.as_str())
            .collect::<Vec<&str>>()
            .join(",\n"),
    )
    .await?;

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
