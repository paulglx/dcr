mod app;
mod dicom;
mod textdiff;
mod ui;
mod validation;

use app::App;
use clap::{value_parser, Arg, Command};
use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::prelude::*;
use ratatui_image::picker::Picker;
use std::{io, path::Path, path::PathBuf};

struct Args {
    /// Baseline and modified paths when `--diff` is given.
    diff: Option<Vec<PathBuf>>,
    /// Single file to view; explorer mode when absent.
    file: Option<PathBuf>,
}

impl Args {
    fn parse() -> Self {
        let matches = Command::new(env!("CARGO_PKG_NAME"))
            .version(env!("CARGO_PKG_VERSION"))
            .about(env!("CARGO_PKG_DESCRIPTION"))
            .arg(
                Arg::new("diff")
                    .short('d')
                    .long("diff")
                    .value_names(["BASELINE", "MODIFIED"])
                    .num_args(2)
                    .value_parser(value_parser!(PathBuf))
                    .help("Compare two DICOM files (baseline vs modified)"),
            )
            .arg(
                Arg::new("file")
                    .value_name("FILE")
                    .value_parser(value_parser!(PathBuf))
                    .help("Path to the DICOM file to view (opens file explorer if omitted)"),
            )
            .get_matches();

        Args {
            diff: matches
                .get_many::<PathBuf>("diff")
                .map(|paths| paths.cloned().collect()),
            file: matches.get_one::<PathBuf>("file").cloned(),
        }
    }
}

fn validate_path(path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    if !path.exists() {
        return Err(format!("file not found: {}", path.display()).into());
    }
    if !path.is_file() {
        return Err(format!("not a file: {}", path.display()).into());
    }
    let mut file = std::fs::File::open(path)?;
    let mut buf = [0u8; 132];
    use std::io::Read;
    if file.read(&mut buf)? < 132 || &buf[128..132] != b"DICM" {
        return Err(format!("not a valid DICOM file: {}", path.display()).into());
    }
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    let picker = Picker::from_query_stdio().ok();

    let mut app = if let Some(diff_files) = &args.diff {
        if diff_files.len() != 2 {
            return Err("--diff requires exactly two file arguments".into());
        }
        let baseline_path = &diff_files[0];
        let modified_path = &diff_files[1];
        validate_path(baseline_path)?;
        validate_path(modified_path)?;

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
            validation::get_sop_class(modified_path).unwrap_or(validation::SopClass::Unknown);
        let validation_result = validation::validate_type1_fields(modified_path)
            .unwrap_or(validation::ValidationResult::NotApplicable);

        App::new_with_diff(
            tags,
            baseline_name,
            Some(modified_name),
            validation_result,
            sop_class,
            true,
            Some(modified_path.clone()),
            Some(baseline_path.clone()),
            picker,
        )
    } else if let Some(file) = args.file {
        validate_path(&file)?;
        let obj = ::dicom_object::open_file(&file)?;
        let tags = dicom::extract_tags(&obj);
        let sop_class = validation::get_sop_class_from_obj(&obj);
        let validation_result = validation::validate_type1_fields_from_obj(&obj);

        let file_name = file
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| file.to_string_lossy().to_string());

        let mut app = App::new_with_diff(
            tags,
            file_name,
            None,
            validation_result,
            sop_class,
            false,
            Some(file),
            None,
            picker,
        );
        app.attach_dicomdir(&obj);
        app
    } else {
        App::new_explorer(picker)
    };

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let result = run_app(&mut terminal, &mut app);

    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
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
