//! Image preview: decodes DICOM pixel data off the UI thread, caches the results, and
//! hands them to the terminal as PNG placements (see [`crate::app::kitty`]).

use crate::app::kitty::{self, Placement};
use image::codecs::png::{CompressionType, FilterType as PngFilter, PngEncoder};
use image::{DynamicImage, ImageEncoder};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui_image::picker::{Picker, ProtocolType};
use ratatui_image::FontSize;
use std::collections::{HashSet, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, OnceLock};
use std::time::SystemTime;

/// Decoded images kept around for instant back-and-forth navigation.
const CACHE_CAPACITY: usize = 32;

/// Callback that wakes the event loop when a decode finishes.
pub type Waker = Arc<dyn Fn() + Send + Sync>;

/// What a preview shows: a file, or a baseline/modified pair in diff mode.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Key {
    pub path: PathBuf,
    pub baseline: Option<PathBuf>,
}

/// Size and mtime of the key's files, so a rewritten file is not served from the cache.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Stamp(Vec<Option<(SystemTime, u64)>>);

impl Stamp {
    fn of(key: &Key) -> Self {
        let stat = |p: &Path| {
            std::fs::metadata(p)
                .ok()
                .and_then(|m| Some((m.modified().ok()?, m.len())))
        };
        let mut v = vec![stat(&key.path)];
        if let Some(b) = &key.baseline {
            v.push(stat(b));
        }
        Stamp(v)
    }
}

pub type Decoded = Arc<Result<DynamicImage, String>>;

struct CacheEntry {
    key: Key,
    stamp: Stamp,
    value: Decoded,
}

struct Job {
    key: Key,
    /// True for the file on screen; false for neighbours fetched ahead of time.
    urgent: bool,
}

struct Done {
    key: Key,
    stamp: Stamp,
    value: Decoded,
}

struct Worker {
    jobs: Sender<Job>,
    done: Receiver<Done>,
}

impl Worker {
    fn spawn(wake: Arc<OnceLock<Waker>>) -> Self {
        let (jobs, job_rx) = mpsc::channel::<Job>();
        let (done_tx, done) = mpsc::channel::<Done>();
        std::thread::Builder::new()
            .name("preview-decoder".into())
            .spawn(move || worker_loop(job_rx, done_tx, wake))
            .expect("failed to spawn preview decoder thread");
        Self { jobs, done }
    }
}

/// Serves the most recent urgent request first (earlier urgent ones are stale: the
/// selection has moved on), then prefetches in arrival order.
fn worker_loop(jobs: Receiver<Job>, done: Sender<Done>, wake: Arc<OnceLock<Waker>>) {
    let mut queue: VecDeque<Job> = VecDeque::new();
    loop {
        if queue.is_empty() {
            match jobs.recv() {
                Ok(job) => queue.push_back(job),
                Err(_) => return,
            }
        }
        while let Ok(job) = jobs.try_recv() {
            queue.push_back(job);
        }
        let job = match queue.iter().rposition(|j| j.urgent) {
            Some(pos) => {
                let job = queue.remove(pos).expect("position from rposition");
                queue.retain(|j| !j.urgent);
                job
            }
            None => queue.pop_front().expect("queue is non-empty"),
        };
        let stamp = Stamp::of(&job.key);
        let value = Arc::new(decode_key(&job.key));
        if done
            .send(Done {
                key: job.key,
                stamp,
                value,
            })
            .is_err()
        {
            return;
        }
        if let Some(wake) = wake.get() {
            wake();
        }
    }
}

pub struct Preview {
    pub show: bool,
    pub error: Option<String>,
    /// Set when the selection changed and the app must call `request` before the next draw.
    pub pending: bool,
    picker: Option<Picker>,
    tmux: bool,
    /// Shared with the worker thread, which calls it after every decode.
    waker: Arc<OnceLock<Waker>>,
    worker: Option<Worker>,
    /// What the pane should currently show.
    current: Option<Key>,
    /// Decoded image for `current`, at native resolution.
    source: Option<Arc<DynamicImage>>,
    cache: VecDeque<CacheEntry>,
    in_flight: HashSet<Key>,
    /// Image currently held by the terminal, and the pane area it was built for.
    placement: Option<(Placement, Rect)>,
    /// Id of the image the terminal holds, deleted when the next one is transmitted.
    terminal_image: Option<u32>,
}

