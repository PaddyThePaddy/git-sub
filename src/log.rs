use super::*;
use chrono::prelude::*;
use clap::*;
use git2::*;
use regex::Regex;
use std::cmp::Ordering;
use std::collections::binary_heap::BinaryHeap;
use std::path::*;

pub struct LogArgs {
  pathspec: Option<Pathspec>,
  all: bool,
  author: Option<Regex>,
  grep: Option<Regex>,
  head: Option<String>,
  print_full: bool,
  print_patch: bool,
  print_list: bool,
  num: Option<usize>,
  start: Option<usize>,
}

impl LogArgs {
  pub fn build_arg() -> Command {
    Command::new("log")
      .about("Collect and show log across all submodules")
      .arg(
        clap::Arg::new("all")
          .long("all")
          .short('a')
          .action(ArgAction::SetTrue)
          .help("Search commits on all branch"),
      )
      .arg(
        clap::Arg::new("author")
          .long("author")
          .help("Filter commits by author"),
      )
      .arg(
        clap::Arg::new("revision")
          .long("revision")
          .short('r')
          .help("Filter commits starting from the specific reference of the root repo"),
      )
      .arg(
        clap::Arg::new("pathspec")
          .action(ArgAction::Append)
          .help("Filter commits by the pathspec"),
      )
      .arg(
        clap::Arg::new("grep")
          .long("grep")
          .help("Filter commits by commit message"),
      )
      .arg(
        clap::Arg::new("list")
          .long("list")
          .short('l')
          .action(ArgAction::SetTrue)
          .help("List file of each commit"),
      )
      .arg(
        clap::Arg::new("full")
          .long("full")
          .short('f')
          .action(ArgAction::SetTrue)
          .help("Show long format of each commit"),
      )
      .arg(
        clap::Arg::new("patch")
          .long("patch")
          .short('p')
          .action(ArgAction::SetTrue)
          .help("Show patch of each commit"),
      )
      .arg(
        clap::Arg::new("num")
          .long("num")
          .short('n')
          .action(ArgAction::Set)
          .help("Set the number of log to be displayed"),
      )
      .arg(
        clap::Arg::new("start")
          .long("start")
          .short('s')
          .action(ArgAction::Set)
          .help("Set the number of log to start to displayed"),
      )
  }
}

impl From<&clap::ArgMatches> for LogArgs {
  fn from(matches: &clap::ArgMatches) -> LogArgs {
    let author_pattern = matches
      .get_one::<&str>("author")
      .map(|s| Regex::new(s).unwrap_or_else(|_| err_exit!("Crate regex for author failed")));
    let grep_pattern = matches
      .get_one::<&str>("grep")
      .map(|s| Regex::new(s).unwrap_or_else(|_| err_exit!("Crate regex for grep failed")));
    return LogArgs {
      pathspec: matches
        .get_many::<String>("pathspec")
        .map(|s| Pathspec::new(s).unwrap_or_else(|_| err_exit!("Crate pathspec failed"))),
      all: matches.get_flag("all"),
      author: author_pattern,
      grep: grep_pattern,
      head: matches.get_one::<String>("revision").map(|s| s.clone()),
      print_full: matches.get_flag("full"),
      print_patch: matches.get_flag("patch"),
      print_list: matches.get_flag("list"),
      num: matches.get_one::<String>("num").map(|s| {
        s.parse::<usize>()
          .unwrap_or_else(|e| err_exit!("Error while parsing -n option: {}", e))
      }),
      start: matches.get_one::<String>("start").map(|s| {
        s.parse::<usize>()
          .unwrap_or_else(|e| err_exit!("Error while parsing -s option: {}", e))
      }),
    };
  }
}

struct CommitWrapper<'a> {
  c: Commit<'a>,
  t: Time,
  p: &'a Path,
  r: &'a Repository,
}

