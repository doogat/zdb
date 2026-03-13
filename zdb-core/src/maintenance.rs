use std::path::Path;
use std::sync::OnceLock;
use std::time::Instant;

use crate::error::{Result, ZettelError};
use crate::git_ops::GitRepo;
use crate::types::MaintenanceReport;

static GIT_MAINTENANCE_AVAILABLE: OnceLock<bool> = OnceLock::new();

fn probe_git_maintenance() -> bool {
    *GIT_MAINTENANCE_AVAILABLE.get_or_init(|| {
        std::process::Command::new("git")
            .args(["maintenance", "run", "-h"])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    })
}

pub fn run(repo_path: &Path, tasks: Option<&[&str]>) -> Result<MaintenanceReport> {
    let start = Instant::now();
    let has_maintenance = probe_git_maintenance();

    let (output, fallback_used, tasks_run) = if has_maintenance {
        let mut args = vec!["maintenance", "run"];
        let task_args: Vec<String>;
        if let Some(task_list) = tasks {
            task_args = task_list.iter().map(|t| format!("--task={t}")).collect();
            for arg in &task_args {
                args.push(arg);
            }
        } else {
            args.push("--auto");
        }
        let label: Vec<String> = args[2..].iter().map(|s| s.to_string()).collect();
        let out = std::process::Command::new("git")
            .args(&args)
            .current_dir(repo_path)
            .output()
            .map_err(ZettelError::Io)?;
        (out, false, label)
    } else {
        let out = std::process::Command::new("git")
            .args(["gc", "--auto"])
            .current_dir(repo_path)
            .output()
            .map_err(ZettelError::Io)?;
        (out, true, vec!["gc --auto".to_string()])
    };

    let success = output.status.success();
    if !success {
        let stderr = String::from_utf8_lossy(&output.stderr);
        tracing::warn!(stderr = %stderr, "git maintenance failed");
    }

    Ok(MaintenanceReport {
        tasks_run,
        success,
        duration_ms: start.elapsed().as_millis() as u64,
        fallback_used,
    })
}

pub fn maybe_auto_run(repo: &GitRepo) {
    let config = match repo.load_config() {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!(error = %e, "auto-maintenance skipped: failed to load config");
            return;
        }
    };
    if !config.maintenance.auto_enabled {
        return;
    }
    match run(&repo.path, None) {
        Ok(report) => {
            tracing::info!(
                success = report.success,
                duration_ms = report.duration_ms,
                fallback = report.fallback_used,
                "auto-maintenance completed"
            );
        }
        Err(e) => {
            tracing::warn!(error = %e, "auto-maintenance failed");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn probe_returns_consistent_result() {
        let a = probe_git_maintenance();
        let b = probe_git_maintenance();
        assert_eq!(a, b);
    }

    #[test]
    fn run_succeeds_on_temp_repo() {
        let dir = tempfile::tempdir().unwrap();
        std::process::Command::new("git")
            .args(["init"])
            .current_dir(dir.path())
            .output()
            .unwrap();
        // Create an initial commit so the repo is non-empty.
        std::process::Command::new("git")
            .args(["commit", "--allow-empty", "-m", "init"])
            .current_dir(dir.path())
            .output()
            .unwrap();

        let report = run(dir.path(), None).unwrap();
        assert!(report.success);
    }

    #[test]
    fn run_with_deleted_dir_returns_error() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().to_path_buf();
        drop(dir); // delete the directory
        let result = run(&path, None);
        assert!(result.is_err());
    }

    #[test]
    fn run_reports_fallback_field() {
        let dir = tempfile::tempdir().unwrap();
        std::process::Command::new("git")
            .args(["init"])
            .current_dir(dir.path())
            .output()
            .unwrap();
        std::process::Command::new("git")
            .args(["commit", "--allow-empty", "-m", "init"])
            .current_dir(dir.path())
            .output()
            .unwrap();
        let report = run(dir.path(), None).unwrap();
        // fallback_used reflects whether git maintenance subcommand was available
        assert_eq!(report.fallback_used, !probe_git_maintenance());
    }

    #[test]
    fn run_with_explicit_task() {
        let dir = tempfile::tempdir().unwrap();
        std::process::Command::new("git")
            .args(["init"])
            .current_dir(dir.path())
            .output()
            .unwrap();
        std::process::Command::new("git")
            .args(["commit", "--allow-empty", "-m", "init"])
            .current_dir(dir.path())
            .output()
            .unwrap();
        let report = run(dir.path(), Some(&["commit-graph"]));
        // If git maintenance is available, explicit task should work.
        // If not (fallback), it falls back to gc --auto regardless.
        if probe_git_maintenance() {
            let report = report.unwrap();
            assert!(!report.fallback_used);
        }
    }

    #[test]
    fn maybe_auto_run_skips_when_disabled() {
        let dir = tempfile::tempdir().unwrap();
        let repo = GitRepo::init(dir.path()).unwrap();
        // Default config has auto_enabled = false; this should be a no-op.
        maybe_auto_run(&repo);
    }

    #[test]
    fn maybe_auto_run_runs_when_enabled() {
        let dir = tempfile::tempdir().unwrap();
        let repo = GitRepo::init(dir.path()).unwrap();
        let toml = "[maintenance]\nauto_enabled = true\n";
        repo.commit_file(".zetteldb.toml", toml, "enable maintenance")
            .unwrap();
        maybe_auto_run(&repo);
    }
}
