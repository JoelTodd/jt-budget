use std::fs;
use std::io::{self, IsTerminal, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::Duration;

use anyhow::{Context, Result, anyhow, bail, ensure};

use crate::cli::SetupArgs;
use crate::locator::RepoLocator;
use crate::repository::Repository;

const DEFAULT_GITHUB_HOST: &str = "github.com";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SetupTarget {
    CreateNew,
    AdoptExisting,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SetupMode {
    GithubCreate,
    GithubConnect,
    LocalOnly,
    AdoptLocal,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct GithubRepoRef {
    owner: String,
    name: String,
}

impl GithubRepoRef {
    fn name_with_owner(&self) -> String {
        format!("{}/{}", self.owner, self.name)
    }

    fn https_remote(&self) -> String {
        format!(
            "https://{}/{}/{}.git",
            DEFAULT_GITHUB_HOST, self.owner, self.name
        )
    }

    fn matches_remote(&self, remote: &str) -> bool {
        github_remote_name_with_owner(remote)
            .is_some_and(|name_with_owner| name_with_owner == self.name_with_owner())
    }
}

/// Runs the repository setup flow and returns the validated repo path.
///
/// # Errors
///
/// Returns an error when the repository path cannot be determined, setup
/// cannot complete, or the repository gate rejects the final repo state.
pub fn run_setup(locator: &RepoLocator, args: &SetupArgs) -> Result<PathBuf> {
    let interactive = io::stdin().is_terminal() && io::stdout().is_terminal();
    match resolve_setup_mode(args, interactive)? {
        SetupMode::GithubCreate => run_github_create_setup(locator, args, interactive),
        SetupMode::GithubConnect => run_github_connect_setup(locator, args, interactive),
        SetupMode::LocalOnly => run_local_only_setup(locator, args, interactive),
        SetupMode::AdoptLocal => run_adopt_local_setup(locator, args, interactive),
    }
}

/// Creates or adopts a repository, validates it through the normal gate, and
/// persists the locator only after that validation succeeds.
///
/// # Errors
///
/// Returns an error when the target path is unsuitable, repository creation or
/// adoption fails, or locator persistence cannot be completed.
pub fn prepare_repository(
    locator: &RepoLocator,
    repo: PathBuf,
    remote: Option<&str>,
) -> Result<PathBuf> {
    match classify_setup_target(&repo)? {
        SetupTarget::CreateNew => {
            Repository::init(&repo, remote)
                .with_context(|| format!("initialising repository `{}`", repo.display()))?;
        }
        SetupTarget::AdoptExisting => {
            if let Some(remote) = remote {
                Repository::connect_remote(&repo, remote).with_context(|| {
                    format!("configuring origin remote for `{}`", repo.display())
                })?;
            } else if let Some(existing_remote) = Repository::origin_remote_url(&repo)? {
                Repository::connect_remote(&repo, &existing_remote).with_context(|| {
                    format!(
                        "repairing origin tracking for existing repository `{}`",
                        repo.display()
                    )
                })?;
            }
        }
    }

    validate_and_save_repository(locator, &repo)
}

fn resolve_setup_mode(args: &SetupArgs, interactive: bool) -> Result<SetupMode> {
    if args.github_repo.is_some() && !args.github_create && !args.github_connect {
        bail!("`--github-repo` requires `--github-create` or `--github-connect`");
    }

    if args.github_create {
        return Ok(SetupMode::GithubCreate);
    }
    if args.github_connect {
        return Ok(SetupMode::GithubConnect);
    }
    if args.local_only {
        return Ok(SetupMode::LocalOnly);
    }
    if args.adopt_local {
        return Ok(SetupMode::AdoptLocal);
    }

    if args.repo.is_some() || args.remote.is_some() {
        return Ok(SetupMode::AdoptLocal);
    }

    if !interactive {
        bail!(
            "setup requires an explicit mode when not running interactively; try `--github-create`, `--github-connect`, `--local-only`, or `--adopt-local`"
        );
    }

    prompt_for_setup_mode()
}

fn run_github_create_setup(
    locator: &RepoLocator,
    args: &SetupArgs,
    interactive: bool,
) -> Result<PathBuf> {
    ensure_github_tooling_ready(interactive)?;

    let repo = resolve_repo_path(args.repo.as_deref(), interactive)?;
    let github_login = authenticated_github_login()?;
    let github_repo = resolve_github_create_repo(
        args.github_repo.as_deref(),
        interactive,
        &github_login,
        &repo,
    )?;

    prepare_github_created_repository(locator, repo, &github_repo)
}

fn run_github_connect_setup(
    locator: &RepoLocator,
    args: &SetupArgs,
    interactive: bool,
) -> Result<PathBuf> {
    ensure_github_tooling_ready(interactive)?;

    let github_login = authenticated_github_login()?;
    let github_repo =
        resolve_github_connect_repo(args.github_repo.as_deref(), interactive, &github_login)?;
    let repo = resolve_repo_path(args.repo.as_deref(), interactive)?;

    prepare_github_cloned_repository(locator, repo, &github_repo)
}

fn run_local_only_setup(
    locator: &RepoLocator,
    args: &SetupArgs,
    interactive: bool,
) -> Result<PathBuf> {
    let repo = resolve_repo_path(args.repo.as_deref(), interactive)?;
    prepare_repository(locator, repo, None)
}

fn run_adopt_local_setup(
    locator: &RepoLocator,
    args: &SetupArgs,
    interactive: bool,
) -> Result<PathBuf> {
    let repo = resolve_repo_path(args.repo.as_deref(), interactive)?;
    let target = classify_setup_target(&repo)?;
    let remote = resolve_remote(args.remote.as_deref(), interactive, &repo, target)?;
    prepare_repository(locator, repo, remote.as_deref())
}

fn prepare_github_created_repository(
    locator: &RepoLocator,
    repo: PathBuf,
    github_repo: &GithubRepoRef,
) -> Result<PathBuf> {
    match classify_setup_target(&repo)? {
        SetupTarget::CreateNew => {
            Repository::init(&repo, None)
                .with_context(|| format!("initialising repository `{}`", repo.display()))?;
        }
        SetupTarget::AdoptExisting => {
            if let Some(existing_remote) = Repository::origin_remote_url(&repo)? {
                ensure!(
                    github_repo.matches_remote(&existing_remote),
                    "existing budget repo `{}` already has origin remote `{}`",
                    repo.display(),
                    existing_remote
                );
            }
        }
    }

    create_github_repository(github_repo).with_context(|| {
        format!(
            "creating private GitHub repo `{}`",
            github_repo.name_with_owner()
        )
    })?;
    connect_github_remote_with_retry(&repo, github_repo).with_context(|| {
        format!(
            "connecting `{}` to GitHub repo `{}`",
            repo.display(),
            github_repo.name_with_owner()
        )
    })?;

    validate_and_save_repository(locator, &repo)
}

fn prepare_github_cloned_repository(
    locator: &RepoLocator,
    repo: PathBuf,
    github_repo: &GithubRepoRef,
) -> Result<PathBuf> {
    verify_github_repository_accessible(github_repo).with_context(|| {
        format!(
            "checking access to GitHub repo `{}`",
            github_repo.name_with_owner()
        )
    })?;
    Repository::clone_from_remote(&github_repo.https_remote(), &repo).with_context(|| {
        format!(
            "cloning GitHub repo `{}` into `{}`",
            github_repo.name_with_owner(),
            repo.display()
        )
    })?;

    validate_and_save_repository(locator, &repo)
}

fn validate_and_save_repository(locator: &RepoLocator, repo: &Path) -> Result<PathBuf> {
    let validated = Repository::open(repo).with_context(|| {
        format!(
            "validating repository `{}` through the repository gate",
            repo.display()
        )
    })?;
    let canonical_repo = validated.root().to_path_buf();
    drop(validated);
    locator.save(&canonical_repo)?;
    Ok(canonical_repo)
}

fn resolve_repo_path(explicit_repo: Option<&Path>, interactive: bool) -> Result<PathBuf> {
    if let Some(repo) = explicit_repo {
        ensure!(
            !repo.as_os_str().is_empty(),
            "repository path cannot be empty"
        );
        return Ok(repo.to_path_buf());
    }

    if !interactive {
        bail!("setup requires `--repo` when not running interactively");
    }

    prompt_for_repo_path()
}

fn resolve_github_create_repo(
    explicit_repo: Option<&str>,
    interactive: bool,
    default_owner: &str,
    local_repo: &Path,
) -> Result<GithubRepoRef> {
    if let Some(repo) = explicit_repo {
        return parse_github_repo_ref(repo, default_owner);
    }

    let default = GithubRepoRef {
        owner: default_owner.to_owned(),
        name: default_github_repo_name(local_repo),
    };

    if !interactive {
        return Ok(default);
    }

    prompt_for_github_repo("GitHub repository", &default, default_owner)
}

fn resolve_github_connect_repo(
    explicit_repo: Option<&str>,
    interactive: bool,
    default_owner: &str,
) -> Result<GithubRepoRef> {
    if let Some(repo) = explicit_repo {
        return parse_github_repo_ref(repo, default_owner);
    }

    if !interactive {
        bail!("`--github-connect` requires `--github-repo OWNER/NAME`");
    }

    let default = GithubRepoRef {
        owner: default_owner.to_owned(),
        name: "budget".to_owned(),
    };
    prompt_for_github_repo("GitHub repository or URL", &default, default_owner)
}

fn resolve_remote(
    explicit_remote: Option<&str>,
    interactive: bool,
    repo: &Path,
    target: SetupTarget,
) -> Result<Option<String>> {
    if let Some(remote) = explicit_remote {
        let trimmed = remote.trim();
        ensure!(!trimmed.is_empty(), "remote cannot be empty");
        return Ok(Some(trimmed.to_owned()));
    }

    if !interactive {
        return Ok(None);
    }

    if matches!(target, SetupTarget::AdoptExisting) && Repository::has_origin_remote(repo)? {
        return Ok(None);
    }

    prompt_for_optional_remote()
}

fn prompt_for_setup_mode() -> Result<SetupMode> {
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

fn prompt_for_repo_path() -> Result<PathBuf> {
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

fn prompt_for_optional_remote() -> Result<Option<String>> {
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

fn prompt_for_github_repo(
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

fn default_github_repo_name(local_repo: &Path) -> String {
    let raw = local_repo
        .file_name()
        .and_then(|value| value.to_str())
        .filter(|value| !value.is_empty())
        .unwrap_or("budget");

    let mut name = String::new();
    let mut last_dash = false;
    for character in raw.chars() {
        let mapped = if character.is_ascii_alphanumeric() {
            character.to_ascii_lowercase()
        } else if matches!(character, '-' | '_' | '.') {
            character
        } else {
            '-'
        };

        if mapped == '-' {
            if name.is_empty() || last_dash {
                continue;
            }
            last_dash = true;
        } else {
            last_dash = false;
        }

        name.push(mapped);
    }

    let trimmed = name.trim_matches(|character| matches!(character, '-' | '_' | '.'));
    if trimmed.is_empty() {
        "budget".to_owned()
    } else {
        trimmed.to_owned()
    }
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

fn parse_github_repo_ref(input: &str, default_owner: &str) -> Result<GithubRepoRef> {
    let trimmed = input.trim();
    ensure!(!trimmed.is_empty(), "GitHub repository cannot be blank");

    if (trimmed.contains("://") || trimmed.starts_with("git@")) && !trimmed.contains("github.com") {
        bail!("only github.com repositories are supported in the GitHub setup flow");
    }

    let trimmed = trimmed
        .strip_prefix("https://github.com/")
        .or_else(|| trimmed.strip_prefix("http://github.com/"))
        .or_else(|| trimmed.strip_prefix("ssh://git@github.com/"))
        .or_else(|| trimmed.strip_prefix("git@github.com:"))
        .or_else(|| trimmed.strip_prefix("github.com/"))
        .unwrap_or(trimmed)
        .trim_end_matches('/')
        .trim_end_matches(".git");

    let parts: Vec<_> = trimmed.split('/').filter(|part| !part.is_empty()).collect();
    let (owner, name) = match parts.as_slice() {
        [name] => (default_owner, *name),
        [owner, name] => (*owner, *name),
        _ => bail!("GitHub repository must be in `OWNER/NAME` form"),
    };

    ensure!(
        !owner.trim().is_empty(),
        "GitHub repository owner cannot be blank"
    );
    ensure!(
        !name.trim().is_empty(),
        "GitHub repository name cannot be blank"
    );

    Ok(GithubRepoRef {
        owner: owner.to_owned(),
        name: name.to_owned(),
    })
}

fn ensure_github_tooling_ready(interactive: bool) -> Result<()> {
    ensure_command_available("git")?;
    ensure_command_available("gh")?;

    if !github_authenticated()? {
        ensure!(
            interactive,
            "GitHub CLI is not authenticated; run `gh auth login` or use interactive setup"
        );
        println!("GitHub CLI is not authenticated. Starting `gh auth login`...");
        run_command_inherit("gh", &["auth", "login"])?;
    }

    run_gh(&["auth", "setup-git", "--hostname", DEFAULT_GITHUB_HOST])
        .context("configuring git to use GitHub CLI credentials")?;
    Ok(())
}

fn authenticated_github_login() -> Result<String> {
    let login = run_gh(&["api", "user", "--jq", ".login"]).context("reading GitHub login")?;
    ensure!(!login.trim().is_empty(), "GitHub login cannot be blank");
    Ok(login)
}

fn create_github_repository(repo: &GithubRepoRef) -> Result<()> {
    let name_with_owner = repo.name_with_owner();
    match run_gh(&[
        "repo",
        "create",
        &name_with_owner,
        "--private",
        "--disable-issues",
        "--disable-wiki",
    ]) {
        Ok(_) => Ok(()),
        Err(error) if github_repo_already_exists(&error) => {
            verify_github_repository_accessible(repo)
                .with_context(|| format!("verifying existing GitHub repo `{name_with_owner}`"))?;
            Ok(())
        }
        Err(error) => Err(error),
    }
}

fn verify_github_repository_accessible(repo: &GithubRepoRef) -> Result<()> {
    let name_with_owner = repo.name_with_owner();
    run_gh(&["repo", "view", &name_with_owner, "--json", "nameWithOwner"]).map(|_| ())
}

fn connect_github_remote_with_retry(repo: &Path, github_repo: &GithubRepoRef) -> Result<()> {
    const MAX_ATTEMPTS: usize = 5;

    let remote = github_remote_for_connection(repo, github_repo)?;
    let mut last_error = None;
    for attempt in 1..=MAX_ATTEMPTS {
        match Repository::connect_remote(repo, &remote) {
            Ok(()) => return Ok(()),
            Err(error) if attempt < MAX_ATTEMPTS && github_repo_not_ready(&error) => {
                last_error = Some(error);
                thread::sleep(Duration::from_secs(1));
            }
            Err(error) => return Err(error),
        }
    }

    Err(last_error.expect("GitHub connection retry should preserve the last error"))
}

fn github_repo_not_ready(error: &anyhow::Error) -> bool {
    error
        .chain()
        .any(|cause| cause.to_string().contains("Repository not found"))
}

fn github_repo_already_exists(error: &anyhow::Error) -> bool {
    error.chain().any(|cause| {
        let text = cause.to_string();
        text.contains("Name already exists on this account") || text.contains("already exists")
    })
}

fn github_remote_for_connection(repo: &Path, github_repo: &GithubRepoRef) -> Result<String> {
    if let Some(existing_remote) = Repository::origin_remote_url(repo)? {
        if github_repo.matches_remote(&existing_remote) {
            return Ok(existing_remote);
        }
    }
    Ok(github_repo.https_remote())
}

fn github_remote_name_with_owner(remote: &str) -> Option<String> {
    let trimmed = remote.trim().trim_end_matches('/').trim_end_matches(".git");

    let stripped = trimmed
        .strip_prefix("https://github.com/")
        .or_else(|| trimmed.strip_prefix("http://github.com/"))
        .or_else(|| trimmed.strip_prefix("ssh://git@github.com/"))
        .or_else(|| trimmed.strip_prefix("git@github.com:"))
        .or_else(|| trimmed.strip_prefix("github.com/"))?;

    let mut parts = stripped.split('/').filter(|part| !part.is_empty());
    let owner = parts.next()?;
    let name = parts.next()?;
    if parts.next().is_some() {
        return None;
    }

    Some(format!("{owner}/{name}"))
}

fn ensure_command_available(program: &str) -> Result<()> {
    let status = Command::new(program)
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .with_context(|| format!("checking whether `{program}` is installed"))?;
    ensure!(status.success(), "`{program}` is not available");
    Ok(())
}

fn github_authenticated() -> Result<bool> {
    let status = Command::new("gh")
        .args(["auth", "status", "--hostname", DEFAULT_GITHUB_HOST])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .context("checking GitHub CLI authentication state")?;
    Ok(status.success())
}

fn run_gh(args: &[&str]) -> Result<String> {
    run_command("gh", args)
}

fn run_command(program: &str, args: &[&str]) -> Result<String> {
    let output = Command::new(program)
        .args(args)
        .output()
        .with_context(|| format!("running `{program} {}`", args.join(" ")))?;
    if output.status.success() {
        return Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned());
    }

    bail!(
        "{} {} failed: {}",
        program,
        args.join(" "),
        String::from_utf8_lossy(&output.stderr).trim()
    );
}

fn run_command_inherit(program: &str, args: &[&str]) -> Result<()> {
    let status = Command::new(program)
        .args(args)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()
        .with_context(|| format!("running `{program} {}`", args.join(" ")))?;
    ensure!(status.success(), "`{program} {}` failed", args.join(" "));
    Ok(())
}

fn classify_setup_target(repo: &Path) -> Result<SetupTarget> {
    if !repo.exists() {
        return Ok(SetupTarget::CreateNew);
    }

    ensure!(
        repo.is_dir(),
        "repository path `{}` must be a directory",
        repo.display()
    );

    if fs::read_dir(repo)
        .with_context(|| format!("reading `{}`", repo.display()))?
        .next()
        .transpose()?
        .is_none()
    {
        return Ok(SetupTarget::CreateNew);
    }

    if Repository::looks_like_budget_repo(repo)? {
        return Ok(SetupTarget::AdoptExisting);
    }

    bail!(
        "repository path `{}` must be missing, empty, or an existing jt-budget repository",
        repo.display()
    );
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::Path;

    use anyhow::anyhow;
    use tempfile::tempdir;

    use super::{
        GithubRepoRef, SetupMode, default_github_repo_name, github_remote_name_with_owner,
        github_repo_not_ready, parse_github_repo_ref, prepare_repository, resolve_setup_mode,
    };
    use crate::cli::SetupArgs;
    use crate::locator::RepoLocator;
    use crate::repository::Repository;

    #[test]
    fn prepare_repository_creates_new_repo_and_saves_locator() {
        let temp = tempdir().unwrap();
        let repo = temp.path().join("budget");
        let locator = RepoLocator::new(temp.path().join("config/jt-budget/config.toml"));

        let canonical_repo = prepare_repository(&locator, repo.clone(), None).unwrap();

        assert_eq!(canonical_repo, repo.canonicalize().unwrap());
        assert_eq!(locator.load().unwrap(), Some(canonical_repo));
        assert!(repo.join("config.toml").exists());
    }

    #[test]
    fn prepare_repository_adopts_existing_repo_and_saves_locator() {
        let temp = tempdir().unwrap();
        let repo = temp.path().join("budget");
        let locator = RepoLocator::new(temp.path().join("config/jt-budget/config.toml"));
        Repository::init(&repo, None).unwrap();

        let canonical_repo = prepare_repository(&locator, repo.clone(), None).unwrap();

        assert_eq!(canonical_repo, repo.canonicalize().unwrap());
        assert_eq!(locator.load().unwrap(), Some(canonical_repo));
    }

    #[test]
    fn prepare_repository_rejects_occupied_non_budget_path() {
        let temp = tempdir().unwrap();
        let repo = temp.path().join("budget");
        let locator = RepoLocator::new(temp.path().join("config/jt-budget/config.toml"));
        fs::create_dir_all(&repo).unwrap();
        fs::write(repo.join("notes.txt"), "not a budget repo").unwrap();

        let error = prepare_repository(&locator, repo, None).unwrap_err();
        assert!(
            error
                .to_string()
                .contains("must be missing, empty, or an existing jt-budget repository")
        );
    }

    #[test]
    fn parse_github_repo_ref_accepts_owner_name() {
        let repo = parse_github_repo_ref("openai/budget", "fallback").unwrap();
        assert_eq!(
            repo,
            GithubRepoRef {
                owner: "openai".to_owned(),
                name: "budget".to_owned(),
            }
        );
    }

    #[test]
    fn parse_github_repo_ref_accepts_url_and_git_suffix() {
        let repo =
            parse_github_repo_ref("https://github.com/openai/budget.git", "fallback").unwrap();
        assert_eq!(repo.owner, "openai");
        assert_eq!(repo.name, "budget");
    }

    #[test]
    fn parse_github_repo_ref_uses_default_owner_for_name_only() {
        let repo = parse_github_repo_ref("budget", "fallback").unwrap();
        assert_eq!(repo.owner, "fallback");
        assert_eq!(repo.name, "budget");
    }

    #[test]
    fn default_github_repo_name_sanitises_local_path_name() {
        let repo = Path::new("/tmp/My Budget!!!");
        assert_eq!(default_github_repo_name(repo), "my-budget");
    }

    #[test]
    fn github_repo_matches_https_and_ssh_remotes() {
        let repo = GithubRepoRef {
            owner: "JoelTodd".to_owned(),
            name: "budget".to_owned(),
        };

        assert!(repo.matches_remote("https://github.com/JoelTodd/budget.git"));
        assert!(repo.matches_remote("git@github.com:JoelTodd/budget.git"));
        assert!(!repo.matches_remote("https://github.com/JoelTodd/other.git"));
    }

    #[test]
    fn github_remote_name_with_owner_rejects_non_github_paths() {
        assert_eq!(
            github_remote_name_with_owner("https://github.com/JoelTodd/budget.git"),
            Some("JoelTodd/budget".to_owned())
        );
        assert_eq!(github_remote_name_with_owner("/tmp/budget.git"), None);
    }

    #[test]
    fn resolve_setup_mode_defaults_to_legacy_local_flow_when_repo_is_explicit() {
        let mode = resolve_setup_mode(
            &SetupArgs {
                repo: Some("/tmp/budget".into()),
                remote: None,
                github_create: false,
                github_connect: false,
                local_only: false,
                adopt_local: false,
                github_repo: None,
                no_open: false,
            },
            false,
        )
        .unwrap();
        assert_eq!(mode, SetupMode::AdoptLocal);
    }

    #[test]
    fn resolve_setup_mode_rejects_github_repo_without_mode() {
        let error = resolve_setup_mode(
            &SetupArgs {
                repo: None,
                remote: None,
                github_create: false,
                github_connect: false,
                local_only: false,
                adopt_local: false,
                github_repo: Some("owner/name".to_owned()),
                no_open: false,
            },
            false,
        )
        .unwrap_err();
        assert!(
            error
                .to_string()
                .contains("`--github-repo` requires `--github-create` or `--github-connect`")
        );
    }

    #[test]
    fn github_repo_not_ready_checks_wrapped_error_chain() {
        let error = anyhow!("git push -u origin main failed: remote: Repository not found.")
            .context("publishing repository state");

        assert!(github_repo_not_ready(&error));
    }

    #[test]
    fn prepare_repository_repairs_upstream_when_origin_exists() {
        let temp = tempdir().unwrap();
        let remote = temp.path().join("remote.git");
        let repo = temp.path().join("budget");
        let locator = RepoLocator::new(temp.path().join("config/jt-budget/config.toml"));

        std::process::Command::new("git")
            .args([
                "init",
                "--bare",
                "--initial-branch=main",
                remote.to_str().unwrap(),
            ])
            .current_dir(temp.path())
            .status()
            .unwrap();
        Repository::init(&repo, None).unwrap();
        std::process::Command::new("git")
            .args(["remote", "add", "origin", remote.to_str().unwrap()])
            .current_dir(&repo)
            .status()
            .unwrap();

        let canonical_repo = prepare_repository(&locator, repo.clone(), None).unwrap();

        assert_eq!(canonical_repo, repo.canonicalize().unwrap());
        let upstream = std::process::Command::new("git")
            .args([
                "rev-parse",
                "--abbrev-ref",
                "--symbolic-full-name",
                "@{upstream}",
            ])
            .current_dir(&repo)
            .output()
            .unwrap();
        assert!(upstream.status.success());
        assert_eq!(
            String::from_utf8_lossy(&upstream.stdout).trim(),
            "origin/main"
        );
    }
}
