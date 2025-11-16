use anyhow::Context as _;
use build_html::{
    Container, Html as _, HtmlContainer as _, HtmlElement, HtmlPage, Table, TableCell, TableRow,
    escape_html,
};
use gix::bstr::ByteSlice as _;
use gix::diff::blob::UnifiedDiff;
use gix::diff::blob::intern::InternedInput;
use gix::diff::blob::unified_diff::{ContextSize, NewlineSeparator};
use gix::objs::tree::EntryKind;
use gix::traverse::tree::Recorder;
use gix::{Repository, Tree};
use gix_date::time::format::ISO8601;
use html::Bold;
use nix::fcntl::{OFlag, RenameFlags, open, renameat2};
use nix::sys::stat::Mode;
use std::fs::{File, create_dir, create_dir_all, read_to_string, remove_dir_all};
use std::path::{Component, Path, PathBuf};
use std::time::Instant;
use tracing::info;
use tracing::{debug, warn};

mod html;

const README_FILES: [&str; 2] = ["README", "README.md"];
const LICENSE_FILES: [&str; 3] = ["LICENSE", "LICENSE.md", "COPYING"];

#[derive(Debug)]
pub struct Meta {
    pub description: String,
    pub url: String,
    pub name: String,
    pub owner: String,
    pub pages: Option<String>,
    pub readme: Option<String>,
    pub license: Option<String>,
}

impl Meta {
    pub fn load(repo: &Repository, path: &Path) -> anyhow::Result<Self> {
        debug!(repo =? repo.path(), ?path, "loading metadata for repo");
        let description = Self::load_meta_file(repo, "description")?.unwrap_or_default();
        if description.is_empty() {
            eprintln!("no description file found");
        }
        let url = Self::load_meta_file(repo, "url")?.unwrap_or_default();
        if url.is_empty() {
            eprintln!("no url file found");
        }
        let owner = Self::load_meta_file(repo, "owner")?.unwrap_or_default();
        if owner.is_empty() {
            eprintln!("no owner file found");
        }
        let pages = Self::load_meta_file(repo, "pages")?;
        if pages.is_none() {
            eprintln!("no pages file found");
        }
        let name = path
            .canonicalize()?
            .with_extension("")
            .file_name()
            .unwrap()
            .to_string_lossy()
            .into_owned();

        let head_tree = repo.head_tree()?;
        let mut readme = None;
        let mut license = None;
        for entry in head_tree.iter() {
            let entry = entry?;
            if !entry.mode().is_blob() {
                continue;
            }
            let filename = entry.filename().to_string();
            if README_FILES.contains(&filename.as_str()) {
                readme = Some(filename);
            } else if LICENSE_FILES.contains(&filename.as_str()) {
                license = Some(filename);
            }
        }

        Ok(Meta {
            description,
            url,
            name,
            owner,
            pages,
            readme,
            license,
        })
    }

    fn load_meta_file(repo: &Repository, name: &str) -> anyhow::Result<Option<String>> {
        debug!(repo =? repo.path(), ?name, "loading metadata file for repo");
        let path = repo.path().join(name);
        let path = PathBuf::from(path);
        if !path.is_file() {
            return Ok(None);
        }
        let content = read_to_string(path)?.trim().to_owned();
        Ok(Some(content))
    }

    pub fn write_html_content_to_file(
        &self,
        title: &str,
        filepath: &Path,
        container: Container,
        nav: bool,
        out_dir: &Path,
    ) -> anyhow::Result<()> {
        debug!(
            ?title,
            ?filepath,
            ?nav,
            ?out_dir,
            "writing html content to file"
        );
        let path = out_dir.join(filepath);
        let mut file = File::create(&path)?;
        let to_repo_root = to_root_path(&path, out_dir);
        let to_index_root = format!("../{}", to_repo_root);
        self.write_html_content(
            title,
            &to_index_root,
            &to_repo_root,
            container,
            nav,
            &mut file,
        )
    }

