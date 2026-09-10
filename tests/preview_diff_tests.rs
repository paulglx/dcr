use dcr::app::state::preview::compose_diff_image;
use image::{DynamicImage, GrayImage, Luma};

fn gray_image(w: u32, h: u32, fill: u8) -> DynamicImage {
    DynamicImage::ImageLuma8(GrayImage::from_pixel(w, h, Luma([fill])))
}

#[test]
fn identical_images_render_neutral_gray() {
    let a = gray_image(4, 4, 120);
    let b = gray_image(4, 4, 120);

    let out = compose_diff_image(&a, &b).unwrap().to_rgb8();

    for pixel in out.pixels() {
        assert_eq!(pixel.0, [120, 120, 120]);
    }
}

#[test]
fn pixels_brighter_in_modified_tint_green() {
    let a = gray_image(2, 2, 50);
    let b = gray_image(2, 2, 200);

    let out = compose_diff_image(&a, &b).unwrap().to_rgb8();

    for pixel in out.pixels() {
        let [r, g, bl] = pixel.0;
        assert_eq!((r, g, bl), (50, 200, 50), "B-only content must read green");
    }
}

#[test]
fn pixels_brighter_in_baseline_tint_red() {
    let a = gray_image(2, 2, 200);
    let b = gray_image(2, 2, 50);

    let out = compose_diff_image(&a, &b).unwrap().to_rgb8();

    for pixel in out.pixels() {
        let [r, g, bl] = pixel.0;
        assert_eq!((r, g, bl), (200, 50, 50), "A-only content must read red");
    }
}

#[test]
fn tint_intensity_scales_with_difference() {
    let mut img_b = GrayImage::from_pixel(2, 1, Luma([100]));
    img_b.put_pixel(1, 0, Luma([160]));
    let a = gray_image(2, 1, 100);
    let b = DynamicImage::ImageLuma8(img_b);

    let out = compose_diff_image(&a, &b).unwrap().to_rgb8();

    let subtle = out.get_pixel(0, 0).0;
    let strong = out.get_pixel(1, 0).0;
    assert_eq!(subtle, [100, 100, 100]);
    assert_eq!(strong, [100, 160, 100]);
    assert!(strong[1] - strong[0] > subtle[1] - subtle[0]);
}

#[test]
fn dimension_mismatch_is_an_error() {
    let a = gray_image(4, 4, 0);
    let b = gray_image(2, 4, 0);

    let err = compose_diff_image(&a, &b).unwrap_err();

    assert!(err.contains("dimensions differ"), "got: {err}");
    assert!(err.contains("4×4"), "got: {err}");
    assert!(err.contains("2×4"), "got: {err}");
}

mod geometry {
    use super::gray_image;
    use dcr::app::state::preview::{fit_grid, prepare_for_grid, Grid};
    use ratatui::layout::Rect;
    use ratatui_image::FontSize;

    // 10x20 px cells; a 40x10 cell pane is 400x200 px.
    const FONT: FontSize = FontSize::new(10, 20);
    const PANE: Rect = Rect {
        x: 0,
        y: 0,
        width: 40,
        height: 10,
    };

    #[test]
    fn square_image_takes_the_largest_square_grid_in_the_pane() {
        // Height limits: 200 px → 10 rows; 200 px wide → 20 columns.
        let grid = fit_grid(512, 512, FONT, PANE).unwrap();
        assert_eq!(
            grid,
            Grid {
                cols: 20,
                rows: 10,
                px_width: 200,
                px_height: 200
            }
        );
    }

    #[test]
    fn wide_image_is_limited_by_the_pane_width() {
        // 4:1 image in a 2:1 pane: 400 px wide → 40 cols, 100 px tall → 5 rows.
        let grid = fit_grid(800, 200, FONT, PANE).unwrap();
        assert_eq!((grid.cols, grid.rows), (40, 5));
    }

    #[test]
    fn grid_never_exceeds_the_pane_nor_drops_below_one_cell() {
        let grid = fit_grid(1, 10_000, FONT, PANE).unwrap();
        assert_eq!((grid.cols, grid.rows), (1, 10));
        let grid = fit_grid(10_000, 1, FONT, PANE).unwrap();
        assert_eq!((grid.cols, grid.rows), (40, 1));
    }

    #[test]
    fn empty_pane_or_image_has_no_grid() {
        assert!(fit_grid(20, 20, FONT, Rect::default()).is_none());
        assert!(fit_grid(0, 20, FONT, PANE).is_none());
    }

