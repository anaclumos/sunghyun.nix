mod actions;
mod assets;
mod ax;
mod bootstrap;
mod config;
mod default_browser;
mod dispatch;
mod error;
mod fn_state;
mod headless;
mod hotkeys;
mod kanata_ctl;
mod menubar;
mod post_switch;
mod spotlight;
mod status;
mod verify;
mod virt;

use clap::{Parser, Subcommand, ValueEnum};
use config::load_or_default;
use std::path::PathBuf;
use std::process::ExitCode;

#[derive(Parser, Debug)]
#[command(
    name = "sunghyun",
    about = "Keyboard actions + residual OS-prompt surfaces (Nix owns the rest)",
    version
)]
struct Cli {
    /// Force headless-safe mode (also: SUNGHYUN_HEADLESS=1)
    #[arg(
        long,
        global = true,
        env = "SUNGHYUN_HEADLESS",
        action = clap::ArgAction::SetTrue,
        value_parser = clap::builder::BoolishValueParser::new()
    )]
    headless: bool,

    /// Path to sunghyun.toml
    #[arg(long, global = true, env = "SUNGHYUN_CONFIG")]
    config: Option<PathBuf>,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Open an app by config key, bundle id, or name (`default-browser` / `browser` → OS default)
    Open { target: String },
    /// Activate the OS default web browser (Hyper+J)
    OpenDefaultBrowser,
    /// Flip the system appearance between light and dark (Hyper+`)
    ToggleDarkMode,
    /// Report, or ask macOS to change, the default web browser
    DefaultBrowser {
        #[command(subcommand)]
        cmd: DefaultBrowserCmd,
    },
    /// Switch input source (ABC / 2SetKorean / raw TIS id)
    InputSource { name: String },
    /// Tile the focused window
    Tile { action: String },
    /// Optional app launcher (Spotlight owns ⌘Space by default)
    Launcher {
        #[arg(long)]
        query: Option<String>,
    },
    /// Optional sunghyun clipboard picker (disabled by default; prefer Spotlight ⌘4)
    Clipboard {
        #[command(subcommand)]
        cmd: Option<ClipboardCmd>,
    },
    /// Spotlight ⌘Space, Clipboard Search, terminal→Ghostty alias
    Spotlight {
        #[command(subcommand)]
        cmd: SpotlightCmd,
    },
    /// Verify inventory capabilities (pass/skip/fail)
    Verify {
        #[arg(long)]
        json: bool,
    },
    /// Residual steps after `darwin-rebuild switch` (open TCC panes + poll, Spotlight, menu bar)
    PostSwitch {
        #[arg(long)]
        dry_run: bool,
        #[arg(long)]
        json: bool,
    },
    /// Top-row fn behaviour: push the declared `fnState` into IOHIDSystem now
    ///
    /// `system.defaults` only writes the preference, which IOHIDSystem reads at
    /// login, so an already-running session keeps the old top row until this runs.
    FnState {
        #[command(subcommand)]
        cmd: FnStateCmd,
    },
    /// Chords reserved for apps: free them from the macOS system shortcuts
    ///
    /// The preference domain is only read at login, so this also asks the
    /// running window server to drop the binding.
    Hotkeys {
        #[command(subcommand)]
        cmd: HotkeysCmd,
    },
    /// Kanata LaunchDaemon control (enable only via `--safe` passthrough proof + rollback)
    Kanata {
        #[command(subcommand)]
        cmd: KanataCmd,
    },
    /// Report virtualization state; exit 0 inside a VM, 1 on bare metal
    ///
    /// Single source of truth for the VM gate: the mas convergence
    /// LaunchDaemon calls this instead of re-implementing the sysctl probe.
    Virt,
}

#[derive(Subcommand, Debug)]
enum DefaultBrowserCmd {
    /// Print the bundle id currently registered for http
    Status,
    /// Ask macOS to make an app the default; macOS raises its own confirmation panel
    Set {
        /// Bundle id. Defaults to Dia.
        #[arg(long, default_value = default_browser::DIA_BUNDLE_ID)]
        bundle_id: String,
        /// Seconds to wait for the confirmation panel to be answered
        #[arg(long, default_value_t = 120)]
        timeout: u64,
    },
}