    pub fn write_html_content(
        &self,
        title: &str,
        to_index_root: &str,
        to_repo_root: &str,
        container: Container,
        nav: bool,
        out: &mut impl std::io::Write,
    ) -> anyhow::Result<()> {
        debug!(
            ?title,
            ?to_index_root,
            ?to_repo_root,
            ?nav,
            "writing html content to writer"
        );
        let mut head_table = Table::new();
        head_table.add_body_row([
            &HtmlElement::new(build_html::HtmlTag::Div)
                .with_link(
                    &format!("{}index.html", to_index_root),
                    HtmlElement::new(build_html::HtmlTag::Div)
                        .with_image_attr(
                            format!("{}logo.png", to_index_root),
                            "logo",
                            [("id", "logo")],
                        )
                        .to_html_string(),
                )
                .to_html_string(),
            &Container::new(build_html::ContainerType::Div)
                .with_header(1, &self.name)
                .with_html(
                    HtmlElement::new(build_html::HtmlTag::Span)
                        .with_attribute("class", "desc")
                        .with_raw(&self.description),
                )
                .to_html_string(),
        ]);
        if !self.url.is_empty() {
            head_table.add_body_row(["", &format!("git clone {}", self.url)]);
        }
        if nav {
            let mut nav = Container::new(build_html::ContainerType::Nav)
                .with_link(format!("{}log.html", to_repo_root), "Log")
                .with_raw(" | ")
                .with_link(format!("{}files.html", to_repo_root), "Files")
                .with_raw(" | ")
                .with_link(format!("{}refs.html", to_repo_root), "Refs");
            if let Some(readme) = &self.readme {
                nav.add_raw(" | ");
                nav.add_link(format!("{}files/{}.html", to_repo_root, readme), "README");
            }
            if let Some(license) = &self.license {
                nav.add_raw(" | ");
                nav.add_link(format!("{}files/{}.html", to_repo_root, license), "LICENSE");
            }
            head_table.add_body_row(["", &nav.to_html_string()]);
        }

        let page = HtmlPage::new()
            .with_title(format!("{} - {} - {}", title, self.name, self.description))
            .with_stylesheet(format!("{}style.css", to_index_root))
            .with_head_link(format!("{}favicon.png", to_index_root), "icon")
            .with_table(head_table)
            .with_html(HtmlElement::new(build_html::HtmlTag::HorizontalRule))
            .with_container(container);

        out.write_all(page.to_html_string().as_bytes())?;
        Ok(())
    }
}

#[derive(Debug)]
pub struct IndexOptions {
    pub out_dir: Option<PathBuf>,
    pub stylesheet: Option<PathBuf>,
    pub logo: Option<PathBuf>,
    pub favicon: Option<PathBuf>,
    pub pages_url: Option<String>,
}

pub fn build_index_page(repos: Vec<PathBuf>, options: IndexOptions) -> anyhow::Result<()> {
    info!(num_repos = repos.len(), ?options, "building index page");
    let index_meta = Meta {
        description: String::new(),
        url: String::new(),
        name: "Repositories".to_owned(),
        owner: String::new(),
        pages: None,
        readme: None,
        license: None,
    };

    let pages_url = options.pages_url.as_deref();

    let mut table = Table::new()
        .with_attributes([("id", "index")])
        .with_header_row(["Name", "Description", "Owner", "Last commit", "Pages URL"]);
    for repo_path in repos {
        if let Err(error) = add_row_for_repo_index(&repo_path, pages_url, &mut table) {
            warn!(?repo_path, %error, "Failed to add index row for repo");
        }
    }
    let container = Container::new(build_html::ContainerType::Div).with_table(table);

    if let Some(out_dir) = options.out_dir {
        let mut out = File::create(out_dir.join("index.html"))?;
        index_meta.write_html_content("Index", "", "", container, false, &mut out)?;
        if let Some(stylesheet) = options.stylesheet {
            std::fs::copy(stylesheet, out_dir.join("style.css"))?;
        }
        if let Some(logo) = options.logo {
            std::fs::copy(logo, out_dir.join("logo.png"))?;
        }
        if let Some(favicon) = options.favicon {
            std::fs::copy(favicon, out_dir.join("favicon.png"))?;
        }
    } else {
        let mut out = std::io::stdout();
        index_meta.write_html_content("Index", "", "", container, false, &mut out)?;
    };

    Ok(())
}

#[derive(Debug)]
pub struct PagesOptions {
    pub out_dir: PathBuf,
    pub working_dir: PathBuf,
}