    #[test]
    fn small_image_is_kept_at_native_resolution_and_letterboxed_to_the_grid_aspect() {
        // Grid is 20x10 cells = 200x200 px (square); a 30x20 image gets padded to 30x30.
        let grid = fit_grid(512, 512, FONT, PANE).unwrap();
        let img = gray_image(30, 20, 7);

        let out = prepare_for_grid(&img, &grid);

        assert_eq!((out.width(), out.height()), (30, 30));
        let out = out.to_luma8();
        assert_eq!(out.get_pixel(0, 0).0, [0], "top padding is black");
        assert_eq!(out.get_pixel(0, 5).0, [7], "image is centred");
        assert_eq!(out.get_pixel(29, 24).0, [7]);
        assert_eq!(out.get_pixel(0, 29).0, [0], "bottom padding is black");
    }

    #[test]
    fn image_already_matching_the_grid_aspect_is_returned_as_is() {
        let grid = fit_grid(512, 512, FONT, PANE).unwrap();
        let img = gray_image(64, 64, 3);

        let out = prepare_for_grid(&img, &grid);

        assert_eq!((out.width(), out.height()), (64, 64));
        assert!(out.to_luma8().pixels().all(|p| p.0 == [3]));
    }

    #[test]
    fn large_image_is_shrunk_to_the_grid_pixels() {
        let grid = fit_grid(2000, 1000, FONT, PANE).unwrap();
        assert_eq!((grid.px_width, grid.px_height), (400, 200));
        let img = gray_image(2000, 1000, 9);

        let out = prepare_for_grid(&img, &grid);

        assert_eq!((out.width(), out.height()), (400, 200));
        assert!(out.to_luma8().pixels().all(|p| p.0 == [9]));
    }
}

mod kitty {
    use dcr::app::kitty::{base64_encode, transmit_png};

    fn b64(data: &[u8]) -> String {
        let mut s = String::new();
        base64_encode(data, &mut s);
        s
    }

    #[test]
    fn base64_matches_the_standard_alphabet_and_padding() {
        assert_eq!(b64(b""), "");
        assert_eq!(b64(b"f"), "Zg==");
        assert_eq!(b64(b"fo"), "Zm8=");
        assert_eq!(b64(b"foo"), "Zm9v");
        assert_eq!(b64(b"foobar"), "Zm9vYmFy");
        assert_eq!(b64(&[0xff, 0xfe, 0xfd, 0x00]), "//79AA==");
    }

    #[test]
    fn transmit_is_a_single_png_virtual_placement_with_the_grid_size() {
        let seq = transmit_png(b"png-bytes", 0x0102_0304, 20, 10, None, false);
        assert_eq!(
            seq,
            "\x1b_Gq=2,a=T,U=1,t=d,f=100,i=16909060,c=20,r=10,m=0;cG5nLWJ5dGVz\x1b\\"
        );
    }

    #[test]
    fn transmit_is_chunked_and_deletes_the_previous_image() {
        let data = vec![0u8; 3072 + 3];
        let seq = transmit_png(&data, 7, 4, 2, Some(5), false);
        let chunks: Vec<&str> = seq.split("\x1b\\").filter(|s| !s.is_empty()).collect();
        assert_eq!(chunks.len(), 3);
        assert_eq!(chunks[0], "\x1b_Ga=d,d=I,i=5,q=2");
        assert!(chunks[1].starts_with("\x1b_Gq=2,a=T,U=1,t=d,f=100,i=7,c=4,r=2,m=1;"));
        assert_eq!(
            chunks[1].len(),
            "\x1b_Gq=2,a=T,U=1,t=d,f=100,i=7,c=4,r=2,m=1;".len() + 4096
        );
        assert_eq!(chunks[2], "\x1b_Gq=2,m=0;AAAA");
    }

    #[test]
    fn transmit_is_wrapped_for_tmux_passthrough() {
        let seq = transmit_png(b"abc", 1, 1, 1, None, true);
        assert_eq!(
            seq,
            "\x1bPtmux;\x1b\x1b_Gq=2,a=T,U=1,t=d,f=100,i=1,c=1,r=1,m=0;YWJj\x1b\x1b\\\x1b\\"
        );
    }
}

mod pipeline {
    use dcr::app::state::preview::Preview;
    use ratatui::buffer::Buffer;
    use ratatui::layout::Rect;
    use ratatui_image::picker::{Picker, ProtocolType};
    use ratatui_image::FontSize;
    use std::path::Path;
    use std::time::{Duration, Instant};

    const CT: &str = "tests/fixtures/dicom/ct-tap.dcm";

