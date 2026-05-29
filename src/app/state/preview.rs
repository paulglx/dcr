use ratatui_image::picker::Picker;
use ratatui_image::protocol::StatefulProtocol;
use std::path::Path;
use std::time::{Duration, Instant};

pub struct Preview {
    pub show: bool,
    pub image: Option<StatefulProtocol>,
    pub error: Option<String>,
    pub pending_since: Option<Instant>,
    pub picker: Option<Picker>,
}

impl Preview {
    pub fn new(picker: Option<Picker>) -> Self {
        Self {
            show: true,
            image: None,
            error: None,
            pending_since: None,
            picker,
        }
    }

    pub fn toggle(&mut self, path: Option<&Path>) {
        self.show = !self.show;

        if self.show && self.image.is_none() && self.error.is_none() {
            self.decode(path);
        }
    }

    pub fn tick_debounce(&mut self, path: Option<&Path>) {
        if let Some(since) = self.pending_since {
            if since.elapsed() >= Duration::from_millis(100) {
                self.pending_since = None;
                self.decode(path);
            }
        }
    }

    pub fn decode(&mut self, path: Option<&Path>) {
        use dicom_pixeldata::PixelDecoder;
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

        let obj = match dicom::object::open_file(path) {
            Ok(obj) => obj,
            Err(e) => {
                self.error = Some(format!("Failed to open DICOM file: {e}"));
                return;
            }
        };

        let pixel_data = match obj.decode_pixel_data() {
            Ok(pd) => pd,
            Err(e) => {
                self.error = Some(format!("Failed to decode pixel data: {e}"));
                return;
            }
        };

        let dyn_img = match pixel_data.to_dynamic_image(0) {
            Ok(img) => img,
            Err(e) => {
                self.error = Some(format!("Failed to convert to image: {e}"));
                return;
            }
        };

        let mut picker = self.picker.take().unwrap();
        self.image = Some(picker.new_resize_protocol(dyn_img));
        self.picker = Some(picker);
    }
}