pub fn build_pages_dirs(repos: Vec<PathBuf>, options: PagesOptions) -> anyhow::Result<()> {
    info!(num_repos = repos.len(), ?options, "building pages dir");

    if !options.out_dir.exists(){
        create_dir_all(&options.out_dir)?;
    }
    let out_dir = options.out_dir.canonicalize()?;
    if !options.working_dir.exists(){
        create_dir_all(&options.working_dir)?;
    }
    let working_dir = options.working_dir.canonicalize()?;

    for repo_path in repos {
        if options.working_dir.exists() {
            remove_dir_all(&working_dir)?;
        }
        create_dir_all(&working_dir)?;
        let abs_repo_path = repo_path.canonicalize()?;
        if let Err(error) = copy_docs_to_out_dir(&abs_repo_path, &out_dir, &working_dir) {
            warn!(?repo_path, ?out_dir, %error, "Failed to copy docs to out_dir");
        }
    }

    Ok(())
}

fn copy_docs_to_out_dir(
    repo_path: &Path,
    out_dir: &Path,
    working_dir: &Path,
) -> anyhow::Result<()> {
    debug!(?repo_path, ?out_dir, "Copying docs to out dir");
    let repo = gix::open(&repo_path)?;
    let head = repo.head_commit()?;
    let meta = Meta::load(&repo, &repo_path)?;

    let Some(repo_name) = repo_path.file_stem() else {
        anyhow::bail!("no repo name found")
    };

    let Some(docs_dir) = meta.pages else {
        return Ok(());
    };

    let docs_dir = if docs_dir.is_empty() {
        PathBuf::new()
    } else {
        PathBuf::from(docs_dir)
    };

    let docs_dir_parts = docs_dir.components();

    let root_tree = find_root_of_docs_dir(docs_dir_parts, head.tree()?)?;

    copy_tree_to_dir(root_tree, working_dir)?;

    let repo_out_dir = out_dir.join(repo_name);
    create_dir_all(&repo_out_dir)?;
    debug!(
        ?working_dir,
        ?repo_out_dir,
        "Moving working dir to repo out dir"
    );
    let working_dir_fd = open(
        working_dir,
        OFlag::O_DIRECTORY | OFlag::O_PATH,
        Mode::S_IWUSR | Mode::S_IWGRP,
    )?;
    let repo_out_dir_fd = open(
        &repo_out_dir,
        OFlag::O_DIRECTORY | OFlag::O_PATH,
        Mode::S_IWUSR | Mode::S_IWGRP,
    )?;
    // swap the directories atomically
    renameat2(
        working_dir_fd,
        working_dir,
        repo_out_dir_fd,
        &repo_out_dir,
        RenameFlags::RENAME_EXCHANGE,
    )?;
    // clean up the working dir
    remove_dir_all(&working_dir)?;

    Ok(())
}

fn copy_tree_to_dir(tree: Tree<'_>, working_dir: &Path) -> anyhow::Result<()> {
    debug!(?working_dir, "Copying tree to temporary dir");
    for entry in tree.iter() {
        let entry = entry?;
        let filename = entry.filename();
        if entry.mode().is_blob() {
            let blob = entry.object()?.into_blob();
            let file_path = working_dir.join(filename.to_str()?);
            std::fs::write(file_path, &blob.data)?;
        } else if entry.mode().is_tree() {
            let tree = entry.object()?.peel_to_tree()?;
            let dir_path = working_dir.join(filename.to_str()?);
            create_dir(&dir_path)?;
            copy_tree_to_dir(tree, &dir_path)?;
        } else {
            warn!(
                ?working_dir,
                ?filename,
                "unmatched mode in copy_tree_to_dir, not copying it"
            );
        }
    }

    Ok(())
}

fn find_root_of_docs_dir<'a, 'repo>(
    mut docs_dir: impl Iterator<Item = Component<'a>>,
    tree: Tree<'repo>,
) -> anyhow::Result<Tree<'repo>> {
    let Some(next_component) = docs_dir.next() else {
        debug!("found root of docs dir");
        return Ok(tree);
    };
    debug!(
        ?next_component,
        "find_root_of_docs_dir, looking for next piece"
    );
    let next_component = next_component.as_os_str().to_str().unwrap();

    for entry in tree.iter() {
        let entry = entry?;
        let filename = entry.filename();
        if filename == next_component && entry.mode().is_tree() {
            let tree = entry.object()?.peel_to_tree()?;
            return find_root_of_docs_dir(docs_dir, tree);
        }
    }

    Err(anyhow::anyhow!("root of docs dir not found"))
}

