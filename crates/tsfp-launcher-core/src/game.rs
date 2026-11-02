//! Running the game (T4351). The playable game is the hybrid build (lifted code plus Rust ports) that each user
//! builds on their own machine from their disc (AGENTS.md rule 61): in a repository checkout the launcher drives
//! `tools/dev/build.sh` and `tools/dev/play.sh`, exactly as a developer would by hand.
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};

/// A repository checkout that can build and run the game.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Repo {
    pub root: PathBuf,
}

/// The first ancestor of any start directory that holds both `tools/dev/play.sh` and `tools/dev/build.sh`.
pub fn find_repo(starts: &[PathBuf]) -> Option<Repo> {
    starts.iter().find_map(|start| {
        start
            .ancestors()
            .find(|d| {
                d.join("tools/dev/play.sh").is_file() && d.join("tools/dev/build.sh").is_file()
            })
            .map(|d| Repo {
                root: d.to_path_buf(),
            })
    })
}

/// Where the launcher looks for a checkout: beside its executable (cargo builds land under the repo) and the
/// working directory.
pub fn default_repo() -> Option<Repo> {
    let mut starts = Vec::new();
    if let Ok(exe) = std::env::current_exe() {
        starts.push(exe);
    }
    if let Ok(cwd) = std::env::current_dir() {
        starts.push(cwd);
    }
    find_repo(&starts)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GameState {
    /// No checkout and no packaged game: this launcher build cannot run the game.
    NoRuntime,
    Ready,
    /// The game must be built first; `args` go to `build.sh`.
    NeedsBuild {
        reason: String,
        args: Vec<String>,
    },
    /// No disc image for the game to read.
    NoDisc,
    Error(String),
}

/// `play.sh --dry-run` exit codes (tools/dev/play.sh) to a state.
pub fn state_from_exit(code: Option<i32>, stderr: &str) -> GameState {
    let build = |reason: &str, args: &[&str]| GameState::NeedsBuild {
        reason: reason.into(),
        args: args.iter().map(|a| a.to_string()).collect(),
    };
    match code {
        Some(0) => GameState::Ready,
        Some(3) => build("The game has not been built on this computer yet.", &[]),
        Some(6) => build("The game sources changed since the last build.", &[]),
        Some(10) => build("The lifter changed since the last build.", &["--relift"]),
        Some(7) => build(
            "The build is from another place or machine.",
            &["--reconfigure"],
        ),
        Some(4) => GameState::NoDisc,
        _ => GameState::Error(
            stderr
                .lines()
                .rev()
                .find(|l| !l.trim().is_empty())
                .unwrap_or("play.sh failed")
                .trim()
                .to_string(),
        ),
    }
}

fn script(repo: &Repo, name: &str) -> Command {
    let mut c = Command::new("sh");
    c.arg(format!("tools/dev/{name}")).current_dir(&repo.root);
    // Own process group, so stopping a build also stops the compilers it started.
    #[cfg(unix)]
    std::os::unix::process::CommandExt::process_group(&mut c, 0);
    c
}

/// Stop a process started by [`build`] or [`play`] together with its children (SIGTERM to its group).
pub fn stop(pid: u32) {
    #[cfg(unix)]
    let _ = Command::new("kill")
        .args(["-TERM", "--", &format!("-{pid}")])
        .status();
    #[cfg(not(unix))]
    let _ = Command::new("taskkill")
        .args(["/T", "/F", "/PID", &pid.to_string()])
        .status();
}

/// Ask `play.sh` whether the game can start, without starting it (~0.1 s).
pub fn probe(repo: &Repo, disc: Option<&Path>) -> GameState {
    let mut c = script(repo, "play.sh");
    c.args(["--dry-run", "default"]);
    if let Some(d) = disc {
        c.env("TSFP_DISC", d);
    }
    match c.stdout(Stdio::null()).stderr(Stdio::piped()).output() {
        Ok(out) => state_from_exit(out.status.code(), &String::from_utf8_lossy(&out.stderr)),
        Err(e) => GameState::Error(format!("cannot run play.sh: {e}")),
    }
}

fn logged(mut c: Command, log: &Path) -> std::io::Result<Child> {
    if let Some(dir) = log.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let out = std::fs::File::create(log)?;
    let err = out.try_clone()?;
    c.stdin(Stdio::null()).stdout(out).stderr(err).spawn()
}

/// Start the game (`play.sh default`); output goes to `log`.
pub fn play(repo: &Repo, disc: Option<&Path>, log: &Path) -> std::io::Result<Child> {
    let mut c = script(repo, "play.sh");
    c.arg("default");
    if let Some(d) = disc {
        c.env("TSFP_DISC", d);
    }
    logged(c, log)
}

/// Build the game (`build.sh [args]`); output goes to `log`.
pub fn build(repo: &Repo, args: &[String], log: &Path) -> std::io::Result<Child> {
    let mut c = script(repo, "build.sh");
    c.args(args);
    logged(c, log)
}

/// Last non-empty line of a log, for a one-line status.
pub fn last_line(log: &Path) -> String {
    std::fs::read_to_string(log)
        .ok()
        .and_then(|t| {
            t.lines()
                .rev()
                .find(|l| !l.trim().is_empty())
                .map(|l| l.trim().to_string())
        })
        .unwrap_or_default()
}
