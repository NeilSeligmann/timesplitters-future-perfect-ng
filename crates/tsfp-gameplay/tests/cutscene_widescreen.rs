// SPDX-License-Identifier: GPL-3.0-or-later
use tsfp_gameplay::cutscene_presentation::*;

fn extent(width: i32, height: i32) -> Extent {
    Extent { width, height }
}

#[test]
fn movie_fits_full_source_and_bars_partition_the_output() {
    for (width, height) in [
        (640, 480),
        (1920, 1080),
        (2520, 1080),
        (1080, 1920),
        (1919, 1079),
    ] {
        let policy = WidePresentation::new(extent(width, height)).unwrap();
        for aspect in [4.0 / 3.0, 16.0 / 9.0, 21.0 / 9.0] {
            let movie = policy.movie_frame(extent(640, 480), aspect).unwrap();
            assert_eq!(
                movie.source,
                Rect {
                    left: 0,
                    top: 0,
                    right: 640,
                    bottom: 480
                }
            );
            let r = movie.destination;
            assert!(r.left >= 0 && r.top >= 0 && r.right <= width && r.bottom <= height);
            let mut area = (r.right - r.left) as i64 * (r.bottom - r.top) as i64;
            for bar in policy.bars(aspect).unwrap() {
                assert!(
                    bar.right <= r.left
                        || bar.left >= r.right
                        || bar.bottom <= r.top
                        || bar.top >= r.bottom
                );
                area += (bar.right - bar.left) as i64 * (bar.bottom - bar.top) as i64;
            }
            assert_eq!(area, width as i64 * height as i64);
        }
    }
    let p = WidePresentation::new(extent(2520, 1080)).unwrap();
    assert_eq!(
        p.content_rect(4.0 / 3.0).unwrap(),
        Rect {
            left: 540,
            top: 0,
            right: 1980,
            bottom: 1080
        }
    );
    assert!(WidePresentation::new(extent(0, 480)).is_none());
    assert!(p.content_rect(f64::NAN).is_none());
    assert!(p.content_rect(0.0).is_none());
    assert!(p.movie_frame(extent(-1, 480), 4.0 / 3.0).is_none());
}

#[test]
fn original_policy_and_exact_aspects_keep_the_original_lens_arithmetic() {
    let calibration = LensCalibration {
        lens_scale: 1.0,
        numerator: 10.373,
        aperture_scale: 1.0,
        aperture: 1.0,
        angle_units: 180.0 / std::f32::consts::PI,
        wide_scale: 0.79,
    };
    for lens in [5.0, 20.0, 35.0, 100.0] {
        for (w, h, wide) in [(640, 480, false), (1920, 1080, true)] {
            let policy = WidePresentation::new(extent(w, h)).unwrap();
            assert_eq!(
                policy.optical_fov(calibration, lens).to_bits(),
                calibration.original_fov(lens, wide).to_bits()
            );
        }
        let p = WidePresentation::new(extent(2520, 1080)).unwrap();
        let base = calibration.original_fov(lens, false) as f64;
        let expected = (base / (2.0 * calibration.angle_units as f64)).tan() * (4.0 / 7.0);
        let actual = (p.optical_fov(calibration, lens) as f64
            / (2.0 * calibration.angle_units as f64))
            .tan();
        assert!((actual - expected).abs() < 1e-6);
    }
    let camera = CameraPresentation {
        position: [1.0, 2.0, 3.0],
        basis: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
        fov: 55.0,
    };
    assert_eq!(Original.camera(camera), camera);
    let movie = MovieState {
        crop_inset: extent(40, 65),
        placement: MoviePlacement::OriginalAspect,
    }
    .rectangles(
        extent(640, 480),
        extent(640, 480),
        extent(640, 480),
        4.0 / 3.0,
    );
    assert_eq!(Original.movie(movie), movie);
    assert_eq!(
        movie.source,
        Rect {
            left: 40,
            top: 65,
            right: 600,
            bottom: 415
        }
    );
}