impl Preview {
    pub fn new(picker: Option<Picker>) -> Self {
        Self {
            show: true,
            error: None,
            pending: false,
            picker,
            tmux: kitty::in_tmux(),
            waker: Arc::new(OnceLock::new()),
            worker: None,
            current: None,
            source: None,
            cache: VecDeque::new(),
            in_flight: HashSet::new(),
            placement: None,
            terminal_image: None,
        }
    }

    /// Registers the callback that wakes the event loop when a decode finishes.
    pub fn set_waker(&mut self, wake: impl Fn() + Send + Sync + 'static) {
        let _ = self.waker.set(Arc::new(wake));
    }

    fn kitty_available(&self) -> bool {
        self.picker
            .as_ref()
            .is_some_and(|p| p.protocol_type() == ProtocolType::Kitty)
    }

    /// Drops the current image and any error, without touching `show` or the cache.
    pub fn clear(&mut self) {
        self.current = None;
        self.source = None;
        self.placement = None;
        self.error = None;
    }

    /// True once the current file has been decoded.
    pub fn has_image(&self) -> bool {
        self.source.is_some()
    }

    /// True while the current file is being decoded in the background.
    pub fn is_loading(&self) -> bool {
        self.source.is_none()
            && self.error.is_none()
            && self
                .current
                .as_ref()
                .is_some_and(|k| self.in_flight.contains(k))
    }

    pub fn toggle(&mut self, path: Option<&Path>, baseline: Option<&Path>) {
        self.show = !self.show;

        if self.show && self.source.is_none() && self.error.is_none() {
            self.request(path, baseline);
        }
    }

    /// Shows `path` (or, with a `baseline`, the red/green composite of baseline (A) vs
    /// `path` (B)). Served from the cache when possible, otherwise decoded in the
    /// background; the image appears on a later draw.
    pub fn request(&mut self, path: Option<&Path>, baseline: Option<&Path>) {
        self.source = None;
        self.placement = None;
        self.error = None;
        self.current = None;

        if !self.kitty_available() {
            self.error = Some("Preview requires a Kitty-compatible terminal".into());
            return;
        }
        let Some(path) = path else {
            self.error = Some("No DICOM file path available".into());
            return;
        };
        let key = Key {
            path: path.to_path_buf(),
            baseline: baseline.map(Path::to_path_buf),
        };
        self.current = Some(key.clone());

        if let Some(value) = self.cached(&key) {
            self.apply(value);
            return;
        }
        self.enqueue(key, true);
    }

    /// Decodes `paths` ahead of time so they show instantly when selected.
    pub fn prefetch(&mut self, paths: impl IntoIterator<Item = PathBuf>) {
        if !self.show || !self.kitty_available() {
            return;
        }
        for path in paths {
            let key = Key {
                path,
                baseline: None,
            };
            if self.cached(&key).is_none() {
                self.enqueue(key, false);
            }
        }
    }

    fn enqueue(&mut self, key: Key, urgent: bool) {
        if self.in_flight.contains(&key) {
            return;
        }
        let waker = self.waker.clone();
        let worker = self.worker.get_or_insert_with(|| Worker::spawn(waker));
        let job = Job {
            key: key.clone(),
            urgent,
        };
        if worker.jobs.send(job).is_ok() {
            self.in_flight.insert(key);
        }
    }

    /// Looks `key` up in the cache, evicting the entry if the files changed since.
    fn cached(&mut self, key: &Key) -> Option<Decoded> {
        let pos = self.cache.iter().position(|e| &e.key == key)?;
        if self.cache[pos].stamp != Stamp::of(key) {
            self.cache.remove(pos);
            return None;
        }
        // Move to the back: most recently used.
        let entry = self.cache.remove(pos)?;
        let value = entry.value.clone();
        self.cache.push_back(entry);
        Some(value)
    }

    fn insert(&mut self, key: Key, stamp: Stamp, value: Decoded) {
        self.cache.retain(|e| e.key != key);
        if self.cache.len() >= CACHE_CAPACITY {
            self.cache.pop_front();
        }
        self.cache.push_back(CacheEntry { key, stamp, value });
    }

