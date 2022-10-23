use super::*;
use clap::*;
use git2::{Pathspec, Repository};
pub struct LsArgs {
  staged: bool,
  pathspec: Option<Pathspec>,
  rev: Option<String>,
}

impl LsArgs {
  pub fn build_arg() -> clap::Command {
    return clap::Command::new("ls-files")
      .about("List files across all submodules")
      .arg(
        Arg::new("staged")
          .long("staged")
          .short('s')
          .action(ArgAction::SetTrue)
          .help("List files in the index"),
      )
      .arg(
        clap::Arg::new("pathspec")
          .action(ArgAction::Append)
          .help("Filter files by the pathspec"),
      )
      .arg(
        clap::Arg::new("revision")
          .long("rev")
          .short('r')
          .help("Search commits starting from the specific reference of the **root** repo"),
      )
      .group(ArgGroup::new("mode").arg("staged").arg("revision"));
  }
}

impl From<&clap::ArgMatches> for LsArgs {
  fn from(matches: &clap::ArgMatches) -> LsArgs {
    return LsArgs {
      staged: matches.get_flag("staged"),
      pathspec: matches
        .get_many::<String>("pathspec")
        .map(|s| Pathspec::new(s).unwrap_or_else(|_| err_exit!("Crate pathspec failed"))),
      rev: matches.get_one::<String>("revision").map(|s| s.into()),
    };
  }
}

fn list_index_file(repo: Repository, args: &LsArgs) {
  // file mode reference: https://github.com/git/git/blob/a08a83db2bf27f015bec9a435f6d73e223c21c5e/Documentation/technical/index-format.txt#L63
  const FILE_MODE_GIT_LINK: u32 = 0b1110;
  let index = repo.index().expect("Get index failed");
  index.iter().for_each(|e| {
    let path_str = String::from_utf8_lossy(&e.path);
    if e.mode >> 12 == FILE_MODE_GIT_LINK {
      let sub = repo
        .find_submodule(&path_str)
        .expect("Can't find submodule");
      let sub_repo = sub.open().expect("Can't open submodule repo");
      list_commit_file(sub_repo, &e.id.to_string(), None, args);
    } else {
      if args.staged {
        print!("{} ", e.id.to_string());
      }
      println!("{}", path_str);
    }
  });
}

fn list_commit_file(repo: Repository, commit: &str, base_path: Option<&str>, args: &LsArgs) {
  let obj = repo
    .revparse_single(commit)
    .unwrap_or_else(|_| err_exit!("Find revision failed"));
  let commit = obj
    .peel_to_commit()
    .unwrap_or_else(|_| err_exit!("The revision can't peel to a commit"));
  let tree = commit.tree().expect("Can't find the tree for the commit");
  list_tree(&repo, &tree, base_path, args, None);
}

fn list_tree(
  repo: &Repository,
  tree: &Tree,
  rel_path_by_root: Option<&str>,
  args: &LsArgs,
  rel_path_by_repo: Option<&str>,
) {
  tree.iter().for_each(|e| {
    // the relative path by the root repo
    let sub_name = if let Some(p) = rel_path_by_root {
      format!("{}/{}", p, e.name().unwrap_or(""))
    } else {
      String::from(e.name().unwrap_or(""))
    };
    // the relative path by the current repo (might be in the submodule)
    let sub_repo_base = if let Some(s) = rel_path_by_repo {
      format!("{}/{}", s, e.name().unwrap_or(""))
    } else {
      String::from(e.name().unwrap_or(""))
    };
    match e.kind().expect("Got an unknown entry") {
      ObjectType::Commit => {
        let sub = repo
          .find_submodule(&sub_repo_base)
          .expect("Find submodule failed");
        let sub_repo = sub.open().expect("Open submodule failed");
        list_commit_file(sub_repo, &e.id().to_string(), Some(&sub_name), args);
      }
      ObjectType::Tree => {
        let obj = e.to_object(repo).expect("Find tree object failed");
        let sub_tree = obj.as_tree().expect("Convert object to tree failed");

        list_tree(repo, sub_tree, Some(&sub_name), args, Some(&sub_repo_base));
      }
      _ => {
        if let Some(pathspec) = &args.pathspec {
          let path = Path::new(&sub_name);
          if !pathspec.matches_path(path, PathspecFlags::DEFAULT) {
            return;
          }
        }
        print!("{} ", e.id().to_string());
        println!("{}", sub_name);
      }
    }
  });
}

pub fn list_files(repo: Repository, args: LsArgs) {
  if args.staged {
    list_index_file(repo, &args);
  } else {
    let rev_str: &str = if let Some(s) = args.rev.as_ref() {
      s
    } else {
      "HEAD"
    };
    list_commit_file(repo, &rev_str, None, &args);
  }
}
