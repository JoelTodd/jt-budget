use std::io::{self, Write};
use std::path::PathBuf;

use anyhow::{Context, Result, anyhow};

use super::SetupMode;
use super::github::{GithubRepoRef, parse_github_repo_ref};

pub(super) fn prompt_for_setup_mode() -> Result<SetupMode> {
    println!("What would you like to do?");
    println!("1. Create a new synced budget on GitHub");
    println!("2. Connect this machine to an existing budget on GitHub");
    println!("3. Keep this budget local only for now");
    println!("4. Advanced: use an existing local budget folder");

    loop {
        let input = prompt_line("Setup mode [1]: ")?;
        match input.trim() {
            "" | "1" => return Ok(SetupMode::GithubCreate),
            "2" => return Ok(SetupMode::GithubConnect),
            "3" => return Ok(SetupMode::LocalOnly),
            "4" => return Ok(SetupMode::AdoptLocal),
            _ => println!("Please choose 1, 2, 3, or 4."),
        }
    }
}

pub(super) fn prompt_for_repo_path() -> Result<PathBuf> {
    let suggested = default_repo_path();
    let input = prompt_line(&format!(
        "Budget repository path [{}]: ",
        suggested.display()
    ))?;
    if input.trim().is_empty() {
        return Ok(suggested);
    }
    expand_home_path(input.trim())
}

pub(super) fn prompt_for_optional_remote() -> Result<Option<String>> {
    if !prompt_yes_no("Configure an origin remote now? [y/N]: ", false)? {
        return Ok(None);
    }

    loop {
        let input = prompt_line("Origin remote URL/path: ")?;
        let trimmed = input.trim();
        if !trimmed.is_empty() {
            return Ok(Some(trimmed.to_owned()));
        }
        println!("Remote cannot be blank.");
    }
}

pub(super) fn prompt_for_github_repo(
    label: &str,
    default: &GithubRepoRef,
    default_owner: &str,
) -> Result<GithubRepoRef> {
    loop {
        let input = prompt_line(&format!("{label} [{}]: ", default.name_with_owner()))?;
        let trimmed = input.trim();
        if trimmed.is_empty() {
            return Ok(default.clone());
        }

        match parse_github_repo_ref(trimmed, default_owner) {
            Ok(repo) => return Ok(repo),
            Err(error) => println!("{error}"),
        }
    }
}

fn prompt_yes_no(prompt: &str, default: bool) -> Result<bool> {
    loop {
        let input = prompt_line(prompt)?;
        let trimmed = input.trim();
        if trimmed.is_empty() {
            return Ok(default);
        }
        match trimmed.to_ascii_lowercase().as_str() {
            "y" | "yes" => return Ok(true),
            "n" | "no" => return Ok(false),
            _ => println!("Please answer y or n."),
        }
    }
}

fn prompt_line(prompt: &str) -> Result<String> {
    print!("{prompt}");
    io::stdout().flush().context("flushing setup prompt")?;

    let mut input = String::new();
    let read = io::stdin()
        .read_line(&mut input)
        .context("reading setup input")?;
    if read == 0 {
        return Err(anyhow!("setup was cancelled before input completed"));
    }
    Ok(input)
}

fn default_repo_path() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("budget")
}

fn expand_home_path(input: &str) -> Result<PathBuf> {
    if input == "~" {
        return dirs::home_dir().ok_or_else(|| anyhow!("could not resolve home directory"));
    }

    if let Some(rest) = input.strip_prefix("~/") {
        let home = dirs::home_dir().ok_or_else(|| anyhow!("could not resolve home directory"))?;
        return Ok(home.join(rest));
    }

    Ok(PathBuf::from(input))
}
