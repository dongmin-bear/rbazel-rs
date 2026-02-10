use serde::Deserialize;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

const USAGE: &str = "Usage:\n  rbazel-rs [bazel] <subcommand> [bazel-args...]\n\nExamples:\n  rbazel-rs version\n  rbazel-rs build --config=aarch64_musl //system/...:target\n  rbazel-rs bazel build //...\n\nResult artifacts will be pulled to:\n  ./_rbazel_artifacts/<branch>/<timestamp>/\n";

const REMOTE_SCRIPT: &str = r#"set -euo pipefail

REPO_DIR="$1"
CACHE_DIR="$2"
LOCAL_HEAD="$3"
HAS_TARGETS="$4"
SUBCMD="$5"
OPTS_Q="${6-}"
TARGETS_Q="${7-}"

cd "$REPO_DIR"
mkdir -p "$CACHE_DIR"

if ! git diff --quiet || ! git diff --cached --quiet; then
  echo "[rbazel][server] ERROR: server repo has local changes. Clean it first." >&2
  exit 3
fi

git fetch --all --prune >/dev/null 2>&1 || true
CURRENT_BRANCH="$(git symbolic-ref --short -q HEAD || true)"
if [[ -n "$CURRENT_BRANCH" ]]; then
  git pull --ff-only >/dev/null 2>&1 || true
fi
if git cat-file -e "${LOCAL_HEAD}^{commit}" 2>/dev/null; then
  git checkout -q --detach "$LOCAL_HEAD"
else
  echo "[rbazel][server] ERROR: commit not found on server after fetch: $LOCAL_HEAD" >&2
  exit 4
fi

STARTUP_HELP="$(bazel help startup_options 2>/dev/null || true)"
REPO_CACHE_FLAG=""
if echo "$STARTUP_HELP" | grep -q -- '--repository_cache'; then
  REPO_CACHE_FLAG="--repository_cache=${CACHE_DIR}/repo-cache"
elif echo "$STARTUP_HELP" | grep -q -- '--experimental_repository_cache'; then
  REPO_CACHE_FLAG="--experimental_repository_cache=${CACHE_DIR}/repo-cache"
fi

COMMON_STARTUP="--output_user_root=${CACHE_DIR}/bazel-out ${REPO_CACHE_FLAG}"

eval "bazel ${COMMON_STARTUP} ${SUBCMD} ${OPTS_Q} ${TARGETS_Q}"

EXECROOT="$(bazel ${COMMON_STARTUP} info execution_root)"
TMPBASE="/tmp/rbazel_${LOCAL_HEAD}_$$"
LIST="${TMPBASE}.files"
TGZ="${TMPBASE}.tgz"

if [[ "$HAS_TARGETS" == "1" ]]; then
  eval "bazel ${COMMON_STARTUP} cquery ${OPTS_Q} ${TARGETS_Q} \
    --output=starlark \
    --starlark:expr='\"\\n\".join([f.path for f in target.files.to_list()])' \
  " | sed '/^$/d' > "$LIST" || true
fi

if [[ ! -s "$LIST" ]]; then
  BIN="$(bazel ${COMMON_STARTUP} info bazel-bin)"
  cd "$EXECROOT"
  find "$BIN" -type f -printf '%P\n' | sed 's#^#'"${BIN##$EXECROOT/}"'/#' > "$LIST"
fi

cd "$EXECROOT"
tar -czf "$TGZ" -T "$LIST"

echo "$TGZ"
"#;

#[derive(Deserialize)]
struct ConfigFile {
    rbazel: Option<ConfigPartial>,
}

#[derive(Deserialize, Clone)]
struct Config {
    server_host: String,
    server_repo_dir: String,
    remote_cache_base: String,
    local_pull_base: String,
    remote_resource_path: Option<String>,
}

#[derive(Deserialize)]
struct ConfigPartial {
    server_host: Option<String>,
    server_repo_dir: Option<String>,
    remote_cache_base: Option<String>,
    local_pull_base: Option<String>,
    remote_resource_path: Option<String>,
}

