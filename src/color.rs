use termcolor::{Color, ColorChoice, ColorSpec, StandardStream, WriteColor};
// The replacement of https://github.com/mackwic/colored to support color in cmd
static mut DO_COLOR: bool = false;
static mut CSTDOUT: Option<StandardStream> = None;

pub fn check_tty() {
  if let Ok(s) = std::env::var("CLICOLOR_FORCE") {
    if !(s.len() == 0 || s == "0") {
      unsafe {
        DO_COLOR = true;
      }
      return;
    }
  }
  if atty::is(atty::Stream::Stdout) {
    unsafe {
      DO_COLOR = true;
    }
  }
}
fn do_color() -> bool {
  unsafe {
    return DO_COLOR;
  }
}
pub trait StrColor {
  fn red(&self) -> ColoredString;
  fn green(&self) -> ColoredString;
  fn purple(&self) -> ColoredString;
  fn cyan(&self) -> ColoredString;
  fn bright_blue(&self) -> ColoredString;
  fn yellow(&self) -> ColoredString;
  fn default(&self) -> ColoredString;
}

impl StrColor for str {
  fn red(&self) -> ColoredString {
    let mut color = ColorSpec::new();
    color.set_fg(Some(Color::Red));
    ColoredString {
      text: String::from(self),
      color: color,
    }
  }
  fn green(&self) -> ColoredString {
    let mut color = ColorSpec::new();
    color.set_fg(Some(Color::Green));
    ColoredString {
      text: String::from(self),
      color: color,
    }
  }
  fn purple(&self) -> ColoredString {
    let mut color = ColorSpec::new();
    color.set_fg(Some(Color::Magenta));
    ColoredString {
      text: String::from(self),
      color: color,
    }
  }
  fn cyan(&self) -> ColoredString {
    let mut color = ColorSpec::new();
    color.set_fg(Some(Color::Cyan));
    ColoredString {
      text: String::from(self),
      color: color,
    }
  }
  fn bright_blue(&self) -> ColoredString {
    let mut color = ColorSpec::new();
    color.set_fg(Some(Color::Blue)).set_intense(true);
    ColoredString {
      text: String::from(self),
      color: color,
    }
  }
  fn yellow(&self) -> ColoredString {
    let mut color = ColorSpec::new();
    color.set_fg(Some(Color::Yellow)).set_intense(true);
    ColoredString {
      text: String::from(self),
      color: color,
    }
  }
  fn default(&self) -> ColoredString {
    let color = ColorSpec::new();
    ColoredString {
      text: String::from(self),
      color: color,
    }
  }
}

pub struct ColoredString {
  color: ColorSpec,
  text: String,
}

impl std::fmt::Display for ColoredString {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    use std::io::Write;
    if do_color() {
      unsafe {
        if let None = CSTDOUT {
          CSTDOUT = Some(termcolor::StandardStream::stdout(ColorChoice::Auto));
        }
        if let Some(ref mut stdout) = CSTDOUT {
          stdout.set_color(&self.color).unwrap();
          write!(stdout, "{}", self.text).unwrap();
          stdout.reset().unwrap();
        }
      }

      Ok(())
    } else {
      write!(f, "{}", self.text)
    }
  }
}
