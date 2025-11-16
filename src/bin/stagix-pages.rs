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

    /// Whether or not to create an index page, the same as stagix-index.
    #[clap(long)]
    index: bool,

    // index options
    /// Path to css stylesheet that will be copied next to the `index.html`, requires --out-dir
    #[clap(long)]
    stylesheet: Option<PathBuf>,
    /// Path to png logo that will be copied next to the `index.html`, requires --out-dir
    #[clap(long)]
    logo: Option<PathBuf>,
    /// Path to png favicon that will be copied next to the `index.html`, requires --out-dir
    #[clap(long)]
    favicon: Option<PathBuf>,
    /// URL to use as the base for repos links.
    #[clap(long)]
    repos_url: Option<String>,
    /// URL to use as the base for pages links.
    #[clap(long)]
    pages_url: Option<String>,
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    tracing_subscriber::fmt::init();

    stagix::build_pages_dirs(
        args.repos,
        PagesOptions {
            out_dir: args.out_dir.clone(),
            working_dir: args.working_dir,
            index: args.index.then_some(stagix::IndexOptions {
                out_dir: Some(args.out_dir),
                stylesheet: args.stylesheet,
                logo: args.logo,
                favicon: args.favicon,
                repos_url: args.repos_url,
                pages_url: args.pages_url,
            }),
        },
    )?;

    Ok(())
}