impl<'a> CommitWrapper<'a> {
  fn new(c: Commit<'a>, repo_path: &'a Path, repo: &'a Repository) -> CommitWrapper<'a> {
    CommitWrapper {
      t: c.time(),
      c: c,
      p: repo_path,
      r: repo,
    }
  }
  fn new_with_repo(c: Commit<'a>, repo: &'a Repository) -> CommitWrapper<'a> {
    CommitWrapper {
      t: c.time(),
      c: c,
      p: repo.workdir().expect("Get workdir failed"),
      r: repo,
    }
  }
}

impl<'a> Eq for CommitWrapper<'a> {}
impl<'a> PartialEq for CommitWrapper<'a> {
  fn eq(&self, other: &CommitWrapper) -> bool {
    return self.t.eq(&other.t);
  }
}

impl<'a> Ord for CommitWrapper<'a> {
  fn cmp(&self, other: &CommitWrapper) -> Ordering {
    return self.t.cmp(&other.t);
  }
}

impl<'a> PartialOrd for CommitWrapper<'a> {
  fn partial_cmp(&self, other: &CommitWrapper) -> Option<Ordering> {
    return self.t.partial_cmp(&other.t);
  }
}

struct CommitsWalker<'a> {
  heads: BinaryHeap<CommitWrapper<'a>>,
}

impl<'a> CommitsWalker<'a> {
  pub fn new(heads: Vec<CommitWrapper<'a>>) -> CommitsWalker<'a> {
    let heap = BinaryHeap::from_iter(heads.into_iter());
    return Self { heads: heap };
  }
}

impl<'a> std::iter::Iterator for CommitsWalker<'a> {
  type Item = CommitWrapper<'a>;
  fn next(&mut self) -> Option<Self::Item> {
    let latest = match self.heads.pop() {
      Some(c) => c,
      None => return None,
    };
    loop {
      if let Some(c) = self.heads.peek() {
        if *c == latest {
          self.heads.pop();
          continue;
        }
      }
      break;
    }
    latest
      .c
      .parents()
      .for_each(|c| self.heads.push(CommitWrapper::new(c, latest.p, latest.r)));
    return Some(latest);
  }
}

fn collect_submodules(repo: Repository) -> Vec<Repository> {
  let subs = repo.submodules().expect("Get submodule failed");
  let mut repos = Vec::new();
  subs
    .iter()
    .map(|s| s.open().expect("Open submodules failed"))
    .for_each(|r| repos.extend(collect_submodules(r)));
  drop(subs);
  repos.push(repo);
  return repos;
}

fn collect_submodule_heads_with_rev<'a>(
  rev: &Commit,
  repo: &Repository,
  heads: &'a mut Vec<Oid>,
  sub_mods: &'a mut Vec<Repository>,
) {
  rev
    .tree()
    .expect("Get tree failed")
    .walk(TreeWalkMode::PreOrder, |_, e| -> TreeWalkResult {
      if e.kind() != Some(ObjectType::Commit) {
        return TreeWalkResult::Ok;
      }
      let sub = repo
        .find_submodule(e.name().expect("Get object name failed"))
        .expect("Find submodule failed")
        .open()
        .expect("Open submodule failed");
      let sub_head = sub
        .find_commit(e.id())
        .expect("Can't find commit in the submodule");
      heads.push(sub_head.id());
      collect_submodule_heads_with_rev(&sub_head, &sub, heads, sub_mods);
      drop(sub_head);
      sub_mods.push(sub);
      return TreeWalkResult::Ok;
    })
    .expect("Walk tree failed");
}

fn collect_heads<'a>(
  repos: &'a Vec<Repository>,
  args: &LogArgs,
  heads: &mut Vec<CommitWrapper<'a>>,
) {
  repos.iter().for_each(|r| {
    let repo_path = r.workdir().unwrap();
    if args.all {
      r.branches(None)
        .expect("Get branches failed")
        .for_each(|b| {
          let commit = b
            .expect("Get branch failed")
            .0
            .get()
            .peel_to_commit()
            .expect("get commit failed");
          heads.push(CommitWrapper::new(commit, repo_path, r));
        })
    } else {
      let commit = r
        .head()
        .expect("Get head failed")
        .peel_to_commit()
        .expect("get commit failed");
      heads.push(CommitWrapper::new(commit, repo_path, r));
    }
  });
}