fn main() {
    if let Err(e) = run() {
        eprintln!("[rbazel-rs] ERROR: {e}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    need("ssh")?;
    need("rsync")?;
    need("git")?;
    need("tar")?;

    let config = load_config()?;
    let remote_cache = config
        .remote_resource_path
        .clone()
        .unwrap_or_else(|| format!("{}/pennybot", config.remote_cache_base));

    let git_top = cmd_stdout("git", &["rev-parse", "--show-toplevel"])?;
    env::set_current_dir(Path::new(git_top.trim()))
        .map_err(|e| format!("cannot cd git root: {e}"))?;

    if !cmd_status_success("git", &["diff", "--quiet"])?
        || !cmd_status_success("git", &["diff", "--cached", "--quiet"])?
    {
        return Err(
            "dirty working tree. Commit/stash before running server-repo build.".to_string(),
        );
    }

    let local_head = cmd_stdout("git", &["rev-parse", "HEAD"])?;
    let mut local_branch = cmd_stdout("git", &["rev-parse", "--abbrev-ref", "HEAD"])?;
    let local_head = local_head.trim().to_string();
    local_branch = local_branch.trim().to_string();
    if local_branch == "HEAD" {
        let short = cmd_stdout("git", &["rev-parse", "--short", "HEAD"])?;
        local_branch = format!("detached-{}", short.trim());
    }
    let safe_branch = local_branch.replace(['/', ' '], "__");

    let mut args: Vec<String> = env::args().skip(1).collect();
    if args.first().is_some_and(|x| x == "bazel") {
        args.remove(0);
    }
    if args.is_empty() {
        eprintln!("{USAGE}");
        std::process::exit(2);
    }

    let subcmd = args.remove(0);
    let mut targets = Vec::new();
    let mut opts = Vec::new();
    for a in args {
        if a.starts_with("//") || a.starts_with(':') || (a.starts_with('@') && a.contains("//")) {
            targets.push(a);
        } else {
            opts.push(a);
        }
    }

    let has_targets = if targets.is_empty() { "0" } else { "1" };
    let stamp = cmd_stdout("date", &["+%Y%m%d_%H%M%S"])?;
    let local_out_dir = PathBuf::from(&config.local_pull_base)
        .join(safe_branch)
        .join(stamp.trim());
    fs::create_dir_all(&local_out_dir).map_err(|e| format!("cannot create output dir: {e}"))?;

    eprintln!("[rbazel-rs] server: {}", config.server_host);
    eprintln!("[rbazel-rs] repo:   {}", config.server_repo_dir);
    eprintln!("[rbazel-rs] head:   {local_head}");
    eprintln!("[rbazel-rs] pull:   {}", local_out_dir.display());

    let opts_q = shell_join_quoted(&opts);
    let targets_q = shell_join_quoted(&targets);

    let mut ssh = Command::new("ssh");
    ssh.arg(&config.server_host)
        .arg("bash")
        .arg("-s")
        .arg("--")
        .arg(&config.server_repo_dir)
        .arg(&remote_cache)
        .arg(&local_head)
        .arg(has_targets)
        .arg(&subcmd)
        .arg(&opts_q)
        .arg(&targets_q)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit());

    let mut child = ssh.spawn().map_err(|e| format!("ssh spawn failed: {e}"))?;
    {
        let stdin = child
            .stdin
            .as_mut()
            .ok_or_else(|| "failed to open ssh stdin".to_string())?;
        std::io::Write::write_all(stdin, REMOTE_SCRIPT.as_bytes())
            .map_err(|e| format!("failed writing remote script: {e}"))?;
    }

    let output = child
        .wait_with_output()
        .map_err(|e| format!("ssh wait failed: {e}"))?;
    if !output.status.success() {
        return Err(format!("remote build failed with status {}", output.status));
    }
    let remote_tgz = String::from_utf8(output.stdout)
        .map_err(|e| format!("remote output decode failed: {e}"))?;
    let remote_tgz = remote_tgz.trim().to_string();
    if remote_tgz.is_empty() {
        return Err("remote tarball path is empty".to_string());
    }

    cmd_status_or_err(
        "rsync",
        &[
            "-az",
            &format!("{}:{}", config.server_host, remote_tgz),
            &format!("{}/artifacts.tgz", local_out_dir.display()),
        ],
        "rsync pull failed",
    )?;

    cmd_status_or_err(
        "tar",
        &[
            "-xzf",
            &format!("{}/artifacts.tgz", local_out_dir.display()),
            "-C",
            &local_out_dir.display().to_string(),
        ],
        "extract failed",
    )?;

    let _ = Command::new("ssh")
        .arg(&config.server_host)
        .arg(format!("rm -f '{}'", remote_tgz.replace('\'', "'\\''")))
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();

    eprintln!(
        "[rbazel-rs] OK: artifacts extracted to: {}",
        local_out_dir.display()
    );
    eprintln!(
        "[rbazel-rs] Tip: find debs via: find '{}' -type f -name '*.deb'",
        local_out_dir.display()
    );
    Ok(())
}

fn load_config() -> Result<Config, String> {
    let mut config = Config {
        server_host: env::var("SERVER_HOST")
            .unwrap_or_else(|_| "ese-rs@192.168.43.177".to_string()),
        server_repo_dir: env::var("SERVER_REPO_DIR")
            .unwrap_or_else(|_| "/home/ese-rs/repositories/pennybot".to_string()),
        remote_cache_base: env::var("REMOTE_CACHE_BASE")
            .unwrap_or_else(|_| "/home/ese-rs/bazel_cache".to_string()),
        local_pull_base: env::var("LOCAL_PULL_BASE").unwrap_or_else(|_| {
            format!(
                "{}/_rbazel_artifacts",
                env::current_dir()
                    .unwrap_or_else(|_| PathBuf::from("."))
                    .display()
            )
        }),
        remote_resource_path: None,
    };

    if let Some(path) = find_config_path() {
        let path_display = path.display().to_string();
        let text =
            fs::read_to_string(&path).map_err(|e| format!("cannot read {}: {e}", path_display))?;
        let parsed: ConfigFile =
            toml::from_str(&text).map_err(|e| format!("invalid {}: {e}", path_display))?;
        if let Some(file_cfg) = parsed.rbazel {
            if let Some(v) = file_cfg.server_host {
                config.server_host = v;
            }
            if let Some(v) = file_cfg.server_repo_dir {
                config.server_repo_dir = v;
            }
            if let Some(v) = file_cfg.remote_cache_base {
                config.remote_cache_base = v;
            }
            if let Some(v) = file_cfg.local_pull_base {
                config.local_pull_base = v;
            }
            config.remote_resource_path = file_cfg.remote_resource_path;
        }
    }

    Ok(config)
}

fn find_config_path() -> Option<PathBuf> {
    let local = PathBuf::from("rbazel_config.toml");
    if local.exists() {
        return Some(local);
    }

    if let Ok(home) = env::var("HOME") {
        let global = PathBuf::from(home).join(".config/rbazel/config.toml");
        if global.exists() {
            return Some(global);
        }
    }

    None
}

fn need(cmd: &str) -> Result<(), String> {
    if cmd_status_success(
        "sh",
        &["-c", &format!("command -v {} >/dev/null 2>&1", cmd)],
    )? {
        Ok(())
    } else {
        Err(format!("missing command: {cmd}"))
    }
}

fn cmd_stdout(cmd: &str, args: &[&str]) -> Result<String, String> {
    let out = Command::new(cmd)
        .args(args)
        .output()
        .map_err(|e| format!("{cmd} failed to start: {e}"))?;
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr);
        return Err(format!("{cmd} failed: {}", err.trim()));
    }
    String::from_utf8(out.stdout).map_err(|e| format!("{cmd} output decode error: {e}"))
}

fn cmd_status_success(cmd: &str, args: &[&str]) -> Result<bool, String> {
    let status = Command::new(cmd)
        .args(args)
        .status()
        .map_err(|e| format!("{cmd} failed to start: {e}"))?;
    Ok(status.success())
}

fn cmd_status_or_err(cmd: &str, args: &[&str], context: &str) -> Result<(), String> {
    let status = Command::new(cmd)
        .args(args)
        .status()
        .map_err(|e| format!("{context}: {e}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("{context}: status {status}"))
    }
}

fn shell_join_quoted(args: &[String]) -> String {
    args.iter()
        .map(|a| {
            let escaped = a.replace('\'', "'\\''");
            format!("'{escaped}'")
        })
        .collect::<Vec<String>>()
        .join(" ")
}
