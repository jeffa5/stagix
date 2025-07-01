use std::path::PathBuf;

use clap::Parser;

#[derive(Debug, Parser)]
struct Args {
    #[clap()]
    repos: Vec<PathBuf>,
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    stagix::build_index_page(args.repos)?;

    Ok(())
}
