use crate::git::{GitRepoBuilder, extract_dir_from_url};
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
        let mut repos = ctx.data().repos.lock().unwrap();

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

    // check if already running: todo

    let response = format!("Fuzzing campaign started for {}", repo.name());
    ctx.say(response).await?;
    Ok(())
}

// async fn run_medusa(repo: &str) -> Result<String, Error> {
//     //     use tokio::process::Command;
//     //     use tokio::time::{Duration, timeout};

//     let mut child = Command::new("medusa")
//         .arg("fuzz")
//         .arg("--repo")
//         .arg(repo)
//         .stdout(Stdio::null())
//         .stderr(Stdio::null())
//         .spawn()
//         .map_err(|e| format!("Failed to spawn Medusa: {e}"))?;

//     {
//         let mut map = PROCESS_MAP.lock().await;
//         map.insert(repo.to_string(), child);
//     }

//     Ok(format!("Started Medusa for {repo}"))
// }

/// Stop a given campaign (stop the fuzz but can be resumed later on using start)
#[poise::command(slash_command)]
pub async fn pause(
    ctx: Context<'_>,
    #[description = "Campaign ID (see status cmd)"] campaign_id: u32,
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
        let repos = ctx.data().repos.lock().unwrap();
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
    #[description = "Campaign ID (see status cmd)"] campaign_id: u32,
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
