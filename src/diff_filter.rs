#[derive(Debug)]
pub struct DiffFilter {
  add: bool,
  deleted: bool,
  modified: bool,
  rename: bool,
  type_changed: bool,
  unknown: bool,
}

impl DiffFilter {
  pub fn default() -> DiffFilter {
    DiffFilter {
      add: true,
      deleted: true,
      modified: true,
      rename: true,
      type_changed: true,
      unknown: true,
    }
  }
  pub fn from(s: &str) -> DiffFilter {
    let mut me = DiffFilter {
      add: false,
      deleted: false,
      modified: false,
      rename: false,
      type_changed: false,
      unknown: false,
    };

    for c in s.chars() {
      if c == 'a' || c == 'A' {
        me.add = c.is_uppercase();
      } else if c == 'd' || c == 'D' {
        me.deleted = c.is_uppercase();
      } else if c == 'm' || c == 'M' {
        me.modified = c.is_uppercase();
      } else if c == 'r' || c == 'R' {
        me.rename = c.is_uppercase();
      } else if c == 't' || c == 'T' {
        me.type_changed = c.is_uppercase();
      } else if c == 'u' || c == 'U' {
        me.unknown = c.is_uppercase();
      }
    }

    return me;
  }

  pub fn test(&self, status: git2::Status) -> bool {
    if status.is_index_new() || status.is_wt_new() {
      return self.add;
    } else if status.is_index_modified() || status.is_wt_modified() {
      return self.modified;
    } else if status.is_index_deleted() || status.is_wt_deleted() {
      return self.deleted;
    } else if status.is_index_renamed() || status.is_wt_renamed() {
      return self.rename;
    } else if status.is_index_typechange() || status.is_wt_typechange() {
      return self.type_changed;
    } else {
      return self.unknown;
    }
  }
}
