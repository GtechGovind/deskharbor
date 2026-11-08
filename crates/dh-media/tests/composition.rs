use dh_media::plane_compositor::*;

fn plane<'a>(pixels: &'a [u8], width: u32, height: u32) -> Plane<'a> {
    Plane {
        id: 1,
        z: 0,
        kind: Kind::Primary,
        width,
        height,
        stride: width * 4,
        pixels,
        format: PixelFormat::Rgba,
        blend: Blend::None,
        opacity: u16::MAX,
        crop: Crop {
            x: 0,
            y: 0,
            width,
            height,
        },
        destination: Destination {
            x: 0,
            y: 0,
            width,
            height,
        },
        rotation: Rotation::R0,
        reflect_x: false,
        reflect_y: false,
    }
}

#[test]
fn overlay_and_cursor_follow_z_order_and_exact_coverage_alpha() {
    let blue = [0, 0, 255, 255, 0, 0, 255, 255];
    let red = [255, 0, 0, 128];
    let green = [0, 255, 0, 255];
    let primary = plane(&blue, 2, 1);
    let mut overlay = plane(&red, 1, 1);
    overlay.id = 2;
    overlay.z = 1;
    overlay.kind = Kind::Overlay;
    overlay.blend = Blend::Coverage;
    let mut cursor = plane(&green, 1, 1);
    cursor.id = 3;
    cursor.z = 2;
    cursor.kind = Kind::Cursor;
    cursor.destination.x = 1;
    let result = compose(&Scanout {
        width: 2,
        height: 1,
        planes: vec![cursor, primary, overlay],
    })
    .unwrap();
    assert_eq!(result.pixels_rgba, [128, 0, 127, 255, 0, 255, 0, 255]);
}

#[test]
fn premultiplied_and_global_opacity_have_independent_expected_pixels() {
    let blue = [0, 0, 255, 255];
    let red = [128, 0, 0, 128];
    let mut overlay = plane(&red, 1, 1);
    overlay.id = 2;
    overlay.z = 1;
    overlay.kind = Kind::Overlay;
    overlay.blend = Blend::Premultiplied;
    let result = compose(&Scanout {
        width: 1,
        height: 1,
        planes: vec![plane(&blue, 1, 1), overlay.clone()],
    })
    .unwrap();
    assert_eq!(result.pixels_rgba, [128, 0, 127, 255]);
    overlay.opacity = 32768;
    let result = compose(&Scanout {
        width: 1,
        height: 1,
        planes: vec![plane(&blue, 1, 1), overlay],
    })
    .unwrap();
    assert_eq!(result.pixels_rgba, [64, 0, 191, 255]);
}

#[test]
fn padded_bgra_rows_and_bgrx_alpha_are_not_misinterpreted() {
    let bgra = [
        0, 0, 255, 0, 255, 0, 0, 0, 99, 99, 99, 99, 0, 255, 0, 0, 255, 255, 255, 0, 99, 99, 99, 99,
    ];
    let mut primary = plane(&bgra, 2, 2);
    primary.stride = 12;
    primary.format = PixelFormat::Bgrx;
    let result = compose(&Scanout {
        width: 2,
        height: 2,
        planes: vec![primary],
    })
    .unwrap();
    assert_eq!(
        result.pixels_rgba,
        [
            255, 0, 0, 255, 0, 0, 255, 255, 0, 255, 0, 255, 255, 255, 255, 255
        ]
    );
}

#[test]
fn source_crop_and_negative_destination_clip_the_correct_pixels() {
    let black = [0, 0, 0, 255].repeat(6);
    let source = [10, 20, 30, 40, 50, 60]
        .into_iter()
        .flat_map(|r| [r, 0, 0, 255])
        .collect::<Vec<_>>();
    let mut overlay = plane(&source, 3, 2);
    overlay.id = 2;
    overlay.z = 1;
    overlay.kind = Kind::Overlay;
    overlay.crop = Crop {
        x: 1,
        y: 0,
        width: 2,
        height: 2,
    };
    overlay.destination = Destination {
        x: -1,
        y: 0,
        width: 2,
        height: 2,
    };
    let result = compose(&Scanout {
        width: 3,
        height: 2,
        planes: vec![plane(&black, 3, 2), overlay],
    })
    .unwrap();
    assert_eq!(
        result.pixels_rgba,
        [
            30, 0, 0, 255, 0, 0, 0, 255, 0, 0, 0, 255, 60, 0, 0, 255, 0, 0, 0, 255, 0, 0, 0, 255
        ]
    );
}

