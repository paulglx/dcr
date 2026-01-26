mod app;
mod dicom;
mod ui;
mod validation;

use app::App;
use clap::Parser;
use crossterm::{
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::prelude::*;
use std::{io, path::PathBuf};

/// DICOM TUI Viewer - View DICOM file tags in a terminal interface
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Path to the DICOM file to view
    #[arg(value_name = "FILE")]
    file: PathBuf,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    // Load DICOM file
    let tags = dicom::load_dicom_file(&args.file)?;
    
    // Validate Type 1 fields
    let validation_result = validation::validate_type1_fields(&args.file)
        .unwrap_or(validation::ValidationResult::NotApplicable);

    // Extract file name for display
    let file_name = args
        .file
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| args.file.to_string_lossy().to_string());

    // Create app state
    let mut app = App::new(tags, file_name, validation_result);

    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Run the main loop
    let result = run_app(&mut terminal, &mut app);

    // Restore terminal
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    // Handle any errors from the app loop
    if let Err(err) = result {
        eprintln!("Error: {}", err);
        return Err(err.into());
    }

    Ok(())
}

/// Main application loop
fn run_app<B: Backend>(terminal: &mut Terminal<B>, app: &mut App) -> io::Result<()> {
    loop {
        terminal.draw(|frame| ui::render(frame, app))?;

        app.handle_events()?;

        if app.should_quit {
            return Ok(());
        }
    }
}