    fn apply(&mut self, value: Decoded) {
        match value.as_ref() {
            Ok(img) => {
                self.source = Some(Arc::new(img.clone()));
                self.error = None;
            }
            Err(e) => self.error = Some(e.clone()),
        }
    }

    /// Collects finished decodes. Cheap when nothing arrived.
    pub fn poll_results(&mut self) {
        let Some(worker) = &self.worker else {
            return;
        };
        let mut finished = Vec::new();
        while let Ok(done) = worker.done.try_recv() {
            finished.push(done);
        }
        for done in finished {
            self.in_flight.remove(&done.key);
            if self.current.as_ref() == Some(&done.key) && self.source.is_none() {
                self.apply(done.value.clone());
            }
            self.insert(done.key, done.stamp, done.value);
        }
    }

    /// Draws the current image into `area`, re-encoding it only when the image or the
    /// pane size changed. Images larger than the pane are shrunk first; smaller ones are
    /// sent at native resolution and scaled up by the terminal.
    pub fn render(&mut self, area: Rect, buf: &mut Buffer) {
        let (Some(source), Some(picker)) = (&self.source, &self.picker) else {
            return;
        };
        let rebuild = !matches!(&self.placement, Some((_, fitted)) if *fitted == area);
        if rebuild {
            let Some(grid) = fit_grid(source.width(), source.height(), picker.font_size(), area)
            else {
                self.placement = None;
                return;
            };
            let prepared = prepare_for_grid(source, &grid);
            let png = encode_png(&prepared);
            let id = kitty::next_id();
            let placement = Placement::new(
                id,
                &png,
                grid.cols,
                grid.rows,
                self.terminal_image.take(),
                self.tmux,
            );
            self.terminal_image = Some(id);
            self.placement = Some((placement, area));
        }
        if let Some((placement, _)) = &mut self.placement {
            placement.render(area, buf);
        }
    }
}

/// Cell grid an image is shown over, and the pixel size it must have (after padding or
/// shrinking) to map onto that grid without distortion.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Grid {
    pub cols: u16,
    pub rows: u16,
    pub px_width: u32,
    pub px_height: u32,
}

/// Largest cell grid inside `area` with (to within one cell) the aspect ratio of a
/// `width × height` image, given `font` pixels per cell. `None` when nothing fits.
pub fn fit_grid(width: u32, height: u32, font: FontSize, area: Rect) -> Option<Grid> {
    if width == 0 || height == 0 || area.width == 0 || area.height == 0 {
        return None;
    }
    let (fw, fh) = (u32::from(font.width).max(1), u32::from(font.height).max(1));
    let avail_w = f64::from(area.width) * f64::from(fw);
    let avail_h = f64::from(area.height) * f64::from(fh);
    let scale = (avail_w / f64::from(width)).min(avail_h / f64::from(height));
    let cols = ((f64::from(width) * scale / f64::from(fw)).floor() as u16).clamp(1, area.width);
    let rows = ((f64::from(height) * scale / f64::from(fh)).floor() as u16).clamp(1, area.height);
    Some(Grid {
        cols,
        rows,
        px_width: u32::from(cols) * fw,
        px_height: u32::from(rows) * fh,
    })
}

/// Returns `img` ready to be stretched over `grid`: shrunk (box filter) if it exceeds the
/// grid's pixels, then letterboxed with black so its aspect ratio matches the grid exactly.
pub fn prepare_for_grid(img: &DynamicImage, grid: &Grid) -> DynamicImage {
    let (w, h) = (img.width(), img.height());
    let img = if w > grid.px_width || h > grid.px_height {
        let scale =
            (f64::from(grid.px_width) / f64::from(w)).min(f64::from(grid.px_height) / f64::from(h));
        let tw = ((f64::from(w) * scale).round() as u32).clamp(1, grid.px_width);
        let th = ((f64::from(h) * scale).round() as u32).clamp(1, grid.px_height);
        std::borrow::Cow::Owned(downscale(img, tw, th))
    } else {
        std::borrow::Cow::Borrowed(img)
    };
    let (w, h) = (img.width(), img.height());
    // Smallest canvas with the grid's aspect ratio that contains the image.
    let (cw, ch) = if u64::from(w) * u64::from(grid.px_height)
        >= u64::from(h) * u64::from(grid.px_width)
    {
        (
            w,
            (u64::from(w) * u64::from(grid.px_height)).div_ceil(u64::from(grid.px_width)) as u32,
        )
    } else {
        (
            (u64::from(h) * u64::from(grid.px_width)).div_ceil(u64::from(grid.px_height)) as u32,
            h,
        )
    };
    if (cw, ch) == (w, h) {
        return img.into_owned();
    }
    let mut canvas = DynamicImage::new(cw, ch, img.color());
    image::imageops::replace(
        &mut canvas,
        img.as_ref(),
        i64::from((cw - w) / 2),
        i64::from((ch - h) / 2),
    );
    canvas
}

