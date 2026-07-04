//! RAII guard for entering/leaving raw mode and the alternate screen.

use std::io;

use crossterm::cursor::Show;
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode,
    enable_raw_mode,
};

/// Marker type representing "the terminal is currently in raw
/// mode/alternate screen for this app". Restores the terminal to its
/// normal state when dropped, so a panic or early return can't leave the
/// user's shell in raw mode or on the alternate screen.
pub struct Terminal;

impl Terminal {
    /// Enables raw mode and switches to the alternate screen buffer.
    /// Restoration happens automatically via `Drop` once the returned
    /// guard goes out of scope.
    pub fn enter() -> io::Result<Self> {
        enable_raw_mode()?;

        execute!(io::stdout(), EnterAlternateScreen)?;

        Ok(Self)
    }

    /// Current terminal size as (columns, rows).
    pub fn size() -> io::Result<(u16, u16)> {
        crossterm::terminal::size()
    }

    /// Leaves the alternate screen, shows the cursor, and disables raw
    /// mode. Safe to call even if entering failed partway, and safe to
    /// call from a panic hook (errors are swallowed rather than
    /// propagated, since there's nothing useful to do with them at that
    /// point).
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
