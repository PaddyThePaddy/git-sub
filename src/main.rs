mod color;
mod diff_filter;
mod log;
mod ls_files;
mod status;
use clap::*;
use color::*;
use diff_filter::DiffFilter;
use git2::*;
use log::*;
use ls_files::*;
use status::*;
use std::path::*;

#[macro_export]
macro_rules! err_exit {
  ( $( $x:expr ),* ) => {{
    eprintln!($($x,)*);
    std::process::exit(1);
  }};
}
enum Args {
  None,
  Status(StatusArgs),
  Log(LogArgs),
  LsFile(LsArgs),
}

// build application's cli argument
fn build_arg() -> (Repository, PathBuf, Args) {
  let matches = Command::new("git-sub")
    .about("Collect information of submodules in a convenience way")
    .author("paddythepaddy@duck.com")
    .version(git_version::git_version!())
    .arg(
      Arg::new("path")
        .long("cwd")
        .short('C')
        .help("The working path or the repository")
        .default_value("."),
    )
    .arg(
      Arg::new("force-color")
        .long("force-color")
        .short('c')
        .action(ArgAction::SetTrue)
        .help("Force print color even using pipeline"),
    )
    .subcommand(StatusArgs::build_arg())
    .subcommand(LogArgs::build_arg())
    .subcommand(LsArgs::build_arg())
    .get_matches();
  let work_dir_path = Path::new(matches.get_one::<String>("path").unwrap_or_else(|| {
    err_exit!("Extract argument failed");
  }))
  .canonicalize()
  .unwrap_or_else(|e| {
    err_exit!("Get canonicalize path failed: {}", e);
  });

  let repo = Repository::open(&work_dir_path).unwrap_or_else(|e| {
    err_exit!("Open repo failed, not a git repo? {}", e);
  });
  let args: Args;
  if let Some((sub_name, sub_matches)) = matches.subcommand() {
    match sub_name {
      "status" => args = Args::Status(StatusArgs::from(sub_matches)),
      "log" => args = Args::Log(LogArgs::from(sub_matches)),
      "ls-files" => args = Args::LsFile(LsArgs::from(sub_matches)),
      _ => {
        err_exit!("Unknown subcommand")
      }
    }
  } else {
    args = Args::None;
  }
  if matches.get_flag("force-color") {
    std::env::set_var("CLICOLOR_FORCE", "1");
  }
  check_tty();

  return (repo, work_dir_path, args);
}

fn main() {
  // preparing
  let (repo, work_dir_path, args) = build_arg();

  // the work
  match args {
    Args::Status(mut a) => {
      show_repo_status(
        &repo,
        &work_dir_path,
        repo
          .head()
          .expect("Extract head failed")
          .resolve()
          .expect("Resolve reference failed")
          .target()
          .expect("Get oid failed"),
        &mut a,
      );
    }
    Args::Log(a) => {
      show_log(repo, &work_dir_path, a);
    }
    Args::LsFile(a) => {
      list_files(repo, a);
    }
    Args::None => {
      err_exit!("No subcommand is given. Supported subcommand: status, log, ls-files")
    }
  }
}
