# Public Release Readiness Checklist

This document breaks the public-release work into categories so it can be reviewed and completed one step at a time.

Suggested order:

1. Privacy and sensitive data
2. Legal and repository metadata
3. Public-facing docs
4. Automation and repository settings
5. Contribution and support policy
6. Final go-live checks

## 1. Privacy And Sensitive Data

Goal: make sure the repository and its history do not expose private information or personal context that should stay internal.

- [x] Review `docs/mvp-brief.md` and replace personal-looking account names, payday details, and real budget amounts with neutral examples if those values reflect real life.
- [x] Review tests and examples for unnecessary personal identifiers and replace them with generic placeholders where practical.
- [x] Run a dedicated secret scan across the full Git history with a tool such as `gitleaks`.
- [x] Confirm there are no tracked local config files, logs, database files, or credentials.
- [x] Confirm the public repo will not contain actual budget data; keep real month files in a separate private budget repo.

## 2. Legal And Repository Metadata

Goal: make the repo legally usable and clearly attributable.

- [x] Choose and add a `LICENSE` file.
- [x] Decide whether the current repo name and crate names are the names you want to keep in public.
- [x] Add package metadata to the Cargo manifests if wanted for discoverability:
  repository URL, licence field, description, readme, keywords, categories.
- [x] Decide whether to keep release tags and versioning exactly as-is or tidy them before publication.

## 3. Public-Facing Docs

Goal: make the project understandable to somebody who is not already inside the workflow.

- [ ] Rewrite `README.md` for public readers rather than local development shorthand.
- [ ] Add explicit install and run instructions.
- [ ] Document system prerequisites such as Rust, `git`, and optional `gh`.
- [ ] Explain the split between this app repo and the user’s separate private budget-data repo.
- [ ] State the supported or expected environments clearly instead of relying on the WSL handoff notes.
- [ ] Decide whether `docs/app-env.md` should stay public, be rewritten as generic setup guidance, or be removed.

## 4. Automation And Repository Settings

Goal: make the public repo enforce the quality bar it already expects locally.

- [ ] Add CI to run:
  `cargo fmt --check`
  `cargo clippy --all-targets --all-features -- -D warnings`
  `cargo test`
  `cargo build --release`
- [ ] Enable branch protection for the default branch.
- [ ] Enable GitHub secret scanning if available.
- [ ] Decide whether to enable Dependabot updates and alerts.
- [ ] Decide whether issues, Discussions, Projects, and wiki should be enabled or disabled.

## 5. Contribution And Support Policy

Goal: make it clear how outside people should interact with the project.

- [ ] Decide whether you want outside contributions or whether the repo is mainly for source visibility.
- [ ] If accepting contributions, add `CONTRIBUTING.md`.
- [ ] Add `SECURITY.md` if you want a defined vulnerability reporting path.
- [ ] Add `CODE_OF_CONDUCT.md` if you want standard community expectations in place.
- [ ] Decide how support requests should be handled and document that in `README.md` or `SUPPORT.md`.

## 6. Final Go-Live Checks

Goal: confirm the repo is ready to be opened without avoidable follow-up churn.

- [ ] Re-read the README as if you were a new user.
- [ ] Re-check the top-level file list for anything that looks local, temporary, or private.
- [ ] Confirm `.gitignore` is still appropriate for public collaboration.
- [ ] Run the local verification commands again before changing visibility.
- [ ] Decide whether to make the repo public immediately or publish after the CI and doc changes are merged.

## Suggested Review Flow

Use these categories as a short sequence of passes:

1. Complete the privacy pass first.
2. Add the legal and metadata pieces next.
3. Fix the public docs.
4. Set up CI and repository settings.
5. Publish contribution and support policy.
6. Do one final review, then change visibility.
