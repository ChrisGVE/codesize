use std::io;
use std::path::PathBuf;
use std::process;

use clap::{CommandFactory, Parser, Subcommand};
use clap_complete::{generate, Shell};
use codesize::{config, scanner};

#[derive(Parser)]
#[command(about = "Report code size violations by file and function.", version)]
struct Args {
    #[command(subcommand)]
    command: Option<Command>,

    /// Root directory to scan (defaults to current directory).
    #[arg(long, default_value = ".")]
    root: PathBuf,

    /// CSV output path.  Defaults to the value of `default_output_file` in
    /// config (built-in default: codesize.csv).  Ignored when --stdout is set.
    #[arg(long)]
    output: Option<PathBuf>,

    /// Write the CSV report to stdout instead of a file.
    #[arg(long, default_value_t = false)]
    stdout: bool,

    /// Percent tolerance added to limits (default 0).
    #[arg(long, default_value_t = 0.0)]
    tolerance: f64,

    /// Respect .gitignore / .ignore files found in the scanned tree.
    /// Overrides the `respect_gitignore` setting in config.toml.
    #[arg(long, default_value_t = false)]
    gitignore: bool,

    /// Exit with status 1 if any violations are found.  Useful for CI.
    #[arg(long, default_value_t = false)]
    fail: bool,
}

#[derive(Subcommand)]
enum Command {
    /// Print a shell completion script to stdout.
    ///
    /// Usage examples:
    ///   codesize init zsh  >> ~/.zshrc
    ///   codesize init bash >> ~/.bashrc
    ///   codesize init fish > ~/.config/fish/completions/codesize.fish
    Init {
        /// Shell to generate completions for.
        #[arg(value_enum)]
        shell: Shell,
    },
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    if let Some(Command::Init { shell }) = args.command {
        let mut cmd = Args::command();
        generate(shell, &mut cmd, "codesize", &mut io::stdout());
        return Ok(());
    }

    if args.tolerance < 0.0 {
        eprintln!("error: --tolerance must be >= 0");
        process::exit(2);
    }

    let root = args.root.canonicalize()?;
    let mut cfg = config::load_config();

    // CLI --gitignore overrides config; it can only enable, not disable.
    cfg.respect_gitignore |= args.gitignore;

    let mut findings = scanner::build_report(&root, args.tolerance, &cfg);

    let output: Option<PathBuf> = if args.stdout {
        None
    } else {
        Some(
            args.output
                .unwrap_or_else(|| PathBuf::from(&cfg.default_output_file)),
        )
    };

    let has_violations = !findings.is_empty();
    scanner::write_csv(&mut findings, output.as_deref())?;

    if args.fail && has_violations {
        process::exit(1);
    }
    Ok(())
}
