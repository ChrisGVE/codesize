use std::path::PathBuf;
use std::process;

use clap::Parser;
use largecode::{config, scanner};

#[derive(Parser)]
#[command(about = "Report code size violations by file and function.")]
struct Args {
    /// Root directory to scan (defaults to current directory).
    #[arg(long, default_value = ".")]
    root: PathBuf,

    /// CSV output path (defaults to largecode.csv in current directory).
    #[arg(long, default_value = "largecode.csv")]
    output: PathBuf,

    /// Percent tolerance added to limits (default 0).
    #[arg(long, default_value_t = 0.0)]
    tolerance: f64,
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    if args.tolerance < 0.0 {
        eprintln!("error: --tolerance must be >= 0");
        process::exit(2);
    }

    let root = args.root.canonicalize()?;
    let cfg = config::load_config();
    let mut findings = scanner::build_report(&root, args.tolerance, &cfg);
    scanner::write_csv(&mut findings, &args.output)?;
    Ok(())
}