#[test]
fn quarter_turns_and_source_reflection_match_asymmetric_fixture() {
    let source = (1..=6).flat_map(|r| [r, 0, 0, 255]).collect::<Vec<_>>();
    let cases = [
        (Rotation::R90, false, 3, 2, vec![5, 3, 1, 6, 4, 2]),
        (Rotation::R180, false, 2, 3, vec![6, 5, 4, 3, 2, 1]),
        (Rotation::R270, false, 3, 2, vec![2, 4, 6, 1, 3, 5]),
        (Rotation::R90, true, 3, 2, vec![6, 4, 2, 5, 3, 1]),
    ];
    for (rotation, reflect_x, width, height, expected) in cases {
        let mut primary = plane(&source, 2, 3);
        primary.rotation = rotation;
        primary.reflect_x = reflect_x;
        primary.destination.width = width;
        primary.destination.height = height;
        let result = compose(&Scanout {
            width,
            height,
            planes: vec![primary],
        })
        .unwrap();
        assert_eq!(
            result
                .pixels_rgba
                .chunks_exact(4)
                .map(|p| p[0])
                .collect::<Vec<_>>(),
            expected
        );
    }
}

#[test]
fn invalid_layout_bounds_order_and_budget_are_rejected() {
    let pixels = [0, 0, 0, 255];
    let base = plane(&pixels, 1, 1);
    assert_eq!(
        compose(&Scanout {
            width: 0,
            height: 1,
            planes: vec![base.clone()]
        })
        .unwrap_err(),
        CompositionError::InvalidDimensions
    );
    assert_eq!(
        compose(&Scanout {
            width: 16384,
            height: 16384,
            planes: vec![base.clone()]
        })
        .unwrap_err(),
        CompositionError::PixelBudget
    );
    let mut wrong = base.clone();
    wrong.stride = 3;
    assert_eq!(
        compose(&Scanout {
            width: 1,
            height: 1,
            planes: vec![wrong]
        })
        .unwrap_err(),
        CompositionError::BufferLayout
    );
    let mut wrong = base.clone();
    wrong.crop.x = u32::MAX;
    assert_eq!(
        compose(&Scanout {
            width: 1,
            height: 1,
            planes: vec![wrong]
        })
        .unwrap_err(),
        CompositionError::SourceBounds
    );
    let mut wrong = base.clone();
    wrong.destination.width = 2;
    assert_eq!(
        compose(&Scanout {
            width: 1,
            height: 1,
            planes: vec![wrong]
        })
        .unwrap_err(),
        CompositionError::UnsupportedScaling
    );
    let mut other = base.clone();
    other.id = 2;
    other.kind = Kind::Cursor;
    assert_eq!(
        compose(&Scanout {
            width: 1,
            height: 1,
            planes: vec![base.clone(), other.clone()]
        })
        .unwrap_err(),
        CompositionError::PlaneOrder
    );
    other.z = 1;
    other.id = 1;
    assert_eq!(
        compose(&Scanout {
            width: 1,
            height: 1,
            planes: vec![base.clone(), other]
        })
        .unwrap_err(),
        CompositionError::PlaneOrder
    );
    let mut wrong = base.clone();
    wrong.destination.x = 1;
    assert_eq!(
        compose(&Scanout {
            width: 1,
            height: 1,
            planes: vec![wrong]
        })
        .unwrap_err(),
        CompositionError::PrimaryCoverage
    );
    assert_eq!(
        compose(&Scanout {
            width: 1,
            height: 1,
            planes: vec![base; 33]
        })
        .unwrap_err(),
        CompositionError::TooManyPlanes
    );
}

#[test]
fn total_source_budget_counts_borrowed_aliases_before_output_allocation() {
    let pixel = [0, 0, 0, 255];
    let buffer = vec![0; 8 * 1024 * 1024];
    let mut planes = vec![plane(&pixel, 1, 1)];
    for id in 2..=17 {
        let mut p = plane(&buffer, 1024, 2048);
        p.id = id;
        p.z = id as i32;
        p.kind = Kind::Overlay;
        p.crop.width = 1;
        p.crop.height = 1;
        p.destination.width = 1;
        p.destination.height = 1;
        planes.push(p);
    }
    assert_eq!(
        compose(&Scanout {
            width: 1,
            height: 1,
            planes
        })
        .unwrap_err(),
        CompositionError::PixelBudget
    );
}

#[test]
fn invalid_premultiplied_visible_pixel_is_rejected() {
    let base = [0, 0, 0, 255];
    let invalid = [200, 0, 0, 100];
    let mut overlay = plane(&invalid, 1, 1);
    overlay.id = 2;
    overlay.z = 1;
    overlay.kind = Kind::Overlay;
    overlay.blend = Blend::Premultiplied;
    assert_eq!(
        compose(&Scanout {
            width: 1,
            height: 1,
            planes: vec![plane(&base, 1, 1), overlay]
        })
        .unwrap_err(),
        CompositionError::InvalidPremultipliedPixel
    );
}
