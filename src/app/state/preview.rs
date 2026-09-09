use image::DynamicImage;
use ratatui::layout::Rect;
use ratatui_image::picker::Picker;
use ratatui_image::protocol::StatefulProtocol;
use ratatui_image::FontSize;
use std::path::Path;
use std::time::{Duration, Instant};

pub struct Preview {
    pub show: bool,
    pub image: Option<StatefulProtocol>,
    pub error: Option<String>,
    pub pending_since: Option<Instant>,
    pub picker: Option<Picker>,
    /// Decoded image at native resolution; `image` is rebuilt from it whenever
    /// the pane area changes so small images can be upscaled to fill the pane.
    source: Option<DynamicImage>,
    /// Pane area (in cells) the current `image` protocol was built for.
    fitted_area: Option<Rect>,
}

impl Preview {
    pub fn new(picker: Option<Picker>) -> Self {
        Self {
            show: true,
            image: None,
            error: None,
            pending_since: None,
            picker,
            source: None,
            fitted_area: None,
        }
    }

    /// Drops the decoded image and any error, without touching `show`.
    pub fn clear(&mut self) {
        self.image = None;
        self.source = None;
        self.fitted_area = None;
        self.error = None;
    }

    /// True once a file has been decoded (whether or not a protocol has been fitted yet).
    pub fn has_image(&self) -> bool {
        self.source.is_some()
    }

    /// (Re)builds the render protocol for `area` if needed. Images smaller than the
    /// pane are upscaled (aspect preserved) so they fill it instead of sitting in a corner.
    pub fn fit_to(&mut self, area: Rect) {
        let Some(source) = &self.source else {
            return;
        };
        if self.image.is_some() && self.fitted_area == Some(area) {
            return;
        }
        let Some(mut picker) = self.picker.take() else {
            return;
        };
        let img = upscale_to_fill(source, picker.font_size(), area);
        self.image = Some(picker.new_resize_protocol(img));
        self.picker = Some(picker);
        self.fitted_area = Some(area);
    }

    pub fn toggle(&mut self, path: Option<&Path>, baseline: Option<&Path>) {
        self.show = !self.show;

        if self.show && self.source.is_none() && self.error.is_none() {
            self.decode(path, baseline);
        }
    }

    pub fn tick_debounce(&mut self, path: Option<&Path>, baseline: Option<&Path>) {
        if let Some(since) = self.pending_since {
            if since.elapsed() >= Duration::from_millis(100) {
                self.pending_since = None;
                self.decode(path, baseline);
            }
        }
    }

    /// Decodes the preview image. With a `baseline` path (diff mode), renders a
    /// red/green composite of baseline (A) vs `path` (B); otherwise renders `path` alone.
    pub fn decode(&mut self, path: Option<&Path>, baseline: Option<&Path>) {
        use ratatui_image::picker::ProtocolType;

        let Some(picker) = &self.picker else {
            self.error = Some("Preview requires a Kitty-compatible terminal".into());
            return;
        };

        if picker.protocol_type() != ProtocolType::Kitty {
            self.error = Some("Preview requires a Kitty-compatible terminal".into());
            return;
        }

        let Some(path) = path else {
            self.error = Some("No DICOM file path available".into());
            return;
        };

        let result = match baseline {
            Some(baseline) => decode_dicom_image(baseline)
                .and_then(|a| decode_dicom_image(path).map(|b| (a, b)))
                .and_then(|(a, b)| compose_diff_image(&a, &b)),
            None => decode_dicom_image(path),
        };

        let dyn_img = match result {
            Ok(img) => img,
            Err(e) => {
                self.error = Some(e);
                return;
            }
        };

        self.source = Some(dyn_img);
        self.image = None;
        self.fitted_area = None;
    }
}

/// Returns `img` scaled up (nearest-neighbour, aspect preserved) to fill `area` cells of
/// `font_size` pixels each, when the image is smaller than the area on both axes.
/// Larger images are returned unchanged; the widget's `Resize::Fit` shrinks them at render.
pub fn upscale_to_fill(img: &DynamicImage, font_size: FontSize, area: Rect) -> DynamicImage {
    let target_w = u32::from(area.width) * u32::from(font_size.0);
    let target_h = u32::from(area.height) * u32::from(font_size.1);
    if target_w == 0 || target_h == 0 || img.width() >= target_w || img.height() >= target_h {
        return img.clone();
    }
    img.resize(target_w, target_h, image::imageops::FilterType::Nearest)
}

fn decode_dicom_image(path: &Path) -> Result<DynamicImage, String> {
    use dicom_pixeldata::PixelDecoder;

    let obj =
        dicom_object::open_file(path).map_err(|e| format!("Failed to open DICOM file: {e}"))?;

    let pixel_data = obj
        .decode_pixel_data()
        .map_err(|e| format!("Failed to decode pixel data: {e}"))?;

    pixel_data
        .to_dynamic_image(0)
        .map_err(|e| format!("Failed to convert to image: {e}"))
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
