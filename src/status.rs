use super::*;
use clap::*;

pub struct StatusArgs {
  status_option: StatusOptions,
  diff_filter: DiffFilter,
  show_option: ShowOption,
  is_short: bool,
  show_patch: bool,
  all: bool,
}

impl StatusArgs {
  pub fn build_arg() -> Command {
    return Command::new("status")
    .about("Collect status information across all submodules")
    .arg(
      Arg::new("staged")
        .long("staged")
        .short('S')
        .action(ArgAction::SetTrue)
        .help("Only show staged changes")
        .conflicts_with("work-tree"),
    )
    .arg(
      Arg::new("work-tree")
        .long("work-tree")
        .short('w')
        .action(ArgAction::SetTrue)
        .help("Only show working tree changes (un-staged)"),
    )
    .arg(
      Arg::new("include-ignored")
        .long("ignored")
        .short('i')
        .action(ArgAction::SetTrue)
        .help("Include ignored files"),
    )
    .arg(
      Arg::new("diff-filter")
        .long("diff-filter")
        .short('f')
        .help("Filter changes with it's status.\nA = Add, D = Delete, M = Modified, R = Rename,\nT = Type changed, U = Unknown\nlowercases will exclude those flags"),
    )
    .arg(
      Arg::new("short")
        .long("short")
        .short('s')
        .action(ArgAction::SetTrue)
        .help("Only show summary of dirty submodules"),
    )
    .arg(
      Arg::new("patch")
        .long("patch")
        .short('p')
        .action(ArgAction::SetTrue)
        .help("Show patch"),
    )
    .arg(
      Arg::new("all")
        .long("all")
        .short('a')
        .action(ArgAction::SetTrue)
        .help("Show all submodules regardless it is dirty or not"),
    )
    .arg(
      Arg::new("pathspec")
      .action(ArgAction::Set)
      .help("Filter commits by the pathspec")
    );
  }
}

