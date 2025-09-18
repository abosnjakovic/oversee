use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::io::{self, Stdout};

pub type Tui = Terminal<CrosstermBackend<Stdout>>;

pub fn init() -> io::Result<Tui> {
    execute!(io::stdout(), EnterAlternateScreen, EnableMouseCapture)?;
    enable_raw_mode()?;

    let backend = CrosstermBackend::new(io::stdout());
    let terminal = Terminal::new(backend)?;

    Ok(terminal)
}

pub fn restore() -> io::Result<()> {
    execute!(io::stdout(), LeaveAlternateScreen, DisableMouseCapture)?;
    disable_raw_mode()?;
    Ok(())
}

// RAII wrapper for automatic cleanup
pub struct TuiGuard {
    pub terminal: Tui,
}

impl TuiGuard {
    pub fn new() -> io::Result<Self> {
        let terminal = init()?;
        Ok(TuiGuard { terminal })
    }
}

impl Drop for TuiGuard {
    fn drop(&mut self) {
        if let Err(err) = restore() {
            eprintln!("Error restoring terminal: {}", err);
        }
    }
}

impl std::ops::Deref for TuiGuard {
    type Target = Tui;

    fn deref(&self) -> &Self::Target {
        &self.terminal
    }
}

impl std::ops::DerefMut for TuiGuard {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.terminal
    }
}
