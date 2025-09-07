mod app;
mod cpu;
mod gpu;
mod memory;
mod process;
mod tui;
mod ui;

use app::App;
use std::error::Error;
use std::time::{Duration, Instant};
use std::thread;

fn main() -> Result<(), Box<dyn Error>> {
    // Initialize terminal
    let mut terminal = tui::TuiGuard::new()?;
    
    // Create app
    let mut app = App::new();
    
    // Frame rate limiting - target 40 FPS for responsive navigation (25ms per frame)
    const TARGET_FRAME_TIME: Duration = Duration::from_millis(25);
    
    // Main event loop
    while app.is_running() {
        let frame_start = Instant::now();
        
        // Update app state and check if render needed
        let data_changed = app.tick();
        
        // Handle events and check if render needed
        let event_occurred = app.handle_event()?;
        
        // Only render if something changed
        if data_changed || event_occurred {
            terminal.draw(|f| ui::render(f, &mut app))?;
        }
        
        // Always sleep for the remaining frame time to avoid busy loop
        let elapsed = frame_start.elapsed();
        if elapsed < TARGET_FRAME_TIME {
            thread::sleep(TARGET_FRAME_TIME - elapsed);
        }
    }
    
    Ok(())
}