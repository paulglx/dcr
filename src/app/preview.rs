use super::App;

impl App {
    pub(super) fn toggle_preview(&mut self) {
        self.show_preview = !self.show_preview;

        if self.show_preview && self.preview_image.is_none() && self.preview_error.is_none() {
            self.decode_preview();
        }
    }

    pub(super) fn decode_preview(&mut self) {
        use dicom_pixeldata::PixelDecoder;
        use ratatui_image::picker::ProtocolType;

        let Some(picker) = &self.picker else {
            self.preview_error = Some("Preview requires a Kitty-compatible terminal".into());
            return;
        };

        if picker.protocol_type() != ProtocolType::Kitty {
            self.preview_error = Some("Preview requires a Kitty-compatible terminal".into());
            return;
        }

        let Some(path) = &self.dicom_file_path else {
            self.preview_error = Some("No DICOM file path available".into());
            return;
        };

        let obj = match dicom::object::open_file(path) {
            Ok(obj) => obj,
            Err(e) => {
                self.preview_error = Some(format!("Failed to open DICOM file: {e}"));
                return;
            }
        };

        let pixel_data = match obj.decode_pixel_data() {
            Ok(pd) => pd,
            Err(e) => {
                self.preview_error = Some(format!("Failed to decode pixel data: {e}"));
                return;
            }
        };

        let dyn_img = match pixel_data.to_dynamic_image(0) {
            Ok(img) => img,
            Err(e) => {
                self.preview_error = Some(format!("Failed to convert to image: {e}"));
                return;
            }
        };

        let mut picker = self.picker.take().unwrap();
        self.preview_image = Some(picker.new_resize_protocol(dyn_img));
        self.picker = Some(picker);
    }
}