fn downscale(img: &DynamicImage, w: u32, h: u32) -> DynamicImage {
    match img {
        DynamicImage::ImageLuma8(i) => image::imageops::thumbnail(i, w, h).into(),
        DynamicImage::ImageRgb8(i) => image::imageops::thumbnail(i, w, h).into(),
        other => image::imageops::thumbnail(&other.to_rgb8(), w, h).into(),
    }
}

/// Fast PNG encoding (speed over size: the bytes go to a local terminal).
pub fn encode_png(img: &DynamicImage) -> Vec<u8> {
    let mut out = Vec::with_capacity(img.as_bytes().len() / 4);
    PngEncoder::new_with_quality(&mut out, CompressionType::Fast, PngFilter::Adaptive)
        .write_image(
            img.as_bytes(),
            img.width(),
            img.height(),
            img.color().into(),
        )
        .expect("in-memory PNG encoding cannot fail");
    out
}

fn decode_key(key: &Key) -> Result<DynamicImage, String> {
    match &key.baseline {
        Some(baseline) => {
            let a = decode_dicom_image(baseline)?;
            let b = decode_dicom_image(&key.path)?;
            compose_diff_image(&a, &b)
        }
        None => decode_dicom_image(&key.path),
    }
}

/// Decodes frame 0 of `path` to an 8-bit grayscale or RGB image.
pub fn decode_dicom_image(path: &Path) -> Result<DynamicImage, String> {
    use dicom_pixeldata::{ConvertOptions, PixelDecoder};

    let obj =
        dicom_object::open_file(path).map_err(|e| format!("Failed to open DICOM file: {e}"))?;

    let pixel_data = obj
        .decode_pixel_data()
        .map_err(|e| format!("Failed to decode pixel data: {e}"))?;

    let img = pixel_data
        .to_dynamic_image_with_options(0, &ConvertOptions::new().force_8bit())
        .map_err(|e| format!("Failed to convert to image: {e}"))?;

    Ok(match img {
        DynamicImage::ImageLuma8(_) | DynamicImage::ImageRgb8(_) => img,
        DynamicImage::ImageLumaA8(_)
        | DynamicImage::ImageLuma16(_)
        | DynamicImage::ImageLumaA16(_) => DynamicImage::ImageLuma8(img.to_luma8()),
        other => DynamicImage::ImageRgb8(other.to_rgb8()),
    })
}

/// Blends baseline (A) and modified (B) into one RGB image: pixels equal in both
/// render gray, pixels brighter in B tint green, pixels brighter in A tint red.
pub fn compose_diff_image(
    baseline: &DynamicImage,
    modified: &DynamicImage,
) -> Result<DynamicImage, String> {
    let a = baseline.to_luma8();
    let b = modified.to_luma8();

    let (aw, ah) = a.dimensions();
    let (bw, bh) = b.dimensions();
    if (aw, ah) != (bw, bh) {
        return Err(format!(
            "Image dimensions differ: {aw}\u{d7}{ah} (A) vs {bw}\u{d7}{bh} (B)"
        ));
    }

    let out = image::RgbImage::from_fn(aw, ah, |x, y| {
        let a = a.get_pixel(x, y).0[0];
        let b = b.get_pixel(x, y).0[0];
        image::Rgb([a, b, a.min(b)])
    });

    Ok(DynamicImage::ImageRgb8(out))
}
