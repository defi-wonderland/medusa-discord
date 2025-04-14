#![warn(clippy::str_to_string)]

mod commands;
mod git;
mod medusa;
use crate::{git::GitRepo, medusa::MedusaHandler};

use poise::serenity_prelude as serenity;
use std::io::BufRead;
use std::{collections::HashMap, env::var, fs::OpenOptions, path::Path, sync::Arc, time::Duration};
use tokio::process::Child;
use tokio::sync::Mutex;

// Types used by all command functions
type Error = Box<dyn std::error::Error + Send + Sync>;
type Context<'a> = poise::Context<'a, Data, Error>;

const REPO_DIR: &str = "repos";
const MEDUSA_TIMEOUT: &str = "3600";
// Data shared between all commands
pub struct Data {
    /// List of all active repo
    repos: Mutex<Vec<GitRepo>>,
    /// All the medusa processes with their current state
    medusa_handler: Arc<Mutex<MedusaHandler>>,
}

async fn on_error(error: poise::FrameworkError<'_, Data, Error>) {
    match error {
        poise::FrameworkError::Setup { error, .. } => panic!("Failed to start bot: {:?}", error),
        poise::FrameworkError::Command { error, ctx, .. } => {
            println!("Error in command `{}`: {:?}", ctx.command().name, error,);

            ctx.say(format!(
                "Error in command `{}`: {:?}",
                ctx.command().name,
                error
            ))
            .await
            .unwrap();
        }
        error => {
            if let Err(e) = poise::builtins::on_error(error).await {
                println!("Error while handling error: {}", e)
            }
        }
    }
}

/// Loads the repos from the repos.txt file or create an empty file
fn load_repos() -> Mutex<Vec<GitRepo>> {
    let mut repos = Vec::new();

    if !Path::new(REPO_DIR).exists() {
        std::fs::create_dir(REPO_DIR).expect("Failed to create repos directory");
    }

    let path = Path::new(REPO_DIR).join("repos.txt");

    let repos_file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .open(path)
        .expect("Failed to open or create repos.txt");

    let repos_reader = std::io::BufReader::new(repos_file);

    for line in repos_reader.lines() {
        match line {
            Ok(repo) => repos.push(GitRepo::new(repo.to_string())),
            Err(e) => eprintln!("Error reading line from repos.txt: {}", e),
        }
    }

    Mutex::new(repos)
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();
    dotenvy::dotenv().expect("Failed to load .env file");

    let options = poise::FrameworkOptions {
        commands: vec![
            commands::help(),
            commands::start(),
            commands::pause(),
            commands::stop(),
            commands::status(),
        ],
        prefix_options: poise::PrefixFrameworkOptions {
            edit_tracker: Some(Arc::new(poise::EditTracker::for_timespan(
                Duration::from_secs(3600),
            ))), // todo: not sure bot should track edits
            ..Default::default()
        },
        on_error: |error| Box::pin(on_error(error)),
        pre_command: |ctx| {
            Box::pin(async move {
                println!("Executing command {}...", ctx.command().qualified_name);
            })
        },
        post_command: |ctx| {
            Box::pin(async move {
                println!("Executed command {}!", ctx.command().qualified_name);
            })
        },
        // From the poise docs, maybe keep to authorize only some users?
        command_check: Some(|ctx| {
            Box::pin(async move {
                if ctx.author().id == 123456789 {
                    return Ok(false);
                }
                Ok(true)
            })
        }),
        skip_checks_for_owners: false,
        event_handler: |_ctx, event, _framework, _data| {
            Box::pin(async move {
                println!(
                    "Got an event in event handler: {:?}",
                    event.snake_case_name()
                );
                Ok(())
            })
        },
        ..Default::default()
    };

    let framework = poise::Framework::builder()
        .setup(move |ctx, _ready, framework| {
            Box::pin(async move {
                println!("Logged in as {}", _ready.user.name);
                poise::builtins::register_globally(ctx, &framework.options().commands).await?;

                let repos = load_repos();
                let medusa_handler = MedusaHandler::new();

                medusa_handler.start_all(repos.lock().await.clone()).await?;

                Ok(Data {
                    repos,
                    medusa_handler: Arc::new(Mutex::new(medusa_handler)),
                })
            })
        })
        .options(options)
        .build();

    let token = var("DISCORD_TOKEN")
        .expect("Missing `DISCORD_TOKEN` env var, see README for more information.");

    let intents =
        serenity::GatewayIntents::non_privileged() | serenity::GatewayIntents::MESSAGE_CONTENT;

    let client = serenity::ClientBuilder::new(token, intents)
        .framework(framework)
        .await;

    client.unwrap().start().await.unwrap()
}
