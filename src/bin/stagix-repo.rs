use clap::Parser;
use std::path::PathBuf;

#[derive(Debug, Parser)]
struct Args {
    #[clap()]
    repo: PathBuf,
    #[clap(long, default_value = ".")]
    out_dir: PathBuf,
    /// Number of commits to limit log history to, uses all commits if not set.
    #[clap(short, long)]
    log_length: Option<usize>,
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    stagix::build_repo_pages(&args.repo, &args.out_dir, args.log_length)?;

    Ok(())
}
