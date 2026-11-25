use std::{env, path::Path, process::Command, time::Duration};

use serde_json::Value;

use crate::{
    child_process::{background_command, output_with_timeout},
    models::QuotaWindow,
};

use super::{mapper::parse_quota_summary, AntigravityError};

pub(super) fn fetch_quotas() -> Option<Result<Vec<QuotaWindow>, AntigravityError>> {
    let path = env::var_os("PATH")?;
    let executable = if cfg!(windows) { "agy.exe" } else { "agy" };
    let binary = env::split_paths(&path)
        .filter(|directory| directory.is_absolute())
        .map(|directory| directory.join(executable))
        .find(|candidate| candidate.is_file())?;
    let work_dir = match tempfile::tempdir() {
        Ok(directory) => directory,
        Err(_) => return Some(Err(AntigravityError::Unavailable)),
    };
    let mut command = agy_command(&binary);
    command.current_dir(work_dir.path()).arg("--version");
    let version = output_with_timeout(command, Duration::from_secs(3));
    let version = match version {
        Ok(output) if output.status.success() => output.stdout,
        _ => return Some(Err(AntigravityError::Unavailable)),
    };
    if !supports_usage_command(&String::from_utf8_lossy(&version)) {
        return None;
    }
    Some(read_report(&binary, work_dir.path()))
}

fn supports_usage_command(version: &str) -> bool {
    // Older versions may interpret /usage as a model prompt in print mode.
    let mut parts = version.trim().split('.').map(str::parse::<u32>);
    let (Some(Ok(major)), Some(Ok(minor)), Some(Ok(patch))) =
        (parts.next(), parts.next(), parts.next())
    else {
        return false;
    };
    parts.next().is_none() && (major, minor, patch) >= (1, 1, 11)
}

fn agy_command(binary: &Path) -> Command {
    let mut command = background_command(binary);
    // The updater can open a console on Windows even when agy runs without one.
    command.env("AGY_CLI_DISABLE_AUTO_UPDATE", "true");
    command
}

fn read_report(binary: &Path, work_dir: &Path) -> Result<Vec<QuotaWindow>, AntigravityError> {
    let mut command = agy_command(binary);
    command.current_dir(work_dir).args([
        "-p",
        "/usage",
        "--output-format",
        "json",
        "--print-timeout",
        "15s",
        "--settings",
        r#"{"disableSlashCommands":false}"#,
    ]);
    let output = output_with_timeout(command, Duration::from_secs(20))
        .map_err(|_| AntigravityError::Unavailable)?;
    if !output.status.success() {
        return Err(AntigravityError::Unavailable);
    }
    let report: Value =
        serde_json::from_slice(&output.stdout).map_err(|_| AntigravityError::Unavailable)?;
    if report["status"] != "SUCCESS" || report["command"]["name"] != "usage" {
        return Err(AntigravityError::Unavailable);
    }
    parse_quota_summary(&report["command"]["data"])
        .filter(|quotas| !quotas.is_empty())
        .ok_or(AntigravityError::Unavailable)
}

#[cfg(test)]
mod tests {
    use super::supports_usage_command;

    #[test]
    fn print_usage_requires_a_cli_version_that_recognizes_the_command() {
        assert!(!supports_usage_command("1.1.10"));
        assert!(!supports_usage_command("unknown"));
        assert!(supports_usage_command("1.1.11\r\n"));
        assert!(supports_usage_command("1.5.15"));
    }
}
