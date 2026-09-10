use clap::{value_parser, Arg, ArgAction, Command};
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use dcr::app::App;
use dcr::ui;
use ratatui::prelude::*;
use ratatui_image::picker::Picker;
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::{Duration, Instant};
use std::{io, path::Path, path::PathBuf, thread};

/// How long a frame waits for a decode already under way, so the image lands together
/// with the selection change instead of one frame later.
const PREVIEW_GRACE: Duration = Duration::from_millis(8);

/// Everything that wakes the main loop.
enum Msg {
    Input(Event),
    /// A preview decode finished.
    PreviewReady,
}

struct Args {
    /// Baseline and modified paths when `--diff` is given.
    diff: Option<Vec<PathBuf>>,
    /// Start the diff with unchanged rows hidden.
    changes_only: bool,
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
                Arg::new("changes-only")
                    .short('c')
                    .long("changes-only")
                    .action(ArgAction::SetTrue)
                    .requires("diff")
                    .help("In diff mode, only show tags whose value differs (toggle with 'c')"),
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
            changes_only: matches.get_flag("changes-only"),
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

    let mut app = if let Some([baseline, modified]) = args.diff.as_deref() {
        validate_path(baseline)?;
        validate_path(modified)?;
        let mut app = App::new_direct(picker);
        app.enter_diff(baseline, modified)?;
        app.set_changes_only(args.changes_only);
        app
    } else if let Some(file) = &args.file {
        validate_path(file)?;
        let mut app = App::new_direct(picker);
        app.try_load_dicom_file(file)?;
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

fn run_app<B: Backend<Error = io::Error>>(
    terminal: &mut Terminal<B>,
    app: &mut App,
) -> io::Result<()> {
    let (tx, rx): (Sender<Msg>, Receiver<Msg>) = mpsc::channel();
    spawn_input_thread(tx.clone());
    app.preview.set_waker(move || {
        let _ = tx.send(Msg::PreviewReady);
    });

    loop {
        app.request_pending_preview();
        app.preview.poll_results();
        terminal.draw(|frame| ui::render(frame, app))?;

        // Block until something happens, then coalesce whatever else is queued (a scroll
        // burst, several decodes) into a single redraw.
        let msg = rx
            .recv()
            .map_err(|_| io::Error::other("input thread stopped"))?;
        handle_msg(app, msg);
        while let Ok(msg) = rx.try_recv() {
            handle_msg(app, msg);
        }

        // Start the decode now so the grace wait below can catch it.
        app.request_pending_preview();
        let deadline = Instant::now() + PREVIEW_GRACE;
        while app.preview.is_loading() && !app.should_quit {
            let Some(left) = deadline.checked_duration_since(Instant::now()) else {
                break;
            };
            match rx.recv_timeout(left) {
                Ok(msg) => handle_msg(app, msg),
                Err(_) => break,
            }
        }

        if app.should_quit {
            return Ok(());
        }
    }
}

fn handle_msg(app: &mut App, msg: Msg) {
    match msg {
        Msg::Input(event) => app.handle_event(event),
        Msg::PreviewReady => app.preview.poll_results(),
    }
}

/// Reads terminal events on their own thread so the main loop can also be woken by the
/// preview decoder. Ends with the process (or when the receiver is gone).
fn spawn_input_thread(tx: Sender<Msg>) {
    thread::Builder::new()
        .name("input".into())
        .spawn(move || {
            while let Ok(event) = event::read() {
                if tx.send(Msg::Input(event)).is_err() {
                    break;
                }
            }
        })
        .expect("failed to spawn input thread");
}
