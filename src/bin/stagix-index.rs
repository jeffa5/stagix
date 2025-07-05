use std::path::PathBuf;

use clap::Parser;
use stagix::IndexOptions;

#[derive(Debug, Parser)]
struct Args {
    #[clap()]
    repos: Vec<PathBuf>,
    /// Directory to write the `index.html` file to, if unset the page is written to stdout.
    #[clap(long)]
    out_dir: Option<PathBuf>,
    /// Path to css stylesheet that will be copied next to the `index.html`, requires --out-dir
    #[clap(long, requires = "out_dir")]
    stylesheet: Option<PathBuf>,
    /// Path to png logo that will be copied next to the `index.html`, requires --out-dir
    #[clap(long, requires = "out_dir")]
    logo: Option<PathBuf>,
    /// Path to png favicon that will be copied next to the `index.html`, requires --out-dir
    #[clap(long, requires = "out_dir")]
    favicon: Option<PathBuf>,
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    stagix::build_index_page(
        args.repos,
        args.out_dir.map(|o| IndexOptions {
            out_dir: o,
            stylesheet: args.stylesheet,
            logo: args.logo,
            favicon: args.favicon,
        }),
    )?;

    Ok(())
}
