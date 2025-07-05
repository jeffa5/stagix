use std::path::PathBuf;

use clap::Parser;

#[derive(Debug, Parser)]
struct Args {
    #[clap()]
    repos: Vec<PathBuf>,
    /// Directory to write the `index.html` file to, if unset the page is written to stdout.
    #[clap(long)]
    out_dir: Option<PathBuf>,
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    stagix::build_index_page(args.repos, args.out_dir)?;

    Ok(())
}
