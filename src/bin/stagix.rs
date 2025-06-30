use clap::Parser;
use stagix::build_index_page;
use std::path::PathBuf;

#[derive(Debug, Parser)]
struct Args {
    #[clap()]
    repos: Vec<PathBuf>,
    #[clap(long, default_value = ".")]
    out_dir: PathBuf,
    /// Number of commits to limit log history to, uses all commits if not set.
    #[clap(short, long)]
    log_length: Option<usize>,
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    build_index_page(args.repos, &args.out_dir, true, args.log_length)?;

    Ok(())
}
