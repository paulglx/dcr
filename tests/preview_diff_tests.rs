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

mod upscale {
    use super::gray_image;
    use dcr::app::state::preview::upscale_to_fill;
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
    fn small_image_is_upscaled_to_fill_pane_preserving_aspect() {
        let img = gray_image(20, 20, 7);

        let out = upscale_to_fill(&img, FONT, PANE);

        // Height is the limiting axis: 200 px tall, square keeps 200 px wide.
        assert_eq!((out.width(), out.height()), (200, 200));
        assert!(out.to_luma8().pixels().all(|p| p.0 == [7]));
    }

    #[test]
    fn large_image_is_left_untouched() {
        let img = gray_image(1000, 1000, 0);

        let out = upscale_to_fill(&img, FONT, PANE);

        assert_eq!((out.width(), out.height()), (1000, 1000));
    }

    #[test]
    fn image_exceeding_pane_on_one_axis_is_not_upscaled() {
        let img = gray_image(50, 300, 0);

        let out = upscale_to_fill(&img, FONT, PANE);

        assert_eq!((out.width(), out.height()), (50, 300));
    }

    #[test]
    fn empty_pane_returns_image_unchanged() {
        let img = gray_image(20, 20, 0);

        let out = upscale_to_fill(&img, FONT, Rect::default());

        assert_eq!((out.width(), out.height()), (20, 20));
    }
}