fn add_row_for_repo_index(
    repo_path: &Path,
    pages_url: Option<&str>,
    table: &mut Table,
) -> anyhow::Result<()> {
    let repo = gix::open(&repo_path)?;
    let head = repo.head_commit()?;
    let time = head.time()?.format(ISO8601);
    let meta = Meta::load(&repo, &repo_path)?;
    let name = HtmlElement::new(build_html::HtmlTag::Link)
        .with_attribute("href", format!("{}/log.html", meta.name))
        .with_raw(&meta.name)
        .to_html_string();
    let pages_url = meta
        .pages
        .and_then(|pages| {
            pages_url.map(|pages_url| {
                let pages_full_url = if pages_url.is_empty() {
                    pages.clone()
                } else {
                    format!("{pages_url}/{pages}")
                };
                HtmlElement::new(build_html::HtmlTag::Link)
                    .with_attribute("href", pages_full_url)
                    .with_raw(&pages)
                    .to_html_string()
            })
        })
        .unwrap_or_default();

    table.add_body_row([name, meta.description, meta.owner, time, pages_url]);
    Ok(())
}

fn get_refs(repo: &Repository) -> anyhow::Result<Container> {
    debug!(repo=?repo.path(), "get refs");
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

fn get_log(repo: &Repository, log_length: Option<usize>) -> anyhow::Result<Container> {
    debug!(repo=?repo.path(), log_length, "get log");
    let mut container = build_html::Container::new(build_html::ContainerType::Div);
    let mut table = build_html::Table::new()
        .with_attributes([("id", "log")])
        .with_header_row(["Time", "Commit message", "Author", "Files", "+", "-", "ID"]);
    let head = repo.head()?;
    let mut revs = repo
        .rev_walk([head.id().unwrap()])
        .first_parent_only()
        .all()?
        .enumerate();
    for (i, rev) in &mut revs {
        if let Some(log_len) = log_length {
            if i >= log_len {
                break;
            }
        }
        let rev = rev?;
        let id = rev.id().to_string();
        let commit = rev.object()?;
        let message = commit.message()?.title.trim().to_str()?.to_owned();
        let message_html = HtmlElement::new(build_html::HtmlTag::Div)
            .with_link_attr::<_, String>(format!("commits/{}.html", id), message, [])
            .to_html_string();
        let author = commit.author()?;
        let name = author.name.to_string();
        let time = author.time()?.format(ISO8601);
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
        let added = format!("+{}", stats.lines_added);
        let removed = format!("-{}", stats.lines_removed);

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
    let remaining = revs.count();
    if remaining > 0 {
        table.add_body_row([
            "...",
            &format!("{} more commits remaining, fetch the repository", remaining),
            "...",
            "...",
            "...",
            "...",
            "...",
        ]);
    }
    container.add_table(table);
    Ok(container)
}

fn get_commits(
    repo: &Repository,
    log_length: Option<usize>,
) -> anyhow::Result<Vec<(String, String, Container)>> {
    debug!(repo=?repo.path(), log_length, "get commits");
    let mut containers = Vec::new();
    let head = repo.head()?;
    let revs = repo
        .rev_walk([head.id().unwrap()])
        .first_parent_only()
        .all()?;
    for (i, rev) in revs.enumerate() {
        if let Some(log_len) = log_length {
            if i >= log_len {
                break;
            }
        }
        let rev = rev?;
        let mut container = build_html::Container::new(build_html::ContainerType::Div)
            .with_attributes([("id", "content")]);
        let mut pre = HtmlElement::new(build_html::HtmlTag::Div);

        pre.add_html(Bold::from("commit "));
        pre.add_link(
            format!("../commits/{}.html", rev.id),
            format!("{}\n", rev.id),
        );

        let commit = rev.object()?;
        let parent_revs = commit.parent_ids().map(|p| p.to_string());

        pre.add_html(Bold::from("parents "));
        for (j, parent_rev) in parent_revs.enumerate() {
            if j == 0 && Some(i + 1) != log_length {
                pre.add_link(format!("../commits/{}.html", parent_rev), parent_rev);
            } else {
                pre.add_child(parent_rev.into());
            }
        }
        pre.add_child("\n".into());

        let author = commit.author()?;

        pre.add_html(Bold::from("author "));
        pre.add_child(escape_html(&format!("{} <{}>\n", author.name, author.email)).into());

        pre.add_html(Bold::from("date "));
        pre.add_child(author.time()?.format(ISO8601).into());
        pre.add_child("\n".into());

        let message = commit.message()?;

        container.add_preformatted(pre);
        container.add_paragraph(message.title);
        container.add_paragraph(message.body.map_or(String::new(), |s| s.to_string()));

        let tree = commit.tree()?;
        let ancestors = commit.ancestors().first_parent_only().all()?;
        let ancestor = ancestors.skip(1).next();
        let ancestor_tree = if let Some(ancestor) = ancestor {
            let commit2 = ancestor?.object()?;
            let ancestor_tree = commit2.tree()?;
            ancestor_tree
        } else {
            repo.empty_tree()
        };

        let mut total_files_changed = 0;
        let mut total_lines_added = 0;
        let mut total_lines_removed = 0;
        let mut diffstat_table = Table::new();

        let mut resource_cache = repo.diff_resource_cache_for_tree_diff()?;

        let mut pre_diffs = Vec::new();
        ancestor_tree.changes()?.for_each_to_obtain_tree(
            &tree,
            |change| -> Result<gix::object::tree::diff::Action, std::convert::Infallible> {
                if change.entry_mode().is_tree() {
                    return Ok(gix::object::tree::diff::Action::Continue);
                }

                // diffstat
                let marker = match change {
                    gix::object::tree::diff::Change::Addition { .. } => "A",
                    gix::object::tree::diff::Change::Deletion { .. } => "D",
                    gix::object::tree::diff::Change::Modification { .. } => "M",
                    gix::object::tree::diff::Change::Rewrite { .. } => "R",
                };

                let mut lines_added = 0;
                let mut lines_removed = 0;

                let mut diff = change.diff(&mut resource_cache).unwrap();
                if let Some(counts) = diff.line_counts().unwrap() {
                    total_files_changed += 1;
                    lines_added += counts.insertions as usize;
                    lines_removed += counts.removals as usize;
                    total_lines_added += lines_added;
                    total_lines_removed += lines_removed;
                }

                let location = change.location().to_str().unwrap();
                diffstat_table.add_body_row([
                    marker,
                    &HtmlElement::new(build_html::HtmlTag::Link)
                        .with_attribute("href", &format!("#{}", location))
                        .with_raw(location)
                        .to_html_string(),
                    "|",
                    &format!("+{} -{}", lines_added, lines_removed),
                    &format!("{}{}", "+".repeat(lines_added), "-".repeat(lines_removed)),
                ]);

                // unified diff
                let (old_location, new_location) = match change {
                    gix::object::tree::diff::Change::Addition { location, .. } => {
                        (location, location)
                    }
                    gix::object::tree::diff::Change::Deletion { location, .. } => {
                        (location, location)
                    }
                    gix::object::tree::diff::Change::Modification { location, .. } => {
                        (location, location)
                    }
                    gix::object::tree::diff::Change::Rewrite {
                        source_location,
                        location,
                        ..
                    } => (source_location, location),
                };

                let location_marker = format!("--- {}\n+++ {}\n", old_location, new_location);
                let location_marker_html = HtmlElement::new(build_html::HtmlTag::Span)
                    .with_attribute("id", new_location)
                    .with_raw(location_marker)
                    .to_html_string();

                let old_string = ancestor_tree
                    .lookup_entry_by_path(change.location().to_str().unwrap())
                    .unwrap()
                    .map_or(String::new(), |entry| {
                        let blob = entry
                            .object()
                            .unwrap()
                            .try_into_blob()
                            .map_or(Vec::new(), |mut b| b.take_data());
                        let string =
                            String::from_utf8(blob).unwrap_or_else(|_| "binary_file".to_owned());
                        string
                    });
                let new_string = tree
                    .lookup_entry_by_path(change.location().to_str().unwrap())
                    .unwrap()
                    .map_or(String::new(), |entry| {
                        let blob = entry
                            .object()
                            .unwrap()
                            .try_into_blob()
                            .map_or(Vec::new(), |mut b| b.take_data());
                        let string =
                            String::from_utf8(blob).unwrap_or_else(|_| "binary_file".to_owned());
                        string
                    });
                let input = InternedInput::new(old_string.as_str(), new_string.as_str());
                let udiff = UnifiedDiff::new(
                    &input,
                    String::new(),
                    NewlineSeparator::AfterHeaderAndWhenNeeded("\n"),
                    ContextSize::symmetrical(5),
                );
                let diff =
                    gix::diff::blob::diff(gix::diff::blob::Algorithm::Histogram, &input, udiff)
                        .unwrap();

                pre_diffs.push(location_marker_html + &escape_html(&diff));

                Ok(gix::object::tree::diff::Action::Continue)
            },
        )?;

        container.add_paragraph(format!(
            "{} files changed, {} insertions(+), {} deletions(-)",
            total_files_changed, total_lines_added, total_lines_removed
        ));
        container.add_html(Bold::from("Diffstat:"));
        container.add_table(diffstat_table);
        container.add_html(HtmlElement::new(build_html::HtmlTag::HorizontalRule));
        for diff in pre_diffs {
            container.add_preformatted(diff);
        }
        let title = message.title.to_string();
        containers.push((commit.id.to_string(), title, container));
    }
    Ok(containers)
}

fn get_files(repo: &Repository) -> anyhow::Result<(Container, Vec<(PathBuf, Container)>)> {
    debug!(repo=?repo.path(), "get files");
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
        let mut content = Container::new(build_html::ContainerType::Div)
            .with_attributes([("id", "content")])
            .with_paragraph(format!(
                "{} ({}B)",
                entry.filepath.to_string(),
                obj.data.len()
            ))
            .with_html(HtmlElement::new(build_html::HtmlTag::HorizontalRule));

        let size = if let Ok(file_content) = str::from_utf8(&obj.data) {
            let lines: Vec<String> = file_content
                .lines()
                .enumerate()
                .map(|(i, line)| {
                    let link = HtmlElement::new(build_html::HtmlTag::Link)
                        .with_attribute("id", format!("l{}", i))
                        .with_attribute("href", format!("#l{}", i))
                        .with_attribute("class", "line")
                        .with_child(format!("{: >7} ", i).into())
                        .to_html_string();
                    let content = escape_html(line);
                    format!("{}{}", link, content)
                })
                .collect();

            content.add_preformatted_attr(&lines.join("\n"), [("id", "blob")]);

            format!("{}L", file_content.lines().count())
        } else {
            content.add_raw("binary file.");
            format!("{}B", obj.data.len())
        };

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
                        .with_raw(size),
                ),
        );
    }
    list_container.add_table(table);

    Ok((list_container, entries))
}

