mod build;
mod check;
mod emit;
mod finding;
mod font;
mod layout;
mod spec;
mod theme;
mod timeline;

use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand, ValueEnum};

/// Animated SVG infographics from a YAML spec, verified before delivery.
///
/// Prints a JSON report on stdout. Exit status: 0 when the static gate
/// passes, 1 when it fails, 2 when the command itself could not run.
#[derive(Parser)]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Build light and dark SVGs plus a storyboard PNG from a spec, and check them.
    Build {
        spec: PathBuf,
        /// Output directory [default: the spec's directory]
        #[arg(short, long)]
        out_dir: Option<PathBuf>,
        /// Also write each storyboard frame as SVG to <slug>.frames/
        #[arg(long)]
        frames: bool,
    },
    /// Check any SVG (hand-written or generated) and render it to PNG.
    Check {
        svg: PathBuf,
        #[arg(short, long, value_enum, default_value_t = ProfileArg::Readme)]
        profile: ProfileArg,
        /// PNG path [default: next to the SVG]
        #[arg(long)]
        png: Option<PathBuf>,
    },
}

#[derive(Clone, Copy, ValueEnum)]
enum ProfileArg {
    Readme,
    Interactive,
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let report = match &cli.command {
        Command::Build {
            spec,
            out_dir,
            frames,
        } => build::build(spec, out_dir.as_deref(), *frames),
        Command::Check { svg, profile, png } => {
            let p = match profile {
                ProfileArg::Readme => spec::Profile::Readme,
                ProfileArg::Interactive => spec::Profile::Interactive,
            };
            build::check(svg, p, png.as_deref())
        }
    };
    match report {
        Ok(r) => {
            println!(
                "{}",
                serde_json::to_string_pretty(&r).expect("report serialises")
            );
            if r.ok {
                ExitCode::SUCCESS
            } else {
                ExitCode::from(1)
            }
        }
        Err(e) => {
            eprintln!("error: {e:#}");
            ExitCode::from(2)
        }
    }
}
