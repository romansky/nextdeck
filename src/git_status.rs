use std::path::PathBuf;

use tokio::process::Command;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct GitStatus {
    pub branch: String,
    pub unstaged: DiffStat,
    pub staged: DiffStat,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct DiffStat {
    pub added: usize,
    pub deleted: usize,
}

impl GitStatus {
    pub fn unknown() -> Self {
        Self {
            branch: "-".to_owned(),
            unstaged: DiffStat::default(),
            staged: DiffStat::default(),
        }
    }
}

pub async fn load(cwd: Option<PathBuf>) -> GitStatus {
    if !is_git_worktree(cwd.clone()).await {
        return GitStatus::unknown();
    }

    GitStatus {
        branch: branch(cwd.clone()).await,
        unstaged: diff_stat(cwd.clone(), false).await,
        staged: diff_stat(cwd, true).await,
    }
}

async fn is_git_worktree(cwd: Option<PathBuf>) -> bool {
    let output = git(cwd, ["rev-parse", "--is-inside-work-tree"]).await;
    output
        .as_deref()
        .map(str::trim)
        .is_some_and(|value| value == "true")
}

async fn branch(cwd: Option<PathBuf>) -> String {
    if let Some(branch) = git(cwd.clone(), ["branch", "--show-current"]).await {
        let branch = branch.trim();
        if !branch.is_empty() {
            return branch.to_owned();
        }
    }

    git(cwd, ["rev-parse", "--short", "HEAD"])
        .await
        .map(|head| head.trim().to_owned())
        .filter(|head| !head.is_empty())
        .unwrap_or_else(|| "-".to_owned())
}

async fn diff_stat(cwd: Option<PathBuf>, staged: bool) -> DiffStat {
    let args = if staged {
        ["diff", "--cached", "--numstat"]
    } else {
        ["diff", "--numstat", ""]
    };
    let Some(output) = git(cwd, args.into_iter().filter(|arg| !arg.is_empty())).await else {
        return DiffStat::default();
    };
    parse_numstat(&output)
}

async fn git<I, S>(cwd: Option<PathBuf>, args: I) -> Option<String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<std::ffi::OsStr>,
{
    let mut command = Command::new("git");
    if let Some(cwd) = cwd {
        command.current_dir(cwd);
    }
    command.args(args);
    let output = command.output().await.ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).to_string())
}

fn parse_numstat(output: &str) -> DiffStat {
    let mut stat = DiffStat::default();
    for line in output.lines() {
        let mut fields = line.split('\t');
        stat.added += parse_count(fields.next());
        stat.deleted += parse_count(fields.next());
    }
    stat
}

fn parse_count(value: Option<&str>) -> usize {
    value
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_numstat_counts() {
        assert_eq!(
            parse_numstat("10\t2\tsrc/a.rs\n-\t-\timage.png\n3\t0\tREADME.md\n"),
            DiffStat {
                added: 13,
                deleted: 2,
            }
        );
    }
}