pub fn build_repo_pages(
    repo_path: &Path,
    out_dir: &Path,
    log_length: Option<usize>,
) -> anyhow::Result<()> {
    info!(?repo_path, ?out_dir, ?log_length, "build repo pages");
    let start = Instant::now();
    let out_dir = out_dir.canonicalize()?;
    let repo = gix::open(&repo_path).context("open repo")?;

    let meta = Meta::load(&repo, repo_path)?;

    let refs = get_refs(&repo).context("get refs")?;
    meta.write_html_content_to_file("Refs", &PathBuf::from("refs.html"), refs, true, &out_dir)?;

    let (file_list, files) = get_files(&repo).context("get files")?;
    create_dir_all(out_dir.join("files"))?;
    for (path, content) in files {
        create_dir_all(out_dir.join("files").join(path.parent().unwrap()))?;
        meta.write_html_content_to_file(
            path.with_extension("")
                .file_name()
                .unwrap()
                .to_str()
                .unwrap(),
            &PathBuf::from("files").join(&path),
            content,
            true,
            &out_dir,
        )?;
    }
    meta.write_html_content_to_file(
        "Files",
        &PathBuf::from("files.html"),
        file_list,
        true,
        &out_dir,
    )?;

    let log = get_log(&repo, log_length).context("get log")?;
    meta.write_html_content_to_file("Log", &PathBuf::from("log.html"), log, true, &out_dir)?;

    let commits = get_commits(&repo, log_length).context("get commits")?;
    create_dir_all(out_dir.join("commits"))?;
    for (id, title, commit) in commits {
        meta.write_html_content_to_file(
            &title,
            &PathBuf::from("commits").join(&id).with_extension("html"),
            commit,
            true,
            &out_dir,
        )?;
    }
    info!(?out_dir, elapsed=? start.elapsed(), "Built repo");
    Ok(())
}

fn to_root_path(from: &Path, to: &Path) -> String {
    let path = from.strip_prefix(to).unwrap();
    "../".repeat(path.components().count().saturating_sub(1))
}
