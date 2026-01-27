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

    let tags = dicom::load_dicom_file(&args.file)?;
    
    let sop_class = validation::get_sop_class(&args.file)
        .unwrap_or(validation::SopClass::Unknown);
    let validation_result = validation::validate_type1_fields(&args.file)
        .unwrap_or(validation::ValidationResult::NotApplicable);

    let file_name = args
        .file
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| args.file.to_string_lossy().to_string());

    let mut app = App::new(tags, file_name, validation_result, sop_class);

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let result = run_app(&mut terminal, &mut app);

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    if let Err(err) = result {
        eprintln!("Error: {}", err);
        return Err(err.into());
    }

    Ok(())
}

fn run_app<B: Backend>(terminal: &mut Terminal<B>, app: &mut App) -> io::Result<()> {
    loop {
        terminal.draw(|frame| ui::render(frame, app))?;

        app.handle_events()?;

        if app.should_quit {
            return Ok(());
        }
    }
}
