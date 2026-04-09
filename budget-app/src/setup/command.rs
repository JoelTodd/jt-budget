use std::process::{Command, Stdio};

use anyhow::{Context, Result, bail, ensure};

pub(super) fn ensure_command_available(program: &str) -> Result<()> {
    let status = Command::new(program)
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .with_context(|| format!("checking whether `{program}` is installed"))?;
    ensure!(status.success(), "`{program}` is not available");
    Ok(())
}

pub(super) fn run_command(program: &str, args: &[&str]) -> Result<String> {
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

pub(super) fn run_command_inherit(program: &str, args: &[&str]) -> Result<()> {
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
