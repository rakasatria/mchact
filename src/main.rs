use argon2::password_hash::{rand_core::OsRng, PasswordHashString, SaltString};
use argon2::{Argon2, PasswordHasher};
use clap::{Args, CommandFactory, Parser, Subcommand};
use mchact::config::Config;
use mchact::error::MchactError;
use mchact::{
    builtin_skills, db, doctor, gateway, hooks, logging, mcp, memory, runtime, setup, skills,
};
use std::path::{Path, PathBuf};
use std::process::Command as ProcessCommand;
use tracing::info;

const VERSION: &str = env!("CARGO_PKG_VERSION");
const LONG_ABOUT: &str = concat!(
    "\x1b[1mmchact v",
    env!("CARGO_PKG_VERSION"),
    "\x1b[22m\n",
    "\x1b[1mWebsite:\x1b[22m https://mchact.ai\n",
    "\x1b[1mGitHub:\x1b[22m https://github.com/mchact/mchact\n",
    "\x1b[1mDiscord:\x1b[22m https://discord.gg/pvmezwkAk5\n",
    "\n",
    "\x1b[1mQuick Start:\x1b[22m\n",
    "  1) mchact setup\n",
    "  2) mchact doctor\n",
    "  3) mchact start",
);

#[derive(Debug, Parser)]
#[command(
    name = "mchact",
    version = VERSION,
    about = LONG_ABOUT
)]
struct Cli {
    /// Explicit config file path (absolute or relative)
    #[arg(long, global = true, value_name = "PATH")]
    config: Option<PathBuf>,
    #[command(subcommand)]
    command: Option<MainCommand>,
}

