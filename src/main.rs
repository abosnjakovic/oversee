mod app;
mod cpu;
mod process;
mod tui;
mod ui;

use app::App;
use std::error::Error;

fn main() -> Result<(), Box<dyn Error>> {
    // Initialize terminal
    let mut terminal = tui::TuiGuard::new()?;
    
    // Create app
    let mut app = App::new();
    
    // Main event loop
    while app.is_running() {
        // Update app state
        app.tick();
        
        // Handle events
        app.handle_event()?;
        
        // Render UI
        terminal.draw(|f| ui::render(f, &app))?;
    }
    
    Ok(())
}