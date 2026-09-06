use crate::core::{
    add_favorite, export_nix, list_favorites, remove_all_favorites, remove_favorite, sync_favorites,
};
use crate::model::{FindbarConfig, InsertPosition, SidebarEntry};
use anyhow::{Context, Result, bail};
use clap::{CommandFactory, Parser, Subcommand, ValueEnum};
use clap_complete::{Shell, generate};
use colored::Colorize;
use std::fs;
use std::io::{self, Read};

#[derive(Parser)]
#[command(
    name = "findbar",
    version,
    about = "Declarative & standalone macOS Finder sidebar favorites manager",
    long_about = "findbar allows you to list, add, remove, and declaratively synchronize macOS Finder sidebar favorite items with support for Nix-darwin and Home Manager."
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// List current Finder sidebar favorites
    List {
        /// Output formatted as JSON
        #[arg(long)]
        json: bool,

        /// Output format
        #[arg(long, short = 'f', value_enum, default_value_t = OutputFormat::Table)]
        format: OutputFormat,
    },

    /// Add a path or URL to the Finder sidebar favorites
    Add {
        /// Path or file URL to the item
        path: String,

        /// Optional custom display name in the sidebar
        #[arg(short, long)]
        name: Option<String>,

        /// Insert before the specified item (by name or path)
        #[arg(long, conflicts_with_all = &["after", "beginning"])]
        before: Option<String>,

        /// Insert after the specified item (by name or path)
        #[arg(long, conflicts_with_all = &["before", "beginning"])]
        after: Option<String>,

        /// Insert at the beginning of the list
        #[arg(long, conflicts_with_all = &["before", "after"])]
        beginning: bool,

        /// Allow adding duplicate items if already present in favorites
        #[arg(short, long)]
        force: bool,
    },

    /// Generate shell completions for the specified shell
    Completions {
        /// Shell to generate completions for
        #[arg(value_enum)]
        shell: Shell,
    },

    /// Remove an item from the Finder sidebar favorites
    Remove {
        /// Name or path of the item to remove
        #[arg(required_unless_present = "all")]
        target: Option<String>,

        /// Remove all favorite items
        #[arg(long)]
        all: bool,

        /// Skip confirmation when removing all items
        #[arg(short, long)]
        force: bool,
    },

    /// Synchronize the sidebar with a declarative configuration (file or stdin)
    Sync {
        /// Path to config file (.json, .toml) or '-' for stdin
        #[arg(default_value = "-")]
        config: String,

        /// Keep unmanaged items that are not in the declarative config
        #[arg(long)]
        keep_unmanaged: bool,

        /// Preview changes without modifying the sidebar
        #[arg(long)]
        dry_run: bool,

        /// Output sync results as JSON
        #[arg(long)]
        json: bool,
    },

    /// Export current sidebar items into a declarative configuration
    Export {
        /// Target format for the exported configuration
        #[arg(long, short = 'f', value_enum, default_value_t = ExportFormat::Nix)]
        format: ExportFormat,

        /// Include keep_unmanaged in the export
        #[arg(long)]
        keep_unmanaged: bool,
    },
}

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum, Debug)]
pub enum OutputFormat {
    Table,
    Json,
    Nix,
}

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum, Debug)]
pub enum ExportFormat {
    Nix,
    Json,
    Toml,
}