#[derive(Debug, Subcommand)]
enum MainCommand {
    /// Start runtime (enabled channels)
    Start,
    /// Serve Agent Client Protocol (ACP) over stdio
    Acp,
    /// Full-screen setup wizard (or `setup --enable-sandbox`)
    Setup(SetupCommand),
    /// Preflight diagnostics
    Doctor {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    /// Manage service (install/start/stop/status/logs)
    Gateway {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    /// Manage ClawHub skills (search/install/list/inspect)
    Skill {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    /// Manage runtime hooks (list/info/enable/disable)
    Hooks {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    /// Manage Weixin native login state
    Weixin(WeixinCommand),
    /// Manage Web UI configurations
    Web(WebCommand),
    /// Re-embed active memories (requires `sqlite-vec` feature)
    Reembed,
    /// Upgrade mchact to latest release
    Upgrade,
    /// Show version
    Version,
    /// Generate training trajectories from a prompt dataset
    Batch {
        /// Path to the JSONL prompt dataset
        dataset: PathBuf,
        /// Number of parallel workers
        #[arg(long, default_value = "4")]
        workers: usize,
        /// Prompts per batch
        #[arg(long, default_value = "10")]
        batch_size: usize,
        /// Tool distribution to use
        #[arg(long, default_value = "default")]
        distribution: String,
        /// Maximum agent iterations per prompt
        #[arg(long, default_value = "10")]
        max_iterations: usize,
        /// Override model for this run
        #[arg(long)]
        model: Option<String>,
        /// Name for this run (auto-generated if not provided)
        #[arg(long)]
        run_name: Option<String>,
        /// Resume a previous run, skipping already-completed prompts
        #[arg(long)]
        resume: bool,
        /// Maximum number of samples to load from dataset
        #[arg(long)]
        max_samples: Option<usize>,
        /// Output directory for batches and results
        #[arg(long)]
        output: Option<PathBuf>,
    },
    /// Internal: worker process for batch trajectory generation
    #[command(hide = true)]
    Worker {
        /// Batch file to process
        #[arg(long)]
        batch_file: PathBuf,
        /// Distribution name
        #[arg(long, default_value = "default")]
        distribution: String,
        /// Max tool iterations
        #[arg(long, default_value = "10")]
        max_iterations: usize,
        /// Override model
        #[arg(long)]
        model: Option<String>,
        /// Distributions YAML file
        #[arg(long)]
        distributions_file: Option<PathBuf>,
    },
    /// Export trajectories to different formats
    Export {
        /// Input trajectories JSONL file
        input: PathBuf,
        /// Output format: openai or sharegpt
        #[arg(long, default_value = "openai")]
        format: String,
        /// Tool-call parser for ShareGPT format
        #[arg(long, default_value = "hermes")]
        parser: String,
        /// Output file
        #[arg(long)]
        output: Option<PathBuf>,
        /// Only export completed trajectories
        #[arg(long)]
        filter_completed: bool,
        /// Minimum tool calls to include
        #[arg(long)]
        filter_min_tools: Option<u64>,
    },
    /// RL training management
    Rl {
        #[command(subcommand)]
        action: RlAction,
    },
    /// Run full training pipeline: batch → export → compress
    Train {
        /// Path to the JSONL prompt dataset
        dataset: PathBuf,
        /// Number of parallel workers
        #[arg(long, default_value = "4")]
        workers: usize,
        /// Prompts per batch
        #[arg(long, default_value = "10")]
        batch_size: usize,
        /// Tool distribution to use
        #[arg(long, default_value = "default")]
        distribution: String,
        /// Maximum agent iterations per prompt
        #[arg(long, default_value = "10")]
        max_iterations: usize,
        /// Override model for this run
        #[arg(long)]
        model: Option<String>,
        /// Output format: openai or sharegpt
        #[arg(long, default_value = "openai")]
        format: String,
        /// Tool-call parser for ShareGPT format
        #[arg(long, default_value = "hermes")]
        parser: String,
        /// Run compression step after export
        #[arg(long)]
        compress: bool,
        /// Target token count for compression
        #[arg(long, default_value = "15250")]
        target_tokens: usize,
        /// Name for this run (auto-generated if not provided)
        #[arg(long)]
        run_name: Option<String>,
        /// Resume a previous run, skipping already-completed prompts
        #[arg(long)]
        resume: bool,
        /// Output directory for batches and results
        #[arg(long)]
        output: Option<PathBuf>,
    },
}

#[derive(Debug, Args)]
struct SetupCommand {
    /// Enable sandbox mode in config
    #[arg(long)]
    enable_sandbox: bool,
    /// Assume yes for follow-up prompts
    #[arg(short = 'y', long)]
    yes: bool,
    /// Suppress follow-up tips
    #[arg(long)]
    quiet: bool,
}

#[derive(Debug, Args)]
struct WebCommand {
    #[command(subcommand)]
    action: Option<WebAction>,
}

#[derive(Debug, Args)]
struct WeixinCommand {
    #[command(subcommand)]
    action: Option<WeixinAction>,
}

#[derive(Debug, Subcommand)]
enum WeixinAction {
    /// Login via QR code and persist native credentials
    Login {
        #[arg(long, value_name = "ACCOUNT")]
        account: Option<String>,
        #[arg(long, value_name = "URL")]
        base_url: Option<String>,
    },
    /// Show stored native credential and sync status
    Status {
        #[arg(long, value_name = "ACCOUNT")]
        account: Option<String>,
    },
    /// Remove stored native credentials and sync cursor
    Logout {
        #[arg(long, value_name = "ACCOUNT")]
        account: Option<String>,
    },
}

#[derive(Debug, Subcommand)]
enum WebAction {
    /// Set the exact new password (min 8 chars)
    Password { value: String },
    /// Generate and set a random password
    PasswordGenerate,
    /// Clear password hash and revoke sessions (test/reset)
    PasswordClear,
}

#[derive(Debug, Subcommand)]
enum RlAction {
    /// List available training environments
    List,
    /// Select an environment and show its config
    Select { name: String },
    /// Show current config (locked + configurable fields)
    Config,
    /// Edit a configurable field
    Edit { field: String, value: String },
    /// Start a training run
    Start,
    /// Check training status and WandB metrics
    Status { run_id: Option<String> },
    /// Stop a training run
    Stop { run_id: Option<String> },
    /// Fetch final results
    Results { run_id: Option<String> },
    /// List all training runs
    Runs,
    /// Test inference without full training
    Test {
        #[arg(long, default_value = "3")]
        steps: usize,
        #[arg(long, default_value = "16")]
        group_size: usize,
    },
}

fn print_version() {
    println!("mchact {VERSION}");
}

fn handle_upgrade_cli() -> anyhow::Result<()> {
    let repo = "mchact/mchact";
    println!("Current version: {VERSION}");
    println!("Upgrading from latest release of {repo}...");

    let status = match std::env::consts::OS {
        "windows" => {
            let script_url = format!("https://raw.githubusercontent.com/{repo}/main/install.ps1");
            let current_pid = std::process::id();
            ProcessCommand::new("powershell")
                .args([
                    "-NoProfile",
                    "-ExecutionPolicy",
                    "Bypass",
                    "-Command",
                    &format!(
                        "$script = (iwr '{url}' -UseBasicParsing).Content; \
                         $scriptPath = Join-Path $env:TEMP ('mchact-install-' + [guid]::NewGuid().ToString() + '.ps1'); \
                         Set-Content -Path $scriptPath -Value $script; \
                         Start-Process powershell -WindowStyle Hidden -ArgumentList @('-NoProfile','-ExecutionPolicy','Bypass','-File', $scriptPath, '-SkipRun', '-WaitForPid', '{pid}')",
                        url = script_url,
                        pid = current_pid
                    ),
                ])
                .status()
                .map_err(|e| anyhow::anyhow!("failed to run powershell installer: {e}"))?
        }
        _ => {
            let script_url = format!("https://raw.githubusercontent.com/{repo}/main/install.sh");
            let cmd = format!(
                "(curl -fsSL '{url}' || wget -qO- '{url}') | bash -s -- --skip-run",
                url = script_url
            );
            ProcessCommand::new("sh")
                .args(["-c", &cmd])
                .status()
                .map_err(|e| anyhow::anyhow!("failed to run shell installer: {e}"))?
        }
    };

    if !status.success() {
        anyhow::bail!(
            "upgrade failed (exit code {:?}). You can retry with install script:\n  macOS/Linux: curl -fsSL https://mchact.ai/install.sh | bash\n  Windows: iwr https://mchact.ai/install.ps1 -UseBasicParsing | iex",
            status.code()
        );
    }

    if std::env::consts::OS == "windows" {
        println!("Upgrade started in the background. Wait a few seconds, then re-run `mchact version` to verify.");
    } else {
        println!("Upgrade completed. Re-run `mchact version` to verify.");
    }
    Ok(())
}

fn print_web_help() {
    println!(
        r#"Manage Web UI Configurations

Usage:
  mchact web [password <value> | password-generate | password-clear]

Options:
  password <value>      Set the exact new password (min 8 chars)
  password-generate     Generate a random password
  password-clear        Clear password hash and revoke sessions (test/reset)

Notes:
  - Existing Web login sessions are revoked automatically.
  - Restart is not required."#
    );
}

fn print_weixin_help() {
    println!(
        r#"Manage Weixin Native State

Usage:
  mchact weixin [login|status|logout] [options]

Commands:
  login           Login via QR code and persist native credentials
  status          Show stored native credential and sync status
  logout          Remove stored native credentials and sync cursor

Options:
  --account <id>  Select configured account id (defaults to the channel default account)
  --base-url <u>  Override base URL for login only

Notes:
  - Native mode supports QR login, polling, text, and file/image/video attachment delivery.
  - Weixin is native-only in mchact; no Node bridge is required."#
    );
}

async fn handle_weixin_cli(action: Option<WeixinAction>) -> anyhow::Result<()> {
    let Some(action) = action else {
        print_weixin_help();
        return Ok(());
    };

    let config = Config::load()?;
    match action {
        WeixinAction::Login { account, base_url } => {
            let message = mchact::channels::weixin::login_via_cli(
                &config,
                account.as_deref(),
                base_url.as_deref(),
            )
            .await
            .map_err(anyhow::Error::msg)?;
            println!("{message}");
        }
        WeixinAction::Status { account } => {
            let message = mchact::channels::weixin::status_via_cli(&config, account.as_deref())
                .map_err(anyhow::Error::msg)?;
            println!("{message}");
        }
        WeixinAction::Logout { account } => {
            let message = mchact::channels::weixin::logout_via_cli(&config, account.as_deref())
                .map_err(anyhow::Error::msg)?;
            println!("{message}");
        }
    }
    Ok(())
}

fn make_password_hash(password: &str) -> anyhow::Result<String> {
    let salt = SaltString::generate(&mut OsRng);
    let hash: PasswordHashString = Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map_err(|e| anyhow::anyhow!("password hashing failed: {e}"))?
        .serialize();
    Ok(hash.to_string())
}

fn generate_password() -> String {
    let rand = uuid::Uuid::new_v4().simple().to_string();
    format!("mc-{}-{}!", &rand[..6], &rand[6..12])
}

fn handle_web_cli(action: Option<WebAction>) -> anyhow::Result<()> {
    if action.is_none() {
        print_web_help();
        return Ok(());
    }

    if matches!(action, Some(WebAction::PasswordClear)) {
        let config = Config::load()?;
        let runtime_data_dir = config.runtime_data_dir();
        let database = db::Database::new(&runtime_data_dir)?;
        database.clear_auth_password_hash()?;
        let revoked = database.revoke_all_auth_sessions()?;
        println!("Web password cleared.");
        println!("Revoked web sessions: {revoked}");
        println!(
            "State is now uninitialized. On next `mchact start`, default password bootstrap policy will apply."
        );
        return Ok(());
    }

    let (password, generated) = match action {
        Some(WebAction::PasswordGenerate) => (generate_password(), true),
        Some(WebAction::Password { value }) => (value, false),
        Some(WebAction::PasswordClear) => unreachable!("handled above"),
        None => unreachable!("handled above"),
    };
    let normalized = password.trim().to_string();
    if normalized.len() < 8 {
        anyhow::bail!("password must be at least 8 chars");
    }

    let config = Config::load()?;
    let runtime_data_dir = config.runtime_data_dir();
    let database = db::Database::new(&runtime_data_dir)?;
    let hash = make_password_hash(&normalized)?;
    database.upsert_auth_password_hash(&hash)?;
    let revoked = database.revoke_all_auth_sessions()?;

    println!("Web password reset successfully.");
    println!("Revoked web sessions: {revoked}");
    if generated {
        println!("Generated password: {normalized}");
    }
    Ok(())
}

fn move_path(src: &Path, dst: &Path) -> std::io::Result<()> {
    if std::fs::rename(src, dst).is_ok() {
        return Ok(());
    }

    if src.is_dir() {
        std::fs::create_dir_all(dst)?;
        for entry in std::fs::read_dir(src)? {
            let entry = entry?;
            let child_src = entry.path();
            let child_dst = dst.join(entry.file_name());
            move_path(&child_src, &child_dst)?;
        }
        std::fs::remove_dir_all(src)?;
    } else {
        if let Some(parent) = dst.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::copy(src, dst)?;
        std::fs::remove_file(src)?;
    }

    Ok(())
}

fn is_legacy_runtime_entry(name: &str) -> bool {
    matches!(name, "groups" | "logs" | "uploads" | "hooks_state.json")
}

fn cleanup_stale_runtime_working_dir(data_root: &Path, runtime_dir: &Path) {
    let runtime_working_dir = runtime_dir.join("working_dir");
    if !runtime_working_dir.is_dir() {
        return;
    }
    let root_working_dir = data_root.join("working_dir");
    if !root_working_dir.exists() {
        return;
    }
    let is_empty = std::fs::read_dir(&runtime_working_dir)
        .map(|mut it| it.next().is_none())
        .unwrap_or(false);
    if is_empty {
        if let Err(e) = std::fs::remove_dir(&runtime_working_dir) {
            tracing::warn!(
                "Failed to remove stale runtime working_dir '{}': {}",
                runtime_working_dir.display(),
                e
            );
        } else {
            tracing::info!(
                "Removed stale empty runtime working_dir '{}'",
                runtime_working_dir.display()
            );
        }
    }
}

fn migrate_legacy_runtime_layout(data_root: &Path, runtime_dir: &Path) {
    cleanup_stale_runtime_working_dir(data_root, runtime_dir);

    let entries = match std::fs::read_dir(data_root) {
        Ok(entries) => entries,
        Err(_) => return,
    };

    let mut runtime_dir_ready = false;
    for entry in entries.flatten() {
        let name = entry.file_name();
        let Some(name_str) = name.to_str() else {
            continue;
        };
        if !is_legacy_runtime_entry(name_str) {
            continue;
        }
        let src = entry.path();
        let dst = runtime_dir.join(name_str);
        if dst.exists() {
            continue;
        }
        if !runtime_dir_ready {
            if std::fs::create_dir_all(runtime_dir).is_err() {
                return;
            }
            runtime_dir_ready = true;
        }
        if let Err(e) = move_path(&src, &dst) {
            tracing::warn!(
                "Failed to migrate legacy data '{}' -> '{}': {}",
                src.display(),
                dst.display(),
                e
            );
        } else {
            tracing::info!(
                "Migrated legacy runtime data '{}' -> '{}'",
                src.display(),
                dst.display()
            );
        }
    }
}

fn migrate_legacy_skills_dir(legacy_dir: &Path, preferred_dir: &Path) {
    if legacy_dir == preferred_dir || !legacy_dir.exists() {
        return;
    }
    if std::fs::create_dir_all(preferred_dir).is_err() {
        return;
    }
    let entries = match std::fs::read_dir(legacy_dir) {
        Ok(entries) => entries,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let src = entry.path();
        let dst = preferred_dir.join(entry.file_name());
        if dst.exists() {
            continue;
        }
        if let Err(e) = move_path(&src, &dst) {
            tracing::warn!(
                "Failed to migrate legacy skills '{}' -> '{}': {}",
                src.display(),
                dst.display(),
                e
            );
        } else {
            tracing::info!(
                "Migrated legacy skill '{}' -> '{}'",
                src.display(),
                dst.display()
            );
        }
    }
}

fn collect_mcp_config_paths(data_root: &Path) -> Vec<PathBuf> {
    let mut paths = vec![data_root.join("mcp.json")];
    let mcp_dir = data_root.join("mcp.d");
    let mut fragments = match std::fs::read_dir(&mcp_dir) {
        Ok(entries) => entries
            .flatten()
            .map(|entry| entry.path())
            .filter(|path| path.extension().and_then(|s| s.to_str()) == Some("json"))
            .collect::<Vec<_>>(),
        Err(_) => Vec::new(),
    };
    fragments.sort();
    paths.extend(fragments);
    paths
}

fn apply_config_override(path: Option<&PathBuf>) -> anyhow::Result<()> {
    let Some(path) = path else {
        return Ok(());
    };
    if path.as_os_str().is_empty() {
        anyhow::bail!("--config path cannot be empty");
    }
    let resolved = if path.is_absolute() {
        path.clone()
    } else {
        std::env::current_dir()
            .map_err(|e| anyhow::anyhow!("failed to resolve current directory: {e}"))?
            .join(path)
    };
    if !resolved.exists() {
        anyhow::bail!(
            "--config points to non-existent file: {}",
            resolved.display()
        );
    }
    std::env::set_var("MCHACT_CONFIG", &resolved);
    Ok(())
}

async fn reembed_memories() -> anyhow::Result<()> {
    let config = Config::load()?;

    #[cfg(not(feature = "sqlite-vec"))]
    {
        let _ = config;
        anyhow::bail!(
            "sqlite-vec feature not enabled. Rebuild with: cargo build --release --features sqlite-vec"
        );
    }

    #[cfg(feature = "sqlite-vec")]
    {
        use mchact::embedding;
        let runtime_data_dir = config.runtime_data_dir();
        let db = db::Database::new(&runtime_data_dir)?;

        let provider = embedding::create_provider(&config);
        let provider = match provider {
            Some(p) => p,
            None => {
                eprintln!("No embedding provider configured. Check embedding_provider in config.");
                std::process::exit(1);
            }
        };

        let dim = provider.dimension();
        db.prepare_vector_index(dim)?;
        println!("Embedding provider: {} ({}D)", provider.model(), dim);

        let memories = db.get_all_active_memories()?;
        println!("Re-embedding {} active memories...", memories.len());

        let mut success = 0usize;
        let mut failed = 0usize;
        for (i, (id, content)) in memories.iter().enumerate() {
            match provider.embed(content).await {
                Ok(embedding) => {
                    if let Err(e) = db.upsert_memory_vec(*id, &embedding) {
                        eprintln!("  [{}] DB error: {}", id, e);
                        failed += 1;
                    } else {
                        let _ = db.update_memory_embedding_model(*id, provider.model());
                        success += 1;
                    }
                }
                Err(e) => {
                    eprintln!("  [{}] Embed error: {}", id, e);
                    failed += 1;
                }
            }
            if (i + 1) % 20 == 0 {
                println!(
                    "  Progress: {}/{} (ok={}, fail={})",
                    i + 1,
                    memories.len(),
                    success,
                    failed
                );
            }
        }

        println!("Done! {} embedded, {} failed", success, failed);
        Ok(())
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    apply_config_override(cli.config.as_ref())?;

    let launch_mode = match cli.command {
        Some(MainCommand::Start) => Some("start"),
        Some(MainCommand::Acp) => Some("acp"),
        Some(MainCommand::Gateway { args }) => {
            gateway::handle_gateway_cli(&args)?;
            return Ok(());
        }
        Some(MainCommand::Setup(setup_args)) => {
            if setup_args.enable_sandbox {
                let path = setup::enable_sandbox_in_config()?;
                println!("Sandbox enabled in {path}");
                if !setup_args.yes && !setup_args.quiet {
                    println!(
                        "Tip: run `mchact doctor sandbox` to verify container runtime and image readiness."
                    );
                }
            } else {
                let saved = setup::run_setup_wizard()?;
                if saved {
                    println!("Setup saved to mchact.config.yaml");
                } else {
                    println!("Setup canceled");
                }
            }
            return Ok(());
        }
        Some(MainCommand::Doctor { args }) => {
            doctor::run_cli(&args)?;
            return Ok(());
        }
        Some(MainCommand::Web(web)) => {
            handle_web_cli(web.action)?;
            return Ok(());
        }
        Some(MainCommand::Skill { args }) => {
            let config = Config::load()?;
            mchact::clawhub::cli::handle_skill_cli(&args, &config).await?;
            return Ok(());
        }
        Some(MainCommand::Hooks { args }) => {
            hooks::handle_hooks_cli(&args).await?;
            return Ok(());
        }
        Some(MainCommand::Weixin(weixin)) => {
            handle_weixin_cli(weixin.action).await?;
            return Ok(());
        }
        Some(MainCommand::Reembed) => {
            return reembed_memories().await;
        }
        Some(MainCommand::Upgrade) => {
            handle_upgrade_cli()?;
            return Ok(());
        }
        Some(MainCommand::Version) => {
            print_version();
            return Ok(());
        }
        Some(MainCommand::Batch {
            dataset,
            workers,
            batch_size,
            distribution,
            max_iterations,
            model,
            run_name,
            resume,
            max_samples,
            output,
        }) => {
            use chrono::Utc;
            use mchact::batch;

            let run_name = run_name.unwrap_or_else(|| {
                format!("run_{}", Utc::now().format("%Y%m%d_%H%M%S"))
            });

            let output_dir = output.unwrap_or_else(|| PathBuf::from("output").join(&run_name));
            std::fs::create_dir_all(&output_dir)
                .map_err(|e| anyhow::anyhow!("failed to create output dir: {e}"))?;

            let mut prompts = match batch::load_dataset(&dataset, max_samples) {
                Ok(p) => p,
                Err(e) => {
                    eprintln!("Dataset load failed: {e}");
                    std::process::exit(1);
                }
            };

            if resume {
                let completed = batch::find_completed_prompts(&output_dir);
                if !completed.is_empty() {
                    println!("Resuming: {} prompts already completed", completed.len());
                    prompts = batch::filter_completed(prompts, &completed);
                }
            }

            let batches = batch::split_batches(prompts, batch_size);
            println!(
                "Run '{}': {} prompts across {} batch(es) (batch_size={}, workers={})",
                run_name,
                batches.iter().map(|b| b.len()).sum::<usize>(),
                batches.len(),
                batch_size,
                workers,
            );

            let config_path = cli.config.clone();

            let output_paths = match batch::spawn_workers(
                &batches,
                &output_dir,
                workers,
                &distribution,
                max_iterations,
                model.as_deref(),
                config_path.as_deref(),
                None,
            ) {
                Ok(paths) => paths,
                Err(e) => {
                    eprintln!("Worker spawning failed: {e}");
                    std::process::exit(1);
                }
            };

            println!("All workers complete: {} batch file(s) written", output_paths.len());

            let all_tool_names: Vec<String> = Vec::new();
            match batch::combine_batches(&output_dir, &all_tool_names) {
                Ok(result) => {
                    println!(
                        "Combined: total={} valid={} filtered={}",
                        result.total, result.valid, result.filtered
                    );
                }
                Err(e) => {
                    eprintln!("Combine batches failed: {e}");
                    std::process::exit(1);
                }
            }

            return Ok(());
        }
        Some(MainCommand::Worker {
            batch_file,
            distribution,
            max_iterations,
            model,
            distributions_file,
        }) => {
            use std::io::Write as IoWrite;
            use mchact::{batch_worker::BatchPrompt, distributions};
            use chrono::Utc;

            // Read input batch file (JSONL)
            let content = std::fs::read_to_string(&batch_file)
                .map_err(|e| anyhow::anyhow!("failed to read batch file '{}': {e}", batch_file.display()))?;

            let prompts: Vec<BatchPrompt> = content
                .lines()
                .filter(|l| !l.trim().is_empty())
                .filter_map(|l| serde_json::from_str(l).ok())
                .collect();

            // Load distributions
            let dist_path = distributions_file
                .unwrap_or_else(|| PathBuf::from("training/distributions.yaml"));
            let dist_map = distributions::load_distributions(&dist_path)
                .map_err(|e| anyhow::anyhow!("failed to load distributions: {e}"))?;
            let dist = dist_map.get(&distribution).ok_or_else(|| {
                anyhow::anyhow!("distribution '{}' not found in distributions file", distribution)
            })?;

            // Determine output path
            let out_path = {
                let mut p = batch_file.as_os_str().to_owned();
                p.push(".out.jsonl");
                PathBuf::from(p)
            };

            let out_file = std::fs::File::create(&out_path)
                .map_err(|e| anyhow::anyhow!("failed to create output '{}': {e}", out_path.display()))?;
            let mut writer = std::io::BufWriter::new(out_file);

            let model_str = model.as_deref().unwrap_or("unknown");
            let timestamp = Utc::now().to_rfc3339();

            for (idx, prompt) in prompts.iter().enumerate() {
                // Use per-prompt toolsets override or sample from distribution
                let toolsets_used = if let Some(ref ts) = prompt.toolsets {
                    ts.clone()
                } else {
                    distributions::sample_tools(dist)
                };

                let entry = serde_json::json!({
                    "prompt_index": prompt.prompt_index,
                    "messages": [
                        {"role": "system", "content": "You are a helpful assistant."},
                        {"role": "user", "content": &prompt.prompt}
                    ],
                    "metadata": {
                        "batch_num": idx,
                        "timestamp": &timestamp,
                        "model": model_str
                    },
                    "completed": false,
                    "partial": false,
                    "api_calls": 0,
                    "toolsets_used": toolsets_used,
                    "tool_stats": {},
                    "tool_error_counts": {}
                });

                let line = serde_json::to_string(&entry)
                    .map_err(|e| anyhow::anyhow!("failed to serialize entry: {e}"))?;
                writeln!(writer, "{line}")
                    .map_err(|e| anyhow::anyhow!("failed to write output: {e}"))?;
            }

            writer.flush()
                .map_err(|e| anyhow::anyhow!("failed to flush output: {e}"))?;

            println!("Worker completed: {} prompts", prompts.len());
            let _ = max_iterations; // used in future integration step
            return Ok(());
        }
        Some(MainCommand::Export {
            input,
            format,
            parser,
            output,
            filter_completed,
            filter_min_tools,
        }) => {
            let output_path = output.unwrap_or_else(|| {
                mchact::export::default_output_path(&input, &format)
            });
            match mchact::export::export_file(
                &input,
                &output_path,
                &format,
                &parser,
                filter_completed,
                filter_min_tools,
            ) {
                Ok(stats) => {
                    println!("Export complete: {}", output_path.display());
                    println!(
                        "  Total: {}  Exported: {}  Filtered: {}",
                        stats.total, stats.exported, stats.filtered
                    );
                }
                Err(e) => {
                    eprintln!("Export failed: {e}");
                    std::process::exit(1);
                }
            }
            return Ok(());
        }
        Some(MainCommand::Rl { action }) => {
            let config = Config::load()?;
            let env_dir = std::path::Path::new(&config.training_environments_dir).to_path_buf();

            match action {
                RlAction::List => {
                    let envs = mchact::rl::discover_environments(&env_dir)
                        .map_err(|e| MchactError::Config(e))?;
                    if envs.is_empty() {
                        println!("No RL environments found in: {}", env_dir.display());
                        println!("Add Python environment files with YAML frontmatter to that directory.");
                    } else {
                        println!("Available RL environments ({}):", envs.len());
                        for env in &envs {
                            println!("  {} — {} ({})", env.name, env.description, env.class_name);
                        }
                    }
                }
                RlAction::Select { name } => {
                    let envs = mchact::rl::discover_environments(&env_dir)
                        .map_err(|e| MchactError::Config(e))?;
                    match envs.iter().find(|e| e.name == name) {
                        None => {
                            eprintln!("Environment '{}' not found.", name);
                            eprintln!("Run `mchact rl list` to see available environments.");
                            std::process::exit(1);
                        }
                        Some(env) => {
                            println!("Environment: {}", env.name);
                            println!("  Class:       {}", env.class_name);
                            println!("  Description: {}", env.description);
                            println!("  File:        {}", env.file_path.display());
                            println!();
                            let locked = mchact::rl::locked_config();
                            println!("Locked config:");
                            println!("{}", serde_json::to_string_pretty(&locked).unwrap_or_default());
                        }
                    }
                }
                RlAction::Config => {
                    let locked = mchact::rl::locked_config();
                    println!("{}", serde_json::to_string_pretty(&locked).unwrap_or_default());
                }
                RlAction::Edit { field, value } => {
                    if mchact::rl::is_locked_field(&field) {
                        eprintln!("Field '{}' is locked and cannot be modified.", field);
                        eprintln!("Locked fields ensure reproducibility and are managed by the infrastructure.");
                        std::process::exit(1);
                    }
                    println!("Configurable field '{}' set to '{}'.", field, value);
                    println!("(Note: persistent config editing is managed via agent tools at runtime.)");
                }
                RlAction::Start => {
                    let tinker_key = std::env::var("TINKER_API_KEY").unwrap_or_default();
                    let wandb_key = std::env::var("WANDB_API_KEY").unwrap_or_default();

                    if tinker_key.is_empty() {
                        eprintln!("TINKER_API_KEY environment variable is not set.");
                        std::process::exit(1);
                    }
                    if wandb_key.is_empty() {
                        eprintln!("WANDB_API_KEY environment variable is not set.");
                        std::process::exit(1);
                    }

                    let envs = mchact::rl::discover_environments(&env_dir)
                        .map_err(|e| MchactError::Config(e))?;
                    if envs.is_empty() {
                        eprintln!("No RL environments found in: {}", env_dir.display());
                        std::process::exit(1);
                    }

                    let env = &envs[0];
                    let run_id = uuid::Uuid::new_v4().simple().to_string()[..8].to_string();
                    let manager = mchact::rl::RlRunManager::new();
                    let locked = mchact::rl::locked_config();
                    let training_dir = std::path::Path::new("training");

                    match manager.start_run(&run_id, env, locked, None, training_dir) {
                        Ok(()) => {
                            println!("Training run '{}' started for environment '{}'.", run_id, env.name);
                            println!("Use `mchact rl status {}` to check progress.", run_id);
                        }
                        Err(e) => {
                            eprintln!("Failed to start training run: {e}");
                            std::process::exit(1);
                        }
                    }
                }
                RlAction::Status { run_id } => {
                    match run_id {
                        Some(id) => println!("Status for run '{}': use agent tools for persistent run tracking.", id),
                        None => println!("No active run ID provided. Use agent tools for persistent run tracking across sessions."),
                    }
                }
                RlAction::Stop { run_id } => {
                    match run_id {
                        Some(id) => println!("Stop requested for run '{}'. Use agent tools for persistent run management.", id),
                        None => println!("No run ID provided. Use `mchact rl runs` to list runs, then `mchact rl stop <run_id>`."),
                    }
                }
                RlAction::Results { run_id } => {
                    match run_id {
                        Some(id) => println!("Results for run '{}': use agent tools for persistent result retrieval.", id),
                        None => println!("No run ID provided. Use `mchact rl runs` to list completed runs."),
                    }
                }
                RlAction::Runs => {
                    println!("Persistent run tracking is managed via agent tools across sessions.");
                    println!("Use the rl_list_runs agent tool to list all tracked runs.");
                }
                RlAction::Test { steps, group_size } => {
                    println!("RL inference test: steps={}, group_size={}", steps, group_size);
                    println!("Test inference is managed via agent tools for full environment integration.");
                }
            }

            return Ok(());
        }
        Some(MainCommand::Train {
            dataset,
            workers,
            batch_size,
            distribution,
            max_iterations,
            model,
            format,
            parser,
            compress,
            target_tokens,
            run_name,
            resume,
            output,
        }) => {
            use chrono::Utc;
            use mchact::train_pipeline::{PipelineConfig, run_pipeline};

            let run_name = run_name.unwrap_or_else(|| {
                format!("run_{}", Utc::now().format("%Y%m%d_%H%M%S"))
            });
            let output_dir = output.unwrap_or_else(|| PathBuf::from("output").join(&run_name));
            let config_path = cli.config.clone();

            let pipeline_config = PipelineConfig {
                dataset,
                workers,
                batch_size,
                distribution,
                max_iterations,
                model,
                format,
                parser,
                compress,
                target_tokens,
                run_name,
                output_dir,
                resume,
                config_path,
            };

            match run_pipeline(&pipeline_config) {
                Ok(result) => {
                    println!("Pipeline complete.");
                    println!("  Trajectories: {}", result.trajectories.display());
                    if let Some(exported) = &result.exported {
                        println!("  Exported:     {}", exported.display());
                    }
                    if let Some(compressed) = &result.compressed {
                        println!("  Compressed:   {}", compressed.display());
                    }
                    println!("  Statistics:   {}", result.statistics.display());
                }
                Err(e) => {
                    eprintln!("Pipeline failed: {e}");
                    std::process::exit(1);
                }
            }
            return Ok(());
        }
        None => {
            let mut cmd = Cli::command();
            cmd.print_help()?;
            println!();
            return Ok(());
        }
    };

    let config = match Config::load() {
        Ok(c) => c,
        Err(MchactError::Config(e)) => {
            eprintln!("Config missing/invalid: {e}");
            eprintln!("Launching setup wizard...");
            let saved = setup::run_setup_wizard()?;
            if !saved {
                return Err(anyhow::anyhow!(
                    "setup canceled and config is still incomplete"
                ));
            }
            Config::load()?
        }
        Err(e) => return Err(e.into()),
    };
    info!("Starting mchact bot...");

    let data_root_dir = config.data_root_dir();
    let runtime_data_dir = config.runtime_data_dir();
    let skills_data_dir = config.skills_data_dir();
    let legacy_skills_dir = data_root_dir.join("skills");
    migrate_legacy_runtime_layout(&data_root_dir, Path::new(&runtime_data_dir));
    migrate_legacy_skills_dir(&legacy_skills_dir, Path::new(&skills_data_dir));

    if std::env::var("MCHACT_GATEWAY").is_ok() {
        logging::init_logging(&runtime_data_dir, config.observability.as_ref())?;
    } else {
        logging::init_console_logging(config.observability.as_ref());
    }

    builtin_skills::ensure_builtin_skills(Path::new(&skills_data_dir))?;

    let db = db::Database::new(&runtime_data_dir)?;
    info!("Database initialized");

    let memory_manager = memory::MemoryManager::new(&runtime_data_dir);
    info!("Memory manager initialized");

    let skill_manager =
        skills::SkillManager::from_skills_and_runtime(&skills_data_dir, &runtime_data_dir);
    let discovered = skill_manager.discover_skills();
    info!(
        "Skill manager initialized ({} skills discovered)",
        discovered.len()
    );

    // Initialize MCP servers (optional, configured via <data_root>/mcp.json and <data_root>/mcp.d/*.json)
    let mcp_config_paths = collect_mcp_config_paths(&data_root_dir);
    let mcp_manager =
        mcp::McpManager::from_config_paths(&mcp_config_paths, config.mcp_request_timeout_secs())
            .await;
    let mcp_tool_count: usize = mcp_manager.all_tools().len();
    if mcp_tool_count > 0 {
        info!("MCP initialized: {} tools available", mcp_tool_count);
    }

    let mut runtime_config = config.clone();
    runtime_config.data_dir = runtime_data_dir;
    // Keep tool-side skill resolution aligned with the already-resolved skills directory.
    // Otherwise, changing data_dir to runtime/ would make tools default to runtime/skills.
    runtime_config.skills_dir = Some(skills_data_dir);

    match launch_mode {
        Some("start") => {
            runtime::run(
                runtime_config,
                db,
                memory_manager,
                skill_manager,
                mcp_manager,
            )
            .await?;
        }
        Some("acp") => {
            mchact::acp::serve(
                runtime_config,
                db,
                memory_manager,
                skill_manager,
                mcp_manager,
            )
            .await?;
        }
        _ => unreachable!("launch mode must be resolved before runtime init"),
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{apply_config_override, migrate_legacy_runtime_layout, Cli, MainCommand};
    use clap::Parser;
    use mchact::config::Config;
    use std::path::{Path, PathBuf};

    fn unique_temp_dir() -> std::path::PathBuf {
        let dir =
            std::env::temp_dir().join(format!("mchact-main-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).expect("create temp test dir");
        dir
    }

    #[test]
    fn migrate_legacy_runtime_layout_keeps_working_dir_at_data_root() {
        let root = unique_temp_dir();
        let runtime_dir = root.join("runtime");
        let working_dir = root.join("working_dir");
        std::fs::create_dir_all(&working_dir).expect("create working_dir");

        migrate_legacy_runtime_layout(&root, Path::new(&runtime_dir));

        assert!(working_dir.exists());
        assert!(!runtime_dir.join("working_dir").exists());
        assert!(!runtime_dir.exists());

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn cli_parses_global_config_option_for_start() {
        let cli = Cli::parse_from([
            "mchact",
            "--config",
            "api_test_mchact.config.yaml",
            "start",
        ]);
        let config = cli
            .config
            .as_ref()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_default();
        assert_eq!(config, "api_test_mchact.config.yaml");
        assert!(matches!(cli.command, Some(MainCommand::Start)));
    }

    #[test]
    fn apply_config_override_accepts_relative_path() {
        let base = unique_temp_dir();
        let cfg = base.join("api_test_mchact.config.yaml");
        std::fs::write(&cfg, "web_enabled: true\n").expect("write config");

        let old_cwd = std::env::current_dir().expect("current_dir");
        let old_cfg = std::env::var("MCHACT_CONFIG").ok();
        std::env::set_current_dir(&base).expect("set_current_dir");
        let rel = PathBuf::from("api_test_mchact.config.yaml");
        apply_config_override(Some(&rel)).expect("apply config override");
        let resolved = std::env::var("MCHACT_CONFIG").expect("MCHACT_CONFIG");
        assert!(resolved.ends_with("api_test_mchact.config.yaml"));

        if let Some(v) = old_cfg {
            std::env::set_var("MCHACT_CONFIG", v);
        } else {
            std::env::remove_var("MCHACT_CONFIG");
        }
        std::env::set_current_dir(old_cwd).expect("restore cwd");
        let _ = std::fs::remove_dir_all(base);
    }

    #[test]
    fn migrate_legacy_runtime_layout_does_not_create_runtime_dir_when_no_entries() {
        let root = unique_temp_dir();
        let runtime_dir = root.join("runtime");

        migrate_legacy_runtime_layout(&root, Path::new(&runtime_dir));

        assert!(!runtime_dir.exists());
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn migrate_legacy_runtime_layout_keeps_souls_at_data_root() {
        let root = unique_temp_dir();
        let runtime_dir = root.join("runtime");
        let souls_dir = root.join("souls");
        std::fs::create_dir_all(&souls_dir).expect("create souls dir");
        std::fs::write(souls_dir.join("bot.md"), "soul").expect("write soul file");

        migrate_legacy_runtime_layout(&root, Path::new(&runtime_dir));

        assert!(souls_dir.exists());
        assert!(souls_dir.join("bot.md").exists());
        assert!(!runtime_dir.join("souls").exists());

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn migrate_legacy_runtime_layout_only_migrates_whitelisted_entries() {
        let root = unique_temp_dir();
        let runtime_dir = root.join("runtime");
        let groups_dir = root.join("groups");
        let souls_dir = root.join("souls");
        std::fs::create_dir_all(&groups_dir).expect("create groups dir");
        std::fs::create_dir_all(&souls_dir).expect("create souls dir");
        std::fs::write(groups_dir.join("AGENTS.md"), "g").expect("write groups file");
        std::fs::write(souls_dir.join("bot.md"), "soul").expect("write soul file");
        std::fs::write(root.join("mchact.db"), "db").expect("write db file");

        migrate_legacy_runtime_layout(&root, Path::new(&runtime_dir));

        assert!(root.join("mchact.db").exists());
        assert!(!groups_dir.exists());
        assert!(!runtime_dir.join("mchact.db").exists());
        assert!(runtime_dir.join("groups").join("AGENTS.md").exists());
        assert!(souls_dir.exists());
        assert!(souls_dir.join("bot.md").exists());
        assert!(!runtime_dir.join("souls").exists());

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn migrate_legacy_runtime_layout_removes_empty_runtime_working_dir() {
        let root = unique_temp_dir();
        let runtime_dir = root.join("runtime");
        let runtime_working_dir = runtime_dir.join("working_dir");
        let root_working_dir = root.join("working_dir");
        std::fs::create_dir_all(&runtime_working_dir).expect("create runtime/working_dir");
        std::fs::create_dir_all(&root_working_dir).expect("create root working_dir");

        migrate_legacy_runtime_layout(&root, Path::new(&runtime_dir));

        assert!(!runtime_working_dir.exists());
        assert!(root_working_dir.exists());

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn runtime_config_keeps_resolved_skills_dir_after_data_dir_swap() {
        let root = unique_temp_dir();
        let mut config: Config = serde_yaml::from_str("{}").expect("default config from yaml");
        config.data_dir = root.to_string_lossy().to_string();

        let runtime_data_dir = config.runtime_data_dir();
        let resolved_skills_dir = config.skills_data_dir();

        let mut runtime_config = config.clone();
        runtime_config.data_dir = runtime_data_dir;
        runtime_config.skills_dir = Some(resolved_skills_dir.clone());

        assert_eq!(runtime_config.skills_data_dir(), resolved_skills_dir);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn cli_parses_upgrade_command() {
        let cli = Cli::parse_from(["mchact", "upgrade"]);
        assert!(matches!(cli.command, Some(MainCommand::Upgrade)));
    }

    #[test]
    fn cli_parses_weixin_status_command() {
        let cli = Cli::parse_from(["mchact", "weixin", "status", "--account", "ops"]);
        match cli.command {
            Some(MainCommand::Weixin(_)) => {}
            other => panic!("unexpected command: {other:?}"),
        }
    }
}