impl From<&clap::ArgMatches> for StatusArgs {
  fn from(matches: &clap::ArgMatches) -> StatusArgs {
    // prepare status option
    let mut status_option = StatusOptions::new();
    status_option
      .exclude_submodules(true)
      .include_untracked(true)
      .renames_head_to_index(true);

    let show = if matches.get_flag("staged") {
      ShowOption::Index
    } else if matches.get_flag("work-tree") {
      ShowOption::WorkTree
    } else {
      ShowOption::Both
    };
    if let Some(p) = matches.get_one::<String>("pathspec") {
      status_option.pathspec(p);
    }
    status_option.include_ignored(matches.get_flag("include-ignored"));
    status_option.recurse_untracked_dirs(matches.get_flag("patch"));

    // prepare diff filter
    let diff_filter = match matches.get_one::<String>("diff-filter") {
      Some(s) => DiffFilter::from(s),
      None => DiffFilter::default(),
    };

    return StatusArgs {
      status_option: status_option,
      diff_filter: diff_filter,
      show_option: show,
      is_short: matches.get_flag("short"),
      show_patch: matches.get_flag("patch"),
      all: matches.get_flag("all"),
    };
  }
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum ShowOption {
  Index,
  WorkTree,
  Both,
}
// helper to check if the status is staged
fn is_staged(status: Status) -> bool {
  if status.is_index_new()
    || status.is_index_modified()
    || status.is_index_deleted()
    || status.is_index_renamed()
    || status.is_index_typechange()
  {
    true
  } else {
    false
  }
}

// print Statuses
// callback to print diff patch
fn print_callback(_: DiffDelta<'_>, _: Option<DiffHunk<'_>>, line: DiffLine<'_>) -> bool {
  if line.origin() == 'F' || line.origin() == 'B' {
    print!("{}", String::from_utf8_lossy(line.content()));
  } else if line.origin() == 'H' {
    print!("{}", String::from_utf8_lossy(line.content()).cyan());
  } else {
    let msg = format!(
      "{} {}",
      line.origin(),
      String::from_utf8_lossy(line.content())
    );
    let colored_msg = if msg.starts_with('+') {
      msg.green()
    } else if msg.starts_with('-') {
      msg.red()
    } else {
      msg.default()
    };
    print!("{}", colored_msg);
  }
  return true;
}

// print patch
pub fn print_patch<'a>(repo: &Repository, delta: &DiffDelta, status: Status) {
  if delta.new_file().mode() == FileMode::Commit || delta.old_file().mode() == FileMode::Commit {
    let old_name = delta
      .old_file()
      .path()
      .map(|p| p.to_owned())
      .unwrap_or(PathBuf::new());
    let new_name = delta
      .new_file()
      .path()
      .map(|p| p.to_owned())
      .unwrap_or(PathBuf::new());
    println!(
      "diff --git a/{} b/{}",
      old_name.display(),
      new_name.display()
    );
    println!(
      "index {}..{} 160000",
      &delta.old_file().id().to_string()[..7],
      &delta.new_file().id().to_string()[..7]
    );
    println!("--- a/{}", old_name.display());
    println!("+++ b/{}", new_name.display());
    println!("{}", "@@ -1 +1 @@".cyan());
    println!(
      "{}",
      format!("-Subproject commit {}", delta.old_file().id()).red()
    );
    println!(
      "{}",
      format!("+Subproject commit {}", delta.new_file().id()).green()
    );
    return;
  }
  let work_path = repo.workdir().expect("Get repo directory failed");
  if status.is_wt_new() {
    // new file case
    // old file = empty
    // new file = working tree file
    let new_path = work_path.join(delta.new_file().path().expect("Get new file path failed"));
    let new_buffer = std::fs::read(&new_path).expect("Read new file failed");
    Patch::from_buffers(&[], None, &new_buffer, delta.new_file().path(), None)
      .expect("Get patch failed")
      .print(&mut print_callback)
      .unwrap();
  } else if status.is_index_new() {
    // new file in stage
    // old file = empty
    // new file = blob
    // libgit doesn't provide such way, so use blob -> buffer and reverse it.
    let new_blob = repo
      .find_blob(delta.new_file().id())
      .expect("Find blob failed");
    let new_path = delta.new_file().path();
    Patch::from_blob_and_buffer(
      &new_blob,
      new_path,
      &[],
      None,
      Some(DiffOptions::new().reverse(true)),
    )
    .expect("Get patch failed")
    .print(&mut print_callback)
    .unwrap();
  } else {
    let old_blob = repo
      .find_blob(delta.old_file().id())
      .expect("Find blob failed");
    let old_path = delta.old_file().path();
    if !is_staged(status) {
      // work tree change
      // old file = blob (should from index)
      // new file = working tree file
      let new_path = work_path.join(delta.new_file().path().expect("Get new file path failed"));
      let new_buffer = std::fs::read(&new_path).expect("Read new file failed");
      Patch::from_blob_and_buffer(
        &old_blob,
        old_path,
        &new_buffer,
        delta.new_file().path(),
        None,
      )
      .expect("Get patch failed")
      .print(&mut print_callback)
      .unwrap();
    } else {
      // staged change
      // old file = blob (should from HEAD)
      // new file = blob (should from index)
      let new_blob = repo
        .find_blob(delta.new_file().id())
        .expect("Find blob failed");
      let new_path = delta.new_file().path();
      Patch::from_blobs(&old_blob, old_path, &new_blob, new_path, None)
        .expect("Get patch failed")
        .print(&mut print_callback)
        .unwrap();
    }
  }
}

// get the label of the change status
fn status_to_str(status: Status) -> ColoredString {
  if status.is_index_new() {
    "A ".green()
  } else if status.is_index_modified() {
    "M ".green()
  } else if status.is_index_deleted() {
    "D ".green()
  } else if status.is_index_renamed() {
    "R ".green()
  } else if status.is_index_typechange() {
    "T ".green()
  } else if status.is_wt_new() {
    "??".red()
  } else if status.is_wt_modified() {
    " M".red()
  } else if status.is_wt_deleted() {
    " D".red()
  } else if status.is_wt_typechange() {
    " T".red()
  } else if status.is_wt_renamed() {
    " R".red()
  } else if status.is_ignored() {
    "!!".red()
  } else {
    "??".red()
  }
}