fn format_duration(dur: chrono::Duration) -> String {
  if dur.num_days() > 30 {
    format!("{} months ago", dur.num_days() / 30)
  } else if dur.num_days() > 0 {
    format!("{} days ago", dur.num_days())
  } else if dur.num_hours() > 0 {
    format!("{} hours ago", dur.num_hours())
  } else if dur.num_minutes() > 0 {
    format!("{} mins ago", dur.num_minutes())
  } else if dur.num_seconds() > 0 {
    format!("{} secs ago", dur.num_seconds())
  } else {
    String::from("just now")
  }
}

fn print_commit(commit: CommitWrapper, base_path: &Path, now: DateTime<Local>, args: &LogArgs) {
  let committer_time = Local.timestamp(commit.t.seconds(), 0);
  let duration = format_duration(now - committer_time);
  let path = commit
    .p
    .canonicalize()
    .expect("Get canonicalize path failed");
  if args.print_full {
    let author_time = Local.timestamp(commit.c.author().when().seconds(), 0);
    if path == base_path {
      println!(
        "{} - {}",
        commit.c.id().to_string().yellow(),
        commit.p.display().to_string().bright_blue()
      );
    } else {
      println!(
        "{} - {}",
        commit.c.id().to_string().yellow(),
        path
          .strip_prefix(base_path)
          .unwrap_or(&path)
          .display()
          .to_string()
          .bright_blue()
      );
    }
    println!("Author:     {}", commit.c.author());
    println!("AuthorDate: {}", author_time.format("%a %b %d %T %Y %z"));
    println!("Commit:     {}", commit.c.committer());
    println!("CommitDate: {}", committer_time.format("%a %b %d %T %Y %z"));
    println!(
      "\n    {}",
      commit.c.message().unwrap_or("").replace("\n", "\n    ")
    );
  } else {
    if path == base_path {
      println!(
        "{} - {:50} ({}) <{}> ({})",
        &commit.c.id().to_string()[..7].red(),
        commit.c.summary().unwrap_or_default(),
        duration.green(),
        commit
          .c
          .author()
          .name()
          .unwrap_or("!!NO NAME!!")
          .to_string()
          .bright_blue(),
        commit.p.display(),
      )
    } else {
      println!(
        "{} - {:50} ({}) <{}> (./{})",
        &commit.c.id().to_string()[..7].red(),
        commit.c.summary().unwrap_or_default(),
        duration.green(),
        commit
          .c
          .author()
          .name()
          .unwrap_or("!!NO NAME!!")
          .to_string()
          .bright_blue(),
        path.strip_prefix(base_path).unwrap_or(&path).display(),
      );
    }
  }
  if args.print_list || args.print_patch {
    let diff = commit
      .r
      .diff_tree_to_tree(
        commit
          .c
          .parent(0)
          .ok()
          .map(|c| c.tree().ok())
          .flatten()
          .as_ref(),
        commit.c.tree().ok().as_ref(),
        Some(&mut DiffOptions::default()),
      )
      .expect("Get diff from parent failed");
    diff.deltas().for_each(|d| {
      if args.print_list {
        let label = match d.status() {
          Delta::Added => "A".green(),
          Delta::Conflicted => "C".red(),
          Delta::Copied => "C".green(),
          Delta::Deleted => "D".red(),
          Delta::Ignored => "I".red(),
          Delta::Modified => "M".red(),
          Delta::Renamed => "R".green(),
          Delta::Typechange => "T".green(),
          Delta::Unmodified => "U".green(),
          Delta::Unreadable => "U".red(),
          Delta::Untracked => "U".default(),
        };
        if d.status() == Delta::Renamed {
          let old_name = d.old_file().path().expect("Get old file name failed");
          let new_name = d.new_file().path().expect("Get old file name failed");
          println!(
            "  {} {} -> {}",
            label,
            old_name.display(),
            new_name.display()
          );
        } else {
          let new_name = d.new_file().path().expect("Get old file name failed");
          println!("  {} {}", label, new_name.display());
        }
      }

      if args.print_patch {
        let status = match d.status() {
          Delta::Added => Status::INDEX_NEW,
          Delta::Conflicted => Status::CONFLICTED,
          Delta::Copied => Status::INDEX_NEW,
          Delta::Deleted => Status::INDEX_DELETED,
          Delta::Ignored => Status::IGNORED,
          Delta::Modified => Status::INDEX_MODIFIED,
          Delta::Renamed => Status::INDEX_RENAMED,
          Delta::Typechange => Status::INDEX_TYPECHANGE,
          Delta::Unmodified => Status::CURRENT,
          Delta::Unreadable => Status::IGNORED,
          Delta::Untracked => Status::IGNORED,
        };
        super::status::print_patch(commit.r, &d, status);
      }
    })
  }
}

