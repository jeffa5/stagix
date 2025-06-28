use build_html::{Container, Html, HtmlContainer, HtmlPage};
use clap::Parser;
use gix::Repository;
use gix::bstr::ByteSlice as _;
use gix_date::time::format::ISO8601;
use std::fs::create_dir_all;
use std::path::Path;
use std::path::PathBuf;

#[derive(Debug, Parser)]
struct Args {
    #[clap()]
    repo: PathBuf,
    #[clap(default_value = "out")]
    out_dir: PathBuf,
}

fn get_refs(repo: &Repository) -> anyhow::Result<Container> {
    let refs = repo.references()?;
    let mut container = build_html::Container::new(build_html::ContainerType::Div);
    container.add_header(2, "Tags");
    let mut table = build_html::Table::new().with_header_row(["Name", "Time", "Author"]);
    for tag in refs.tags()? {
        let mut tag = tag.unwrap();
        let commit = tag.peel_to_commit()?;
        let author = commit.author()?;
        let tag_name = tag.name().shorten().to_str()?;
        let name = author.name.to_str()?;
        let time = author.time()?.format(ISO8601);
        table.add_body_row([tag_name, &time, name]);
    }
    container.add_table(table);

    container.add_header(2, "Branches");
    let mut table = build_html::Table::new().with_header_row(["Name", "Time", "Author"]);
    for branch in refs.local_branches()? {
        let mut branch = branch.unwrap();
        let commit = branch.peel_to_commit()?;
        let author = commit.author()?;
        let branch_name = branch.name().shorten().to_str()?;
        let name = author.name.to_str()?;
        let time = author.time()?.format(ISO8601);
        table.add_body_row([branch_name, &time, name]);
    }
    container.add_table(table);
    Ok(container)
}

fn get_log(repo: &Repository) -> anyhow::Result<Container> {
    let mut container = build_html::Container::new(build_html::ContainerType::Div);
    container.add_header(2, "Log");
    let mut table = build_html::Table::new().with_header_row([
        "Time", "ID", "Message", "Author", "Files", "Lines added", "Lines removed",
    ]);
    let head = repo.head()?;
    let revs = repo
        .rev_walk([head.id().unwrap()])
        .first_parent_only()
        .all()?;
    for rev1 in revs {
        let rev = rev1?;
        let id = rev.id().to_string();
        let commit = rev.object()?;
        let message = commit.message()?.title.trim().to_str()?.to_owned();
        let author = commit.author()?;
        let name = author.name.to_string();
        let time = author.time()?.format(ISO8601);
        let tree = commit.tree()?;
        let ancestors = commit.ancestors().first_parent_only().all()?;
        let (changed, added, removed) = if let Some(ancestor) = ancestors.skip(1).next() {
            let commit2 = ancestor?.object()?;
            let ancestor_tree = commit2.tree()?;
            let stats = ancestor_tree.changes()?.stats(&tree)?;
            (
                stats.files_changed.to_string(),
                stats.lines_added.to_string(),
                stats.lines_removed.to_string(),
            )
        } else {
            (0.to_string(), 0.to_string(), 0.to_string())
        };
        table.add_body_row([time, id, message, name, changed, added, removed]);
    }
    container.add_table(table);
    Ok(container)
}

fn write_html_content(path: &Path, container: Container) -> anyhow::Result<()> {
    let page = HtmlPage::new()
        .with_title("Stagix")
        .with_container(container);
    std::fs::write(path, page.to_html_string())?;
    Ok(())
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    let repo = gix::open(args.repo)?;

    let refs = get_refs(&repo)?;
    write_html_content(&args.out_dir.join("refs.html"), refs)?;

    create_dir_all(args.out_dir.join("commits"))?;

    let log = get_log(&repo)?;
    write_html_content(&args.out_dir.join("log.html"), log)?;

    Ok(())
}
