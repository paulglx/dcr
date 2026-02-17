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
    /// Compare two DICOM files (baseline vs modified)
    #[arg(short = 'd', long, value_names = ["BASELINE", "MODIFIED"], num_args = 2)]
    diff: Option<Vec<PathBuf>>,

    /// Path to the DICOM file to view (used when --diff is not specified)
    #[arg(value_name = "FILE", required_unless_present = "diff")]
    file: Option<PathBuf>,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    let (tags, file_name, modified_name, validation_result, sop_class, diff_mode) =
        if let Some(diff_files) = &args.diff {
            if diff_files.len() != 2 {
                return Err("--diff requires exactly two file arguments".into());
            }
            let baseline_path = &diff_files[0];
            let modified_path = &diff_files[1];

            let tags = dicom::compare_dicom_files(baseline_path, modified_path)?;

            let baseline_name = baseline_path
                .file_name()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_else(|| baseline_path.to_string_lossy().to_string());
            let modified_name = modified_path
                .file_name()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_else(|| modified_path.to_string_lossy().to_string());

            let sop_class =
                validation::get_sop_class(baseline_path).unwrap_or(validation::SopClass::Unknown);
            let validation_result = validation::validate_type1_fields(baseline_path)
                .unwrap_or(validation::ValidationResult::NotApplicable);

            (
                tags,
                baseline_name,
                Some(modified_name),
                validation_result,
                sop_class,
                true,
            )
        } else {
            let file = args
                .file
                .ok_or("Either --diff with two files or a single file argument is required")?;
            let tags = dicom::load_dicom_file(&file)?;

            let sop_class =
                validation::get_sop_class(&file).unwrap_or(validation::SopClass::Unknown);
            let validation_result = validation::validate_type1_fields(&file)
                .unwrap_or(validation::ValidationResult::NotApplicable);

            let file_name = file
                .file_name()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_else(|| file.to_string_lossy().to_string());

            (tags, file_name, None, validation_result, sop_class, false)
        };

    let mut app = App::new_with_diff(
        tags,
        file_name,
        modified_name,
        validation_result,
        sop_class,
        diff_mode,
    );

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