pub fn run() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::List { json, format } => {
            let items = list_favorites()?;
            if json || format == OutputFormat::Json {
                println!("{}", serde_json::to_string_pretty(&items)?);
            } else if format == OutputFormat::Nix {
                println!("{}", export_nix(&items, false));
            } else {
                if items.is_empty() {
                    println!("{}", "No favorite items found in Finder sidebar.".yellow());
                    return Ok(());
                }
                println!(
                    "{:<4}  {:<24}  {}",
                    "ID".bold(),
                    "NAME".bold(),
                    "PATH / URL".bold()
                );
                println!("{:-<4}  {:-<24}  {:-<40}", "", "", "");
                for (idx, item) in items.iter().enumerate() {
                    let path_or_url = item
                        .path
                        .as_deref()
                        .or(item.url.as_deref())
                        .unwrap_or("(unresolved)");
                    println!(
                        "{:<4}  {:<24}  {}",
                        format!("{}.", idx + 1).dimmed(),
                        item.name.cyan().bold(),
                        path_or_url
                    );
                }
            }
        }

        Commands::Add {
            path,
            name,
            before,
            after,
            beginning,
            force,
        } => {
            let position = if beginning {
                InsertPosition::Beginning
            } else if let Some(b) = before {
                InsertPosition::Before(b)
            } else if let Some(a) = after {
                InsertPosition::After(a)
            } else {
                InsertPosition::End
            };

            let item = add_favorite(&path, name.as_deref(), position, force)?;
            println!(
                "{} Added '{}' ({}) to Finder sidebar favorites.",
                "✔".green().bold(),
                item.name.bold(),
                item.path.as_deref().unwrap_or(&path).dimmed()
            );
        }

        Commands::Completions { shell } => {
            let mut cmd = Cli::command();
            generate(shell, &mut cmd, "findbar", &mut io::stdout());
        }

        Commands::Remove { target, all, force } => {
            if all {
                if !force {
                    eprint!(
                        "Are you sure you want to remove ALL Finder sidebar favorites? [y/N]: "
                    );
                    let mut input = String::new();
                    io::stdin().read_line(&mut input)?;
                    if !input.trim().eq_ignore_ascii_case("y") {
                        println!("Aborted.");
                        return Ok(());
                    }
                }
                remove_all_favorites()?;
                println!(
                    "{} Removed all favorites from Finder sidebar.",
                    "✔".green().bold()
                );
            } else if let Some(t) = target {
                let removed = remove_favorite(&t)?;
                if removed {
                    println!(
                        "{} Removed '{}' from Finder sidebar favorites.",
                        "✔".green().bold(),
                        t.bold()
                    );
                } else {
                    bail!("Sidebar item '{}' not found.", t);
                }
            }
        }

        Commands::Sync {
            config,
            keep_unmanaged,
            dry_run,
            json,
        } => {
            let content = if config == "-" {
                let mut buffer = String::new();
                io::stdin()
                    .read_to_string(&mut buffer)
                    .context("Failed to read config from stdin")?;
                buffer
            } else {
                fs::read_to_string(&config)
                    .with_context(|| format!("Failed to read config file at '{}'", config))?
            };

            // Support parsing both FindbarConfig object and raw list of SidebarEntry or string paths
            let mut parsed_config: FindbarConfig =
                if let Ok(cfg) = serde_json::from_str::<FindbarConfig>(&content) {
                    cfg
                } else if let Ok(entries) = serde_json::from_str::<Vec<SidebarEntry>>(&content) {
                    FindbarConfig {
                        items: entries,
                        keep_unmanaged,
                    }
                } else if let Ok(paths) = serde_json::from_str::<Vec<String>>(&content) {
                    FindbarConfig {
                        items: paths
                            .into_iter()
                            .map(|p| SidebarEntry {
                                name: None,
                                path: p,
                            })
                            .collect(),
                        keep_unmanaged,
                    }
                } else if let Ok(cfg) = toml::from_str::<FindbarConfig>(&content) {
                    cfg
                } else {
                    bail!("Failed to parse configuration as JSON or TOML. Ensure format is valid.");
                };

            if keep_unmanaged {
                parsed_config.keep_unmanaged = true;
            }

            let report = sync_favorites(&parsed_config, dry_run)?;

            if json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                if dry_run {
                    println!(
                        "{}",
                        "[DRY RUN] Planned sidebar modifications:".yellow().bold()
                    );
                } else {
                    println!(
                        "{}",
                        "Finder sidebar synchronization complete:".green().bold()
                    );
                }

                for action in &report.actions {
                    let symbol = match action.action.as_str() {
                        "added" | "add" => "+".green().bold(),
                        "removed" | "remove" => "-".red().bold(),
                        "reordered" => "~".yellow().bold(),
                        "unchanged" | "preserve" => "=".dimmed(),
                        _ => "*".normal(),
                    };
                    println!(
                        "  {} {:<10} {:<24} {}",
                        symbol,
                        action.action,
                        action.name.bold(),
                        action.path.as_deref().unwrap_or("").dimmed()
                    );
                }

                println!(
                    "\nSummary: {} added, {} removed, {} unchanged",
                    report.added.to_string().green(),
                    report.removed.to_string().red(),
                    report.unchanged.to_string().dimmed()
                );
            }
        }

        Commands::Export {
            format,
            keep_unmanaged,
        } => {
            let items = list_favorites()?;
            match format {
                ExportFormat::Nix => {
                    println!("{}", export_nix(&items, keep_unmanaged));
                }
                ExportFormat::Json => {
                    let config = FindbarConfig {
                        items: items
                            .into_iter()
                            .map(|it| SidebarEntry {
                                name: Some(it.name),
                                path: it.path.unwrap_or_else(|| "~".to_string()),
                            })
                            .collect(),
                        keep_unmanaged,
                    };
                    println!("{}", serde_json::to_string_pretty(&config)?);
                }
                ExportFormat::Toml => {
                    let config = FindbarConfig {
                        items: items
                            .into_iter()
                            .map(|it| SidebarEntry {
                                name: Some(it.name),
                                path: it.path.unwrap_or_else(|| "~".to_string()),
                            })
                            .collect(),
                        keep_unmanaged,
                    };
                    println!("{}", toml::to_string_pretty(&config)?);
                }
            }
        }
    }

    Ok(())
}
