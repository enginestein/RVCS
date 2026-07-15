use std::io::IsTerminal;

fn enabled() -> bool {
    std::io::stdout().is_terminal() && std::env::var("NO_COLOR").is_err()
}

macro_rules! ansi {
    ($code:expr) => { concat!("\x1b[", $code, "m") };
}

const RESET: &str = ansi!("0");
const GREEN: &str = ansi!("32");
const RED: &str = ansi!("31");
const YELLOW: &str = ansi!("33");
const CYAN: &str = ansi!("36");
const BOLD: &str = ansi!("1");
const WHITE_ON_RED: &str = ansi!("41;37");

pub fn green(s: &str) -> String {
    format!("{}{}{}", GREEN, s, RESET)
}

pub fn red(s: &str) -> String {
    format!("{}{}{}", RED, s, RESET)
}

pub fn yellow(s: &str) -> String {
    format!("{}{}{}", YELLOW, s, RESET)
}

pub fn cyan(s: &str) -> String {
    format!("{}{}{}", CYAN, s, RESET)
}

pub fn bold(s: &str) -> String {
    format!("{}{}{}", BOLD, s, RESET)
}

pub fn white_on_red(s: &str) -> String {
    format!("{}{}{}", WHITE_ON_RED, s, RESET)
}

pub struct Color {
    enabled: bool,
}

impl Color {
    pub fn new() -> Self {
        Self { enabled: enabled() }
    }

    pub fn green(&self, s: &str) -> String {
        if self.enabled { format!("{}{}{}", GREEN, s, RESET) } else { s.to_string() }
    }

    pub fn red(&self, s: &str) -> String {
        if self.enabled { format!("{}{}{}", RED, s, RESET) } else { s.to_string() }
    }

    pub fn yellow(&self, s: &str) -> String {
        if self.enabled { format!("{}{}{}", YELLOW, s, RESET) } else { s.to_string() }
    }

    pub fn cyan(&self, s: &str) -> String {
        if self.enabled { format!("{}{}{}", CYAN, s, RESET) } else { s.to_string() }
    }

    pub fn bold(&self, s: &str) -> String {
        if self.enabled { format!("{}{}{}", BOLD, s, RESET) } else { s.to_string() }
    }

    pub fn white_on_red(&self, s: &str) -> String {
        if self.enabled { format!("{}{}{}", WHITE_ON_RED, s, RESET) } else { s.to_string() }
    }

    pub fn header(&self, s: &str) -> String {
        if self.enabled { format!("{}{}{}", BOLD, s, RESET) } else { s.to_string() }
    }
}

impl Default for Color {
    fn default() -> Self {
        Self::new()
    }
}