#[derive(Subcommand, Debug)]
enum FnStateCmd {
    /// Print the mode IOHIDSystem is enforcing right now
    Status,
    /// Make IOHIDSystem enforce the given mode
    Apply {
        /// true = "Use F1, F2, etc. keys as standard function keys"
        #[arg(
            long,
            action = clap::ArgAction::Set,
            value_parser = clap::builder::BoolishValueParser::new()
        )]
        standard_function_keys: bool,
    },
}

#[derive(Subcommand, Debug)]
enum HotkeysCmd {
    /// Print the system shortcuts currently sitting on a reserved chord
    Status,
    /// Disable them, now and at the next login
    Apply,
}

#[derive(Subcommand, Debug)]
enum KanataCmd {
    /// Show daemon / plist / pid state
    Status,
    /// Bootout + kill + move plist to `.disabled` (emergency)
    Disable,
    /// Staged enable: VirtualHID up → passthrough proof → full config proof → LaunchDaemon
    Enable {
        /// Required. Refuses plain enable; always runs proof + automatic rollback on failure.
        #[arg(long)]
        safe: bool,
    },
}

#[derive(Subcommand, Debug)]
enum ClipboardCmd {
    Show,
    Capture,
    Paste { index: usize },
}

#[derive(Subcommand, Debug)]
enum SpotlightCmd {
    /// Enable ⌘Space, Clipboard Search, and ~/Applications/terminal.app → Ghostty
    Restore,
    /// Install Spotlight name alias: typing "terminal" opens Ghostty
    InstallTerminalAlias,
    /// Open Clipboard Search (⌘Space then ⌘4). Bound to ⌘⇧V in kanata.kbd.
    Clipboard,
    /// Print whether Spotlight ⌘Space is enabled
    Status {
        #[arg(long, value_enum, default_value_t = OutputFormat::Plain)]
        format: OutputFormat,
    },
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum OutputFormat {
    Plain,
    Json,
}

fn main() -> ExitCode {
    // Fast paths for TCC probe/register children (no clap, no Settings, no sudo).
    if std::env::var_os("SUNGHYUN_AX_PROBE").is_some() {
        return if ax::probe_exit_trusted() {
            ExitCode::SUCCESS
        } else {
            ExitCode::from(1)
        };
    }
    if std::env::var_os("SUNGHYUN_AX_REGISTER").is_some() {
        return if ax::register_exit_trusted() {
            ExitCode::SUCCESS
        } else {
            ExitCode::from(1)
        };
    }

    let cli = Cli::parse();
    if cli.headless {
        headless::force(true);
    }

    // AXUIElement calls attribute to the responsible process. When that
    // process is trusted (karabiner_console_user_server, Cursor, a granted
    // terminal), run in-process and ride the inherited grant. Otherwise
    // re-exec disclaimed so this binary becomes its own TCC principal and
    // its direct grant applies regardless of the spawning chain.
    if matches!(cli.command, Commands::Tile { .. }) && !ax::is_process_trusted() {
        if let Some(code) = ax::reexec_disclaimed_exit_code() {
            return ExitCode::from(u8::try_from(code).unwrap_or(1));
        }
    }

    match cli.command {
        Commands::Open { target } => action_exit(cli.config.as_deref(), |cfg| {
            actions::open::open_target(cfg, &target)
        }),
        Commands::OpenDefaultBrowser => action_exit(cli.config.as_deref(), |_cfg| {
            actions::open::open_default_browser()
        }),
        Commands::ToggleDarkMode => {
            action_exit(cli.config.as_deref(), |_cfg| actions::appearance::toggle())
        }
        Commands::DefaultBrowser { cmd } => match cmd {
            DefaultBrowserCmd::Status => {
                match default_browser::current_handler() {
                    Some(id) => println!("default_browser={id}"),
                    None => println!("default_browser=unknown"),
                }
                ExitCode::SUCCESS
            }
            DefaultBrowserCmd::Set { bundle_id, timeout } => {
                match default_browser::converge(&bundle_id, std::time::Duration::from_secs(timeout))
                {
                    Ok(msg) => {
                        println!("{msg}");
                        ExitCode::SUCCESS
                    }
                    Err(error::ActionError::Skipped(m)) => {
                        eprintln!("skipped: {m}");
                        ExitCode::SUCCESS
                    }
                    Err(e) => {
                        eprintln!("{e}");
                        ExitCode::FAILURE
                    }
                }
            }
        },
        Commands::InputSource { name } => action_exit(cli.config.as_deref(), |cfg| {
            actions::input_source::switch(cfg, &name)
        }),
        Commands::Tile { action } => action_exit(cli.config.as_deref(), |cfg| {
            actions::tile::tile(cfg, &action)
        }),
        Commands::Launcher { query } => action_exit(cli.config.as_deref(), |cfg| {
            actions::launcher::launch(cfg, query.as_deref())
        }),
        Commands::Clipboard { cmd } => {
            let cmd = cmd.unwrap_or(ClipboardCmd::Show);
            action_exit(cli.config.as_deref(), |cfg| match cmd {
                ClipboardCmd::Show => actions::clipboard::show(cfg),
                ClipboardCmd::Capture => actions::clipboard::capture(cfg),
                ClipboardCmd::Paste { index } => actions::clipboard::paste_index(cfg, index),
            })
        }
        Commands::Spotlight { cmd } => match cmd {
            SpotlightCmd::Restore => {
                let mut failed = None;
                match spotlight::restore_command_space() {
                    Ok(()) => {}
                    Err(error::ActionError::Skipped(m)) => eprintln!("skipped: {m}"),
                    Err(e) => failed = Some(e.to_string()),
                }
                if failed.is_none() {
                    match spotlight::enable_pasteboard_history() {
                        Ok(()) => {}
                        Err(error::ActionError::Skipped(m)) => eprintln!("skipped: {m}"),
                        Err(e) => failed = Some(e.to_string()),
                    }
                }
                if failed.is_none() {
                    if let Some(home) = directories::UserDirs::new() {
                        match spotlight::install_terminal_ghostty_alias(home.home_dir()) {
                            Ok(()) => {}
                            Err(error::ActionError::Skipped(m)) => eprintln!("skipped: {m}"),
                            Err(e) => failed = Some(e.to_string()),
                        }
                    }
                }
                match failed {
                    None => ExitCode::SUCCESS,
                    Some(e) => {
                        eprintln!("{e}");
                        ExitCode::FAILURE
                    }
                }
            }
            SpotlightCmd::InstallTerminalAlias => {
                let Some(home) = directories::UserDirs::new() else {
                    eprintln!("no home directory");
                    return ExitCode::FAILURE;
                };
                match spotlight::install_terminal_ghostty_alias(home.home_dir()) {
                    Ok(()) => ExitCode::SUCCESS,
                    Err(error::ActionError::Skipped(m)) => {
                        eprintln!("skipped: {m}");
                        ExitCode::SUCCESS
                    }
                    Err(e) => {
                        eprintln!("{e}");
                        ExitCode::FAILURE
                    }
                }
            }
            SpotlightCmd::Clipboard => match spotlight::open_clipboard_search() {
                Ok(()) => ExitCode::SUCCESS,
                Err(error::ActionError::Skipped(m)) => {
                    eprintln!("skipped: {m}");
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("{e}");
                    ExitCode::FAILURE
                }
            },
            SpotlightCmd::Status { format } => match spotlight::is_command_space_enabled() {
                Ok(enabled) => {
                    match format {
                        OutputFormat::Plain => {
                            println!("spotlight_command_space_enabled={enabled}")
                        }
                        OutputFormat::Json => {
                            println!("{{\"spotlight_command_space_enabled\":{enabled}}}")
                        }
                    }
                    ExitCode::SUCCESS
                }
                Err(error::ActionError::Skipped(m)) => {
                    eprintln!("skipped: {m}");
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("{e}");
                    ExitCode::FAILURE
                }
            },
        },
        Commands::Verify { json } => {
            let report = verify::run(&verify::VerifyOpts {
                config_path: cli.config.clone(),
                json,
                headless: cli.headless || headless::is_headless(),
            });
            if json {
                println!("{}", report.to_json());
            } else {
                println!("{}", report.to_plain());
            }
            if report.exit_code() == 0 {
                ExitCode::SUCCESS
            } else {
                ExitCode::FAILURE
            }
        }
        Commands::PostSwitch { dry_run, json } => {
            let report = post_switch::run(&post_switch::PostSwitchOpts {
                dry_run,
                headless: cli.headless || headless::is_headless(),
                manifest: post_switch::default_manifest(),
            });
            if json {
                println!("{}", report.to_json());
            } else {
                println!("{}", report.to_plain());
            }
            if report.exit_code() == 0 {
                ExitCode::SUCCESS
            } else {
                ExitCode::FAILURE
            }
        }
        Commands::FnState { cmd } => match cmd {
            FnStateCmd::Status => match fn_state::current_mode() {
                Ok(mode) => {
                    println!("standard_function_keys={}", mode != 0);
                    ExitCode::SUCCESS
                }
                Err(error::ActionError::Skipped(m)) => {
                    eprintln!("skipped: {m}");
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("{e}");
                    ExitCode::FAILURE
                }
            },
            FnStateCmd::Apply {
                standard_function_keys,
            } => match fn_state::apply(standard_function_keys) {
                Ok(()) => {
                    println!("standard_function_keys={standard_function_keys}");
                    ExitCode::SUCCESS
                }
                Err(error::ActionError::Skipped(m)) => {
                    eprintln!("skipped: {m}");
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("{e}");
                    ExitCode::FAILURE
                }
            },
        },
        Commands::Hotkeys { cmd } => match cmd {
            HotkeysCmd::Status => match hotkeys::claimants() {
                Ok(found) => {
                    if found.is_empty() {
                        println!("no system shortcut claims a reserved chord");
                    }
                    for c in found {
                        println!(
                            "{} enabled={}",
                            c.describe(),
                            c.enabled
                        );
                    }
                    ExitCode::SUCCESS
                }
                Err(error::ActionError::Skipped(m)) => {
                    eprintln!("skipped: {m}");
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("{e}");
                    ExitCode::FAILURE
                }
            },
            HotkeysCmd::Apply => match hotkeys::apply() {
                Ok(()) => ExitCode::SUCCESS,
                Err(error::ActionError::Skipped(m)) => {
                    eprintln!("skipped: {m}");
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("{e}");
                    ExitCode::FAILURE
                }
            },
        },
        Commands::Kanata { cmd } => match cmd {
            KanataCmd::Status => match kanata_ctl::status() {
                Ok(()) => ExitCode::SUCCESS,
                Err(e) => {
                    eprintln!("{e}");
                    ExitCode::FAILURE
                }
            },
            KanataCmd::Disable => match kanata_ctl::disable() {
                Ok(()) => ExitCode::SUCCESS,
                Err(e) => {
                    eprintln!("{e}");
                    ExitCode::FAILURE
                }
            },
            KanataCmd::Enable { safe } => {
                if !safe {
                    eprintln!("refusing: use `sunghyun kanata enable --safe` (passthrough proof + rollback)");
                    return ExitCode::FAILURE;
                }
                match kanata_ctl::enable_safe() {
                    Ok(()) => ExitCode::SUCCESS,
                    Err(error::ActionError::Skipped(m)) => {
                        eprintln!("skipped: {m}");
                        ExitCode::SUCCESS
                    }
                    Err(e) => {
                        eprintln!("{e}");
                        ExitCode::FAILURE
                    }
                }
            }
        },
        Commands::Virt => {
            println!("{}", virt::describe());
            if virt::detect().is_guest() {
                ExitCode::SUCCESS
            } else {
                ExitCode::FAILURE
            }
        }
    }
}

fn action_exit(
    config: Option<&std::path::Path>,
    f: impl FnOnce(&config::Config) -> error::ActionResult,
) -> ExitCode {
    let (cfg, _) = match load_or_default(config) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::FAILURE;
        }
    };
    match f(&cfg) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error::ActionError::Skipped(m)) => {
            eprintln!("skipped: {m}");
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("{e}");
            ExitCode::FAILURE
        }
    }
}