fn show_statuses(statuses: &Vec<StatusEntry>, repo: &Repository, patch: bool) {
  for st in statuses.iter() {
    if st.status().is_index_renamed() || st.status().is_wt_renamed() {
      let delta = if st.status().is_index_renamed() {
        st.head_to_index().expect("Get head to index delta failed")
      } else {
        st.index_to_workdir()
          .expect("Get index to working tree delta failed")
      };
      let old_file = delta.old_file().path().expect("Get old file path failed");
      let new_file = delta.new_file().path().expect("Get new file path failed");
      println!(
        " {} {} -> {}",
        status_to_str(st.status()),
        old_file.display(),
        new_file.display()
      );
    } else {
      println!(
        " {} {}",
        status_to_str(st.status()),
        st.path().unwrap_or_else(|| {
          err_exit!("Extract path failed");
        })
      );
    }
    if patch {
      let delta = if is_staged(st.status()) {
        st.head_to_index().expect("Get head to index delta failed")
      } else {
        st.index_to_workdir()
          .expect("Get index to working tree delta failed")
      };

      print_patch(repo, &delta, st.status());
    }
  }
}

// recursively list change of the repo and it's submodule
pub fn show_repo_status(repo: &Repository, work_dir: &PathBuf, head: Oid, args: &mut StatusArgs) {
  let index_statuses = match args.show_option {
    ShowOption::Both | ShowOption::Index => Some(
      repo
        .statuses(Some(args.status_option.show(StatusShow::Index)))
        .unwrap_or_else(|e| {
          err_exit!("Get status failed: {}", e);
        }),
    ),
    _ => None,
  };
  let index_stat_vec = if let Some(ref s) = index_statuses {
    s.iter()
      .filter(|s| args.diff_filter.test(s.status()))
      .collect()
  } else {
    Vec::new()
  };
  let work_tree_statuses = match args.show_option {
    ShowOption::Both | ShowOption::WorkTree => Some(
      repo
        .statuses(Some(args.status_option.show(StatusShow::Workdir)))
        .unwrap_or_else(|e| {
          err_exit!("Get status failed: {}", e);
        }),
    ),
    _ => None,
  };
  let work_tree_stat_vec = if let Some(ref s) = work_tree_statuses {
    s.iter()
      .filter(|s| args.diff_filter.test(s.status()))
      .collect()
  } else {
    Vec::new()
  };
  let head_id = repo
    .head()
    .expect("Extract head failed")
    .resolve()
    .expect("Resolve reference failed")
    .target()
    .expect("Get oid failed");
  if args.all
    || !index_stat_vec.is_empty()
    || !work_tree_stat_vec.is_empty()
    || repo.state() != RepositoryState::Clean
    || head_id != head
  {
    // make and print repo header
    let mut repo_dir = repo
      .workdir()
      .unwrap_or_else(|| {
        err_exit!("Extract path failed");
      })
      .canonicalize()
      .unwrap_or_else(|e| {
        err_exit!("Get canonicalize path failed: {}", e);
      });
    if repo_dir != *work_dir {
      if let Ok(p) = repo_dir.strip_prefix(work_dir) {
        repo_dir = Path::new(".").join(p);
      }
    }
    let repo_str = repo_dir.display().to_string().replace("\\", "/");
    print!(
      "{} @ {}",
      format!(
        "Repo: {}",
        repo_str.strip_prefix("//?/").unwrap_or(&repo_str)
      )
      .bright_blue(),
      &head_id.to_string()[..7].green()
    );
    if repo.state() != RepositoryState::Clean {
      print!(" | {}", format!("State: {:?}", repo.state()).purple());
    }
    print!("\n");

    if head_id != head {
      println!("Repo head changed:\n From {}\n To   {}", head, head_id);
    }

    println!("{} changes staged", index_stat_vec.len());
    println!("{} changes in working tree", work_tree_stat_vec.len());
    if !args.is_short {
      // print staged changes
      if args.show_option == ShowOption::Both || args.show_option == ShowOption::Index {
        show_statuses(&index_stat_vec, repo, args.show_patch);
      }
      // print un-staged changes
      if args.show_option == ShowOption::Both || args.show_option == ShowOption::WorkTree {
        show_statuses(&work_tree_stat_vec, repo, args.show_patch);
      }
    }
  }

  // recurse submodules
  for sub in repo
    .submodules()
    .unwrap_or_else(|e| {
      err_exit!("Get submodules failed: {}", e);
    })
    .iter()
  {
    show_repo_status(
      &sub.open().unwrap_or_else(|e| {
        err_exit!("Open repo failed, not a git repo? {}", e);
      }),
      work_dir,
      sub.head_id().expect("Get submodule head id failed"),
      args,
    );
  }
}
