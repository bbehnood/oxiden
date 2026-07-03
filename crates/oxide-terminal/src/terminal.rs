use std::io;

use crossterm::cursor::{Hide, Show};
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode,
    enable_raw_mode,
};

pub struct Terminal;

impl Terminal {
    pub fn enter() -> io::Result<Self> {
        enable_raw_mode()?;

        execute!(io::stdout(), EnterAlternateScreen, Hide)?;

        Ok(Self)
    }

    pub fn size() -> io::Result<(u16, u16)> {
        crossterm::terminal::size()
    }

    pub fn restore() {
        let _ = execute!(io::stdout(), LeaveAlternateScreen, Show);
        let _ = disable_raw_mode();
    }
}

impl Drop for Terminal {
    fn drop(&mut self) {
        Self::restore();
    }
}