fn test_pathspec(commit: &CommitWrapper, pathspec: &Pathspec, work_dir: &Path) -> bool {
  return commit.c.parents().any(|p| {
    commit
      .r
      .diff_tree_to_tree(
        p.tree().ok().as_ref(),
        commit.c.tree().ok().as_ref(),
        Some(&mut DiffOptions::default()),
      )
      .unwrap()
      .deltas()
      .any(|d| {
        let new_path = commit.p.join(d.new_file().path().unwrap());
        if d.status() == Delta::Renamed {
          let old_path = commit.p.join(d.old_file().path().unwrap());
          pathspec.matches_path(
            new_path.strip_prefix(work_dir).unwrap(),
            PathspecFlags::DEFAULT,
          ) || pathspec.matches_path(
            old_path.strip_prefix(work_dir).unwrap(),
            PathspecFlags::DEFAULT,
          )
        } else {
          pathspec.matches_path(
            new_path.strip_prefix(work_dir).unwrap(),
            PathspecFlags::DEFAULT,
          )
        }
      })
  });
}

pub fn show_log(repo: Repository, repo_dir: &Path, args: LogArgs) {
  let org_repo_path = repo.workdir().unwrap().to_owned();
  let mut repos: Vec<Repository>;
  let mut heads: Vec<CommitWrapper>;
  if let Some(rev) = &args.head {
    repos = Vec::new();
    heads = Vec::new();
    let obj = repo
      .revparse_single(rev)
      .unwrap_or_else(|_| err_exit!("Can't find the revision in the root repo."));
    let rev = obj
      .as_commit()
      .unwrap_or_else(|| err_exit!("The revision is not a commit"));
    let mut oids = Vec::new();
    collect_submodule_heads_with_rev(rev, &repo, &mut oids, &mut repos);
    oids.push(rev.id());
    drop(rev);
    drop(obj);
    repos.push(repo);
    for (i, id) in oids.iter().enumerate() {
      heads.push(CommitWrapper::new_with_repo(
        repos[i]
          .find_commit(*id)
          .expect("Can't find the commit in submodule"),
        &repos[i],
      ));
    }
  } else {
    repos = collect_submodules(repo);
    heads = Vec::new();
    collect_heads(&repos, &args, &mut heads);
  }

  let walker = CommitsWalker::new(heads);
  let now: DateTime<Local> = Local::now();
  let mut count = args.num;

  walker
    .filter(|commit| {
      if let Some(ref grep) = args.grep {
        if !grep.is_match(commit.c.message().unwrap_or("")) {
          return false;
        }
      }
      if let Some(ref author) = args.author {
        if !author.is_match(&commit.c.author().to_string()) {
          return false;
        }
      }
      if let Some(ref pathspec) = args.pathspec {
        if !test_pathspec(&commit, &pathspec, &org_repo_path) {
          return false;
        }
      }
      return true;
    })
    .skip(args.start.unwrap_or(0))
    .take_while(|_| {
      if let Some(n) = count {
        if n == 0 {
          count = None;
          return false;
        } else {
          count = Some(n - 1);
          return true;
        }
      } else {
        return true;
      }
    })
    .for_each(|c| {
      print_commit(c, repo_dir, now, &args);
    });
}
