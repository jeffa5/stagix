use anyhow::Context as _;
use build_html::{Container, Html, HtmlContainer, HtmlPage, TableCell, TableRow, escape_html};
use build_html::{HtmlElement, Table};
use clap::Parser;
use gix::Repository;
use gix::bstr::ByteSlice as _;
use gix::objs::tree::EntryKind;
use gix::traverse::tree::Recorder;
use gix_date::time::format::ISO8601;
use std::env::current_dir;
use std::fs::{create_dir_all, read_to_string};
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
    let mut table = build_html::Table::new()
        .with_attributes([("id", "tags")])
        .with_header_row(["Name", "Last commit time", "Author"]);
    let mut has_tags = false;
    for tag in refs.tags()? {
        let mut tag = tag.unwrap();
        let commit = tag.peel_to_commit()?;
        let author = commit.author()?;
        let tag_name = tag.name().shorten().to_str()?;
        let name = author.name.to_str()?;
        let time = author.time()?.format(ISO8601);
        table.add_body_row([tag_name, &time, name]);
        has_tags = true;
    }
    if has_tags {
        container.add_header(2, "Tags");
        container.add_table(table);
    }

    container.add_header(2, "Branches");
    let mut table = build_html::Table::new()
        .with_attributes([("id", "branches")])
        .with_header_row(["Name", "Last commit time", "Author"]);
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
    let mut table = build_html::Table::new()
        .with_attributes([("id", "log")])
        .with_header_row(["Time", "Commit message", "Author", "Files", "+", "-", "ID"]);
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
        table.add_custom_body_row(
            TableRow::new()
                .with_cell(TableCell::default().with_raw(time))
                .with_cell(TableCell::default().with_raw(message_html))
                .with_cell(TableCell::default().with_raw(name))
                .with_cell(
                    TableCell::default()
                        .with_attributes([("class", "num")])
                        .with_raw(changed),
                )
                .with_cell(
                    TableCell::default()
                        .with_attributes([("class", "num")])
                        .with_raw(added),
                )
                .with_cell(
                    TableCell::default()
                        .with_attributes([("class", "num")])
                        .with_raw(removed),
                )
                .with_cell(TableCell::default().with_raw(id)),
        );
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
        table.add_body_row(["Last commit time", &author.time()?.format(ISO8601)]);
        container.add_table(table);

        container.add_paragraph(message.title);
        container.add_paragraph(message.body.map_or(String::new(), |s| s.to_string()));

        let tree = commit.tree()?;
        let ancestors = commit.ancestors().first_parent_only().all()?;
        let ancestor_tree = if let Some(ancestor) = ancestors.skip(1).next() {
            let commit2 = ancestor?.object()?;
            let ancestor_tree = commit2.tree()?;
            ancestor_tree
        } else {
            repo.empty_tree()
        };
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
                            container.add_raw("addition");
                            let html_lines: Vec<String> =
                                lines.into_iter().map(|l| format!("+ {}", l)).collect();
                            container.add_preformatted(html_lines.join("\n"));
                        }
                        gix::object::blob::diff::lines::Change::Deletion { lines } => {
                            container.add_raw("deletion");
                            let html_lines: Vec<String> =
                                lines.into_iter().map(|l| format!("- {}", l)).collect();
                            container.add_preformatted(html_lines.join("\n"));
                        }
                        gix::object::blob::diff::lines::Change::Modification {
                            lines_before,
                            lines_after,
                        } => {
                            container.add_raw("modification");
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
    let mut table = Table::new()
        .with_attributes([("id", "files")])
        .with_header_row(["Mode", "Name", "Size"]);
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
        table.add_custom_body_row(
            TableRow::new()
                .with_cell(TableCell::default().with_raw(mode))
                .with_cell(
                    TableCell::default().with_html(
                        HtmlElement::new(build_html::HtmlTag::Span)
                            .with_link(format!("files/{}.html", path), path)
                            .to_html_string(),
                    ),
                )
                .with_cell(
                    TableCell::default()
                        .with_attributes([("class", "num")])
                        .with_raw(format!("{}L", file_data.lines().count())),
                ),
        );
    }
    list_container.add_table(table);

    Ok((list_container, entries))
}

struct Meta {
    description: String,
    url: String,
    name: String,
}

impl Meta {
    fn load(repo: &Repository) -> anyhow::Result<Self> {
        let description = Self::load_meta_file(repo, "description").unwrap_or_default();
        if description.is_empty() {
            eprintln!("no description file found");
        }
        let url = Self::load_meta_file(repo, "url").unwrap_or_default();
        if url.is_empty() {
            eprintln!("no url file found");
        }
        let name = current_dir()?;
        let name = name.file_name().unwrap().to_string_lossy().into_owned();

        Ok(Meta {
            description,
            url,
            name,
        })
    }

    fn load_meta_file(repo: &Repository, name: &str) -> anyhow::Result<String> {
        let path = if repo.is_bare() {
            name.to_string()
        } else {
            format!(".git/{}", name)
        };
        let path = PathBuf::from(path);
        let content = read_to_string(path)?;
        Ok(content)
    }

    fn write_html_content(&self, path: &Path, container: Container) -> anyhow::Result<()> {
        let to_root = "../".repeat(path.components().count().saturating_sub(2));
        let mut head_table = Table::new();
        head_table.add_body_row([
            "",
            &Container::new(build_html::ContainerType::Div)
                .with_header(1, &self.name)
                .with_html(
                    HtmlElement::new(build_html::HtmlTag::Span)
                        .with_attribute("class", "desc")
                        .with_raw(&self.description),
                )
                .to_html_string(),
        ]);
        head_table.add_body_row(["", &format!("git clone {}", self.url)]);
        let nav = Container::new(build_html::ContainerType::Nav)
            .with_link(format!("{}log.html", to_root), "Log")
            .with_raw(" | ")
            .with_link(format!("{}files.html", to_root), "Files")
            .with_raw(" | ")
            .with_link(format!("{}refs.html", to_root), "Refs");
        head_table.add_body_row(["", &nav.to_html_string()]);

        let page = HtmlPage::new()
            .with_title(format!("Stagix | {}", self.name))
            .with_stylesheet(format!("{}style.css", to_root))
            .with_table(head_table)
            .with_html(HtmlElement::new(build_html::HtmlTag::HorizontalRule))
            .with_container(container);

        std::fs::write(path, page.to_html_string()).context(path.to_string_lossy().into_owned())?;
        Ok(())
    }
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    let repo = gix::open(args.repo).context("open repo")?;

    let meta = Meta::load(&repo)?;

    let refs = get_refs(&repo).context("get refs")?;
    meta.write_html_content(&args.out_dir.join("refs.html"), refs)?;

    let (file_list, files) = get_files(&repo).context("get files")?;
    create_dir_all(args.out_dir.join("files"))?;
    for (path, content) in files {
        create_dir_all(args.out_dir.join("files").join(path.parent().unwrap()))?;
        meta.write_html_content(&args.out_dir.join("files").join(path), content)?;
    }
    meta.write_html_content(&args.out_dir.join("files.html"), file_list)?;

    let log = get_log(&repo).context("get log")?;
    meta.write_html_content(&args.out_dir.join("log.html"), log)?;

    let commits = get_commits(&repo).context("get commits")?;
    create_dir_all(args.out_dir.join("commits"))?;
    for (id, commit) in commits {
        meta.write_html_content(&args.out_dir.join("commits").join(id), commit)?;
    }

    Ok(())
}
