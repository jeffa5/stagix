use std::path::PathBuf;

use clap::Parser;
use stagix::PagesOptions;

#[derive(Debug, Parser)]
struct Args {
    #[clap()]
    repos: Vec<PathBuf>,
    /// Directory to write the `index.html` file to, if unset the page is written to stdout.
    #[clap(long)]
    out_dir: PathBuf,
    /// Directory to use for temporarily copying the files to for a repo. This must be on the same
    /// filesytem as the out_dir.
    #[clap(long)]
    working_dir: PathBuf,
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    tracing_subscriber::fmt::init();

    stagix::build_pages_dirs(
        args.repos,
        PagesOptions {
            out_dir: args.out_dir,
            working_dir: args.working_dir,
        },
    )?;

    Ok(())
}
