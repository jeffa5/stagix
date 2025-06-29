use anyhow::Context as _;
use build_html::{Container, Html, HtmlContainer, HtmlPage, escape_html};
use build_html::{HtmlElement, Table};
use clap::Parser;
use gix::Repository;
use gix::bstr::ByteSlice as _;
use gix::objs::tree::EntryKind;
use gix::traverse::tree::Recorder;
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
        "Time",
        "Message",
        "Author",
        "Files",
        "Lines added",
        "Lines removed",
        "ID",
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
        let message_html = HtmlElement::new(build_html::HtmlTag::Div)
            .with_link_attr::<_, String>(format!("commits/{}", id), message, [])
            .to_html_string();
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
        table.add_body_row([time, message_html, name, changed, added, removed, id]);
    }
    container.add_table(table);
    Ok(container)
}

fn get_commits(repo: &Repository) -> anyhow::Result<Vec<(String, Container)>> {
    let mut containers = Vec::new();
    let head = repo.head()?;
    let revs = repo
        .rev_walk([head.id().unwrap()])
        .first_parent_only()
        .all()?;
    for rev in revs {
        let rev = rev?;
        let mut container = build_html::Container::new(build_html::ContainerType::Div);
        container.add_header(2, "Commit");

        let mut table = Table::new();
        table.add_body_row(["Revision", &rev.id.to_string()]);
        let commit = rev.object()?;
        let message = commit.message()?;

        let author = commit.author()?;
        table.add_body_row([
            "Author",
            &escape_html(&format!("{} <{}>", author.name, author.email)),
        ]);
        table.add_body_row(["Time", &author.time()?.format(ISO8601)]);
        container.add_table(table);

        container.add_paragraph(message.title);
        container.add_paragraph(message.body.map_or(String::new(), |s| s.to_string()));

        let tree = commit.tree()?;
        let ancestors = commit.ancestors().first_parent_only().all()?;
        if let Some(ancestor) = ancestors.skip(1).next() {
            let commit2 = ancestor?.object()?;
            let ancestor_tree = commit2.tree()?;
            let stats = ancestor_tree.changes()?.stats(&tree)?;
            let changed = stats.files_changed.to_string();
            let added = stats.lines_added.to_string();
            let removed = stats.lines_removed.to_string();
            container.add_paragraph(format!(
                "Files changed {}, Lines added {}, Lines removed {}",
                changed, added, removed
            ));

            let mut resource_cache = repo.diff_resource_cache_for_tree_diff()?;
            ancestor_tree.changes()?.for_each_to_obtain_tree(
                &tree,
                |change| -> Result<gix::object::tree::diff::Action, std::convert::Infallible> {
                    if change.entry_mode().is_tree() {
                        return Ok(gix::object::tree::diff::Action::Continue);
                    }
                    container.add_preformatted(format!("{}", change.location()));
                    let mut diff = change.diff(&mut resource_cache).unwrap();
                    diff.lines(|change_line| -> Result<(), std::convert::Infallible> {
                        match change_line {
                            gix::object::blob::diff::lines::Change::Addition { lines } => {
                                let html_lines: Vec<String> =
                                    lines.into_iter().map(|l| format!("+ {}", l)).collect();
                                container.add_preformatted(html_lines.join("\n"));
                            }
                            gix::object::blob::diff::lines::Change::Deletion { lines } => {
                                let html_lines: Vec<String> =
                                    lines.into_iter().map(|l| format!("- {}", l)).collect();
                                container.add_preformatted(html_lines.join("\n"));
                            }
                            gix::object::blob::diff::lines::Change::Modification {
                                lines_before,
                                lines_after,
                            } => {
                                let html_lines_before =
                                    lines_before.into_iter().map(|l| format!("- {}", l));
                                let html_lines_after =
                                    lines_after.into_iter().map(|l| format!("+ {}", l));
                                let html_lines: Vec<String> =
                                    html_lines_before.chain(html_lines_after).collect();
                                container.add_preformatted(html_lines.join("\n"));
                            }
                        }
                        Ok(())
                    })
                    .unwrap();
                    Ok(gix::object::tree::diff::Action::Continue)
                },
            )?;
        };
        containers.push((commit.id.to_string(), container));
    }
    Ok(containers)
}

fn get_files(repo: &Repository) -> anyhow::Result<(Container, Vec<(PathBuf, Container)>)> {
    let head_tree = repo.head_tree()?;
    let mut recorder = Recorder::default();
    head_tree.traverse().depthfirst(&mut recorder)?;

    let mut entries = Vec::new();
    let mut list_container = Container::new(build_html::ContainerType::Div);
    let mut table = Table::new().with_header_row(["Mode", "Name", "Size"]);
    for entry in recorder.records {
        let mode = match entry.mode.kind() {
            EntryKind::Tree => continue,
            EntryKind::Blob => "-rw-r--r--",
            EntryKind::BlobExecutable => "-rwxr-xr-x",
            EntryKind::Link => continue,
            EntryKind::Commit => continue,
        };
        let obj = repo.find_object(entry.oid)?;

        let path = PathBuf::from(format!("{}.html", entry.filepath.to_string()));
        let file_data = str::from_utf8(&obj.data)?;
        let file_data_with_line_nums: Vec<String> = file_data
            .lines()
            .enumerate()
            .map(|(i, line)| format!("{: >4} | {}", i, line))
            .collect();
        let content = Container::new(build_html::ContainerType::Div)
            .with_preformatted(escape_html(&file_data_with_line_nums.join("\n")));
        entries.push((path, content));

        let path = escape_html(&entry.filepath.to_string());
        table.add_body_row([
            mode,
            &HtmlElement::new(build_html::HtmlTag::Span)
                .with_link(format!("files/{}.html", path), path)
                .to_html_string(),
            &file_data.len().to_string(),
        ]);
    }
    list_container.add_table(table);

    Ok((list_container, entries))
}

fn write_html_content(path: &Path, container: Container) -> anyhow::Result<()> {
    let to_root = "../".repeat(path.components().count().saturating_sub(2));
    let page = HtmlPage::new()
        .with_title("Stagix")
        .with_container(
            Container::new(build_html::ContainerType::Nav)
                .with_link(format!("{}log.html", to_root), "Log")
                .with_raw(" | ")
                .with_link(format!("{}files.html", to_root), "Files")
                .with_raw(" | ")
                .with_link(format!("{}refs.html", to_root), "Refs")
                .with_html(HtmlElement::new(build_html::HtmlTag::HorizontalRule)),
        )
        .with_container(container);
    std::fs::write(path, page.to_html_string()).context(path.to_string_lossy().into_owned())?;
    Ok(())
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    let repo = gix::open(args.repo).context("open repo")?;

    let refs = get_refs(&repo).context("get refs")?;
    write_html_content(&args.out_dir.join("refs.html"), refs)?;

    let (file_list, files) = get_files(&repo).context("get files")?;
    create_dir_all(args.out_dir.join("files"))?;
    for (path, content) in files {
        create_dir_all(args.out_dir.join("files").join(path.parent().unwrap()))?;
        write_html_content(&args.out_dir.join("files").join(path), content)?;
    }
    write_html_content(&args.out_dir.join("files.html"), file_list)?;

    let log = get_log(&repo).context("get log")?;
    write_html_content(&args.out_dir.join("log.html"), log)?;

    let commits = get_commits(&repo).context("get commits")?;
    create_dir_all(args.out_dir.join("commits"))?;
    for (id, commit) in commits {
        write_html_content(&args.out_dir.join("commits").join(id), commit)?;
    }

    Ok(())
}