    // `from_fontsize` is the only constructor that takes an explicit cell size without
    // probing a terminal, which tests do not have.
    #[allow(deprecated)]
    fn kitty_preview() -> Preview {
        let mut picker = Picker::from_fontsize(FontSize::new(10, 20));
        picker.set_protocol_type(ProtocolType::Kitty);
        Preview::new(Some(picker))
    }

    fn wait_for_image(preview: &mut Preview) {
        let deadline = Instant::now() + Duration::from_secs(10);
        while !preview.has_image() && preview.error.is_none() {
            assert!(Instant::now() < deadline, "decode timed out");
            std::thread::sleep(Duration::from_millis(2));
            preview.poll_results();
        }
    }

    #[test]
    fn without_kitty_the_pane_reports_the_missing_capability() {
        let mut preview = Preview::new(None);
        preview.request(Some(Path::new(CT)), None);
        assert_eq!(
            preview.error.as_deref(),
            Some("Preview requires a Kitty-compatible terminal")
        );
        assert!(!preview.is_loading());
    }

    #[test]
    fn decodes_in_the_background_and_renders_placeholders_with_one_png_transmit() {
        let mut preview = kitty_preview();
        preview.request(Some(Path::new(CT)), None);
        assert!(preview.is_loading(), "decode runs off the main thread");
        wait_for_image(&mut preview);
        assert!(preview.error.is_none(), "{:?}", preview.error);

        // 40x10 cells of 10x20 px: the 512² CT fits a 10-row, 20-column square.
        let area = Rect::new(2, 1, 40, 10);
        let mut buf = Buffer::empty(Rect::new(0, 0, 50, 12));
        preview.render(area, &mut buf);

        let first = buf[(2, 1)].symbol().to_string();
        assert!(first.contains("_Gq=2,a=T,U=1,t=d,f=100,i="), "{first:.80}");
        assert!(first.contains(",c=20,r=10,m="), "{first:.120}");
        assert_eq!(
            first.matches('\u{10EEEE}').count(),
            20,
            "one placeholder per column"
        );
        let last_row = buf[(2, 10)].symbol().to_string();
        assert!(
            !last_row.contains("_G"),
            "transmit is sent once, on the first row"
        );
        assert_eq!(last_row.matches('\u{10EEEE}').count(), 20);
        assert_eq!(
            buf[(22, 1)].symbol(),
            " ",
            "cells right of the grid are untouched"
        );

        // A second draw of the same area re-uses the transmitted image.
        let mut buf2 = Buffer::empty(Rect::new(0, 0, 50, 12));
        preview.render(area, &mut buf2);
        assert!(!buf2[(2, 1)].symbol().contains("_G"));

        // A resize re-encodes and deletes the previous image from the terminal.
        let mut buf3 = Buffer::empty(Rect::new(0, 0, 50, 12));
        preview.render(Rect::new(0, 0, 20, 5), &mut buf3);
        let sym = buf3[(0, 0)].symbol().to_string();
        assert!(sym.contains("_Ga=d,d=I,i="), "{sym:.80}");
        assert!(sym.contains(",c=10,r=5,m="), "{sym:.160}");
    }

    #[test]
    fn revisiting_a_file_is_served_from_the_cache_without_a_decode() {
        let mut preview = kitty_preview();
        preview.request(Some(Path::new(CT)), None);
        wait_for_image(&mut preview);

        preview.clear();
        assert!(!preview.has_image());
        preview.request(Some(Path::new(CT)), None);
        assert!(preview.has_image(), "cache hit is synchronous");
        assert!(!preview.is_loading());
    }

    #[test]
    fn prefetched_files_show_up_instantly_when_selected() {
        let mut preview = kitty_preview();
        let other = Path::new("tests/fixtures/dicom/ct-tap-with-missing-data.dcm");
        preview.request(Some(Path::new(CT)), None);
        preview.prefetch([other.to_path_buf()]);
        wait_for_image(&mut preview);
        // Let the prefetch land too.
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            preview.poll_results();
            preview.clear();
            preview.request(Some(other), None);
            if preview.has_image() || preview.error.is_some() {
                break;
            }
            assert!(Instant::now() < deadline, "prefetch timed out");
            std::thread::sleep(Duration::from_millis(2));
        }
        assert!(preview.has_image(), "{:?}", preview.error);
    }

    #[test]
    fn missing_file_reports_the_open_error() {
        let mut preview = kitty_preview();
        preview.request(Some(Path::new("tests/fixtures/dicom/nope.dcm")), None);
        wait_for_image(&mut preview);
        assert!(
            preview
                .error
                .as_deref()
                .is_some_and(|e| e.starts_with("Failed to open DICOM file")),
            "{:?}",
            preview.error
        );
    }
}
