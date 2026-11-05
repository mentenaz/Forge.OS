//! What the surface draws, checked pixel by pixel against a plain model.

mod common;

use common::*;
use pixel::Rgb;
use surface::{Image, Rect, Surface, SurfaceError};

const W: usize = 5;
const H: usize = 4;
/// Six words per row: five pixels and one word of padding.
const PITCH: usize = 24;
const STRIDE: usize = PITCH / 4;

fn fresh() -> Vec<u32> {
    vec![SENTINEL; STRIDE * H]
}

/// Every word of the buffer that is padding, not a pixel.
fn padding_untouched(words: &[u32]) -> bool {
    (0..H).all(|y| (W..STRIDE).all(|x| words[y * STRIDE + x] == SENTINEL))
}

#[test]
fn a_buffer_too_small_for_its_layout_is_refused_and_one_word_more_is_not() {
    let mut words = vec![0; STRIDE * H - 1];
    let error = Surface::new(&mut words, xrgb(), W, H, PITCH).unwrap_err();
    assert_eq!(
        error,
        SurfaceError::BufferTooSmall {
            needed: STRIDE * H,
            have: STRIDE * H - 1
        }
    );
    let mut words = vec![0; STRIDE * H];
    assert!(Surface::new(&mut words, xrgb(), W, H, PITCH).is_ok());
}

#[test]
fn a_layout_that_is_not_a_framebuffer_is_refused() {
    let mut words = vec![0; 64];
    let layout = |width, height, pitch| {
        matches!(
            Surface::new(&mut vec![0; 64], xrgb(), width, height, pitch),
            Err(SurfaceError::Layout(_))
        )
    };
    assert!(layout(0, 4, 16));
    assert!(layout(4, 0, 16));
    assert!(layout(4, 4, 12), "a pitch shorter than a row");
    assert!(layout(4, 4, 17), "a pitch that is not whole words");
    assert!(Surface::new(&mut words, xrgb(), 4, 4, 16).is_ok());
}

#[test]
fn a_side_longer_than_a_rectangle_can_address_is_refused() {
    let side = i32::MAX as usize + 1;
    let mut words = vec![0; 4];
    assert_eq!(
        Surface::new(&mut words, xrgb(), side, 1, side * 4).unwrap_err(),
        SurfaceError::TooLarge
    );
}

#[test]
fn fill_rect_paints_exactly_the_rectangle_inside_the_clip_for_every_small_case() {
    let clips = [
        None,
        Some(Rect::new(1, 1, 3, 2)),
        Some(Rect::new(-2, -2, 4, 4)),
    ];
    let mut cases = 0;
    for clip in clips {
        for x in -2..7 {
            for y in -2..6 {
                for width in 0..8 {
                    for height in 0..7 {
                        let rect = Rect::new(x, y, width, height);
                        let mut words = fresh();
                        let mut surface = Surface::new(&mut words, xrgb(), W, H, PITCH).unwrap();
                        if let Some(clip) = clip {
                            surface.set_clip(clip);
                        }
                        let allowed = surface.clip();
                        surface.fill_rect(rect, RED);
                        for py in 0..H {
                            for px in 0..W {
                                let inside = rect.contains(px as i32, py as i32)
                                    && allowed.contains(px as i32, py as i32);
                                let expected = if inside { 0x00ff_0000 } else { SENTINEL };
                                assert_eq!(
                                    surface.word(px, py),
                                    Some(expected),
                                    "{rect:?} clip {clip:?} at {px},{py}"
                                );
                            }
                        }
                        assert!(padding_untouched(&words), "{rect:?}");
                        cases += 1;
                    }
                }
            }
        }
    }
    assert_eq!(cases, 3 * 9 * 8 * 8 * 7);
}

#[test]
fn a_clip_beyond_the_surface_is_cut_back_to_the_surface() {
    let mut words = fresh();
    let mut surface = Surface::new(&mut words, xrgb(), W, H, PITCH).unwrap();
    surface.set_clip(Rect::new(-100, -100, 1000, 1000));
    assert_eq!(surface.clip(), Rect::new(0, 0, W as u32, H as u32));
    surface.set_clip(Rect::new(10, 10, 5, 5));
    assert!(surface.clip().is_empty());
    surface.fill_rect(Rect::new(0, 0, 5, 4), RED);
    assert_eq!(surface.word(0, 0), Some(SENTINEL));
    surface.reset_clip();
    assert_eq!(surface.clip(), surface.bounds());
}

#[test]
fn clear_paints_every_pixel_and_no_padding() {
    let mut words = fresh();
    let mut surface = Surface::new(&mut words, xrgb(), W, H, PITCH).unwrap();
    surface.clear(BLUE);
    for y in 0..H {
        for x in 0..W {
            assert_eq!(surface.word(x, y), Some(0x0000_00ff));
        }
    }
    assert!(padding_untouched(&words));
}

#[test]
fn reading_off_the_surface_gives_nothing() {
    let mut words = fresh();
    let surface = Surface::new(&mut words, xrgb(), W, H, PITCH).unwrap();
    assert_eq!(surface.word(W, 0), None);
    assert_eq!(surface.word(0, H), None);
    assert_eq!(surface.pixel(W, H), None);
}

#[test]
fn the_channels_land_where_the_pixel_format_says() {
    for (format, red, green, blue) in [
        (xrgb(), 0x00ff_0000, 0x0000_ff00, 0x0000_00ff),
        (bgrx(), 0x0000_00ff, 0x0000_ff00, 0x00ff_0000),
        (rgb565(), 0xf800, 0x07e0, 0x001f),
    ] {
        let mut words = fresh();
        let mut surface = Surface::new(&mut words, format, W, H, PITCH).unwrap();
        surface.put_pixel(0, 0, RED);
        surface.put_pixel(1, 0, GREEN);
        surface.put_pixel(2, 0, BLUE);
        assert_eq!(surface.word(0, 0), Some(red));
        assert_eq!(surface.word(1, 0), Some(green));
        assert_eq!(surface.word(2, 0), Some(blue));
    }
}

#[test]
fn a_colour_written_to_an_eight_bit_surface_reads_back_exactly() {
    let mut rng = Lcg(7);
    for format in [xrgb(), bgrx()] {
        let mut words = fresh();
        let mut surface = Surface::new(&mut words, format, W, H, PITCH).unwrap();
        for _ in 0..500 {
            let colour = Rgb {
                red: rng.bits() as u8,
                green: rng.bits() as u8,
                blue: rng.bits() as u8,
            };
            surface.put_pixel(2, 2, colour);
            assert_eq!(surface.pixel(2, 2), Some(colour));
        }
    }
}

#[test]
fn a_colour_written_to_a_565_surface_reads_back_the_nearest_it_can_hold() {
    let mut words = fresh();
    let mut surface = Surface::new(&mut words, rgb565(), W, H, PITCH).unwrap();
    for colour in [BLACK, WHITE, RED, GREEN, BLUE] {
        surface.put_pixel(0, 0, colour);
        assert_eq!(surface.pixel(0, 0), Some(colour), "full scale survives");
    }
    surface.put_pixel(
        0,
        0,
        Rgb {
            red: 100,
            green: 150,
            blue: 200,
        },
    );
    let held = surface.pixel(0, 0).unwrap();
    let before = surface.word(0, 0);
    surface.put_pixel(0, 0, held);
    assert_eq!(
        surface.word(0, 0),
        before,
        "reading back and rewriting is stable"
    );
}

#[test]
fn reading_a_pixel_and_packing_it_again_gives_back_the_same_word_on_every_format() {
    let mut rng = Lcg(565);
    for format in [xrgb(), bgrx(), rgb565()] {
        let mut words = fresh();
        let mut surface = Surface::new(&mut words, format, W, H, PITCH).unwrap();
        for _ in 0..3000 {
            let colour = Rgb {
                red: rng.bits() as u8,
                green: rng.bits() as u8,
                blue: rng.bits() as u8,
            };
            surface.put_pixel(1, 1, colour);
            let word = surface.word(1, 1);
            let read = surface.pixel(1, 1).unwrap();
            surface.put_pixel(1, 1, read);
            assert_eq!(surface.word(1, 1), word, "{colour:?}");
        }
    }
}

#[test]
fn blending_a_colour_over_itself_never_changes_it_even_with_narrow_channels() {
    let mut rng = Lcg(31);
    let mut words = fresh();
    let mut surface = Surface::new(&mut words, rgb565(), W, H, PITCH).unwrap();
    for _ in 0..2000 {
        let colour = Rgb {
            red: rng.bits() as u8,
            green: rng.bits() as u8,
            blue: rng.bits() as u8,
        };
        surface.put_pixel(2, 1, colour);
        let word = surface.word(2, 1);
        let held = surface.pixel(2, 1).unwrap();
        for alpha in [1, 64, 128, 200, 254] {
            surface.blend_rect(Rect::new(2, 1, 1, 1), held, alpha);
            assert_eq!(surface.word(2, 1), word, "{colour:?} at alpha {alpha}");
        }
    }
}

#[test]
fn a_translucent_fill_is_the_blend_of_what_was_there_and_the_new_colour() {
    let under = Rgb {
        red: 100,
        green: 100,
        blue: 100,
    };
    for format in [xrgb(), bgrx()] {
        let mut words = fresh();
        let mut surface = Surface::new(&mut words, format, W, H, PITCH).unwrap();
        surface.clear(under);
        surface.blend_rect(Rect::new(1, 1, 2, 2), RED, 128);
        assert_eq!(surface.pixel(1, 1), Some(under.blend(RED, 128)));
        assert_eq!(surface.pixel(2, 2), Some(under.blend(RED, 128)));
        assert_eq!(surface.pixel(0, 0), Some(under), "outside the rectangle");
        assert_eq!(surface.pixel(3, 3), Some(under), "outside the rectangle");
    }
}

#[test]
fn alpha_zero_changes_nothing_and_alpha_full_is_a_plain_fill() {
    let mut words = fresh();
    let mut surface = Surface::new(&mut words, xrgb(), W, H, PITCH).unwrap();
    surface.blend_rect(Rect::new(0, 0, 5, 4), RED, 0);
    assert_eq!(surface.word(0, 0), Some(SENTINEL));
    surface.blend_rect(Rect::new(0, 0, 5, 4), RED, 255);
    assert_eq!(surface.word(0, 0), Some(0x00ff_0000));
    assert_eq!(surface.word(4, 3), Some(0x00ff_0000));
}

#[test]
fn stroke_rect_draws_a_ring_of_the_asked_thickness() {
    let mut words = vec![0; 7 * 7];
    let mut surface = Surface::new(&mut words, xrgb(), 7, 7, 28).unwrap();
    surface.stroke_rect(Rect::new(1, 1, 5, 5), 1, WHITE);
    let picture: Vec<String> = (0..7)
        .map(|y| {
            (0..7)
                .map(|x| {
                    if surface.word(x, y) == Some(0) {
                        '.'
                    } else {
                        '#'
                    }
                })
                .collect()
        })
        .collect();
    assert_eq!(
        picture,
        [
            ".......", //
            ".#####.", //
            ".#...#.", //
            ".#...#.", //
            ".#...#.", //
            ".#####.", //
            ".......",
        ]
    );
}

#[test]
fn a_thick_stroke_with_no_room_inside_fills_the_rectangle() {
    for thickness in [3, 4, 100] {
        let mut words = vec![0; 7 * 7];
        let mut surface = Surface::new(&mut words, xrgb(), 7, 7, 28).unwrap();
        surface.stroke_rect(Rect::new(1, 1, 5, 5), thickness, WHITE);
        assert_eq!(
            surface.word(3, 3),
            Some(0x00ff_ffff),
            "thickness {thickness}"
        );
        assert_eq!(surface.word(0, 0), Some(0));
    }
    let mut words = vec![0; 7 * 7];
    let mut surface = Surface::new(&mut words, xrgb(), 7, 7, 28).unwrap();
    surface.stroke_rect(Rect::new(1, 1, 5, 5), 2, WHITE);
    assert_eq!(
        surface.word(3, 3),
        Some(0),
        "a two pixel ring leaves the middle"
    );
    surface.stroke_rect(Rect::new(1, 1, 5, 5), 0, BLUE);
    assert_eq!(surface.word(3, 3), Some(0), "zero thickness draws nothing");
}

#[test]
fn a_stroke_partly_off_the_surface_draws_only_the_visible_edges() {
    let mut words = vec![0; 4 * 4];
    let mut surface = Surface::new(&mut words, xrgb(), 4, 4, 16).unwrap();
    surface.stroke_rect(Rect::new(-2, -2, 5, 5), 1, WHITE);
    // Only the right edge (x = 2) and the bottom edge (y = 2) are on screen.
    assert_eq!(surface.word(2, 0), Some(0x00ff_ffff));
    assert_eq!(surface.word(0, 2), Some(0x00ff_ffff));
    assert_eq!(surface.word(0, 0), Some(0));
    assert_eq!(surface.word(1, 1), Some(0));
}

#[test]
fn a_blit_copies_the_image_and_clips_at_every_edge() {
    let source = [1, 2, 3, 4, 5, 6];
    let image = Image::new(&source, 3, 2).unwrap();
    let mut words = vec![0; 4 * 4];
    let mut surface = Surface::new(&mut words, xrgb(), 4, 4, 16).unwrap();
    surface.blit(image, 1, 1);
    assert_eq!(
        [surface.word(1, 1), surface.word(2, 1), surface.word(3, 1)],
        [Some(1), Some(2), Some(3)]
    );
    assert_eq!(
        [surface.word(1, 2), surface.word(2, 2), surface.word(3, 2)],
        [Some(4), Some(5), Some(6)]
    );
    assert_eq!(surface.word(0, 1), Some(0));
    assert_eq!(surface.word(1, 3), Some(0));

    let mut words = vec![0; 4 * 4];
    let mut surface = Surface::new(&mut words, xrgb(), 4, 4, 16).unwrap();
    surface.blit(image, -1, -1);
    assert_eq!(
        surface.word(0, 0),
        Some(5),
        "the image's second column and second row"
    );
    assert_eq!(surface.word(1, 0), Some(6));
    assert_eq!(surface.word(2, 0), Some(0));
    assert_eq!(surface.word(0, 1), Some(0));

    let mut words = vec![0; 4 * 4];
    let mut surface = Surface::new(&mut words, xrgb(), 4, 4, 16).unwrap();
    surface.blit(image, 3, 3);
    assert_eq!(surface.word(3, 3), Some(1));
    surface.blit(image, 4, 0);
    surface.blit(image, 0, 4);
    surface.blit(image, -3, 0);
    surface.blit(image, 0, -2);
    assert_eq!(
        surface.word(0, 0),
        Some(0),
        "images entirely off the surface draw nothing"
    );
}

#[test]
fn a_blit_obeys_the_clip_and_leaves_padding_alone() {
    let source = [9; 12];
    let image = Image::new(&source, 4, 3).unwrap();
    let mut words = fresh();
    let mut surface = Surface::new(&mut words, xrgb(), W, H, PITCH).unwrap();
    surface.set_clip(Rect::new(1, 1, 2, 2));
    surface.blit(image, 0, 0);
    assert_eq!(surface.word(0, 0), Some(SENTINEL));
    assert_eq!(surface.word(1, 1), Some(9));
    assert_eq!(surface.word(2, 2), Some(9));
    assert_eq!(surface.word(3, 1), Some(SENTINEL));
    assert_eq!(surface.word(1, 3), Some(SENTINEL));
    assert!(padding_untouched(&words));
}

#[test]
fn an_image_must_be_exactly_as_long_as_its_size() {
    assert!(Image::new(&[0; 5], 2, 3).is_none());
    assert!(Image::new(&[0; 7], 2, 3).is_none());
    assert!(Image::new(&[0; 6], 2, 3).is_some());
    assert!(
        Image::new(&[], usize::MAX, 2).is_none(),
        "the size overflows"
    );
}

#[test]
fn text_draws_solid_ink_blends_partial_ink_and_advances_by_whole_cells() {
    let mut words = vec![0; 12 * 4];
    let mut surface = Surface::new(&mut words, xrgb(), 12, 4, 48).unwrap();
    let next = surface.draw_text(0, 0, &Tiny, "I I", WHITE);
    assert_eq!(next, 12, "three cells of four pixels");
    // 'I': the top bar is solid, the stem is only in the middle column.
    assert_eq!(surface.pixel(0, 0), Some(WHITE));
    assert_eq!(surface.pixel(1, 1), Some(WHITE));
    assert_eq!(surface.pixel(0, 1), Some(BLACK));
    // The space has no glyph, so the second 'I' starts a full two cells along.
    assert_eq!(surface.pixel(3, 0), Some(BLACK));
    assert_eq!(surface.pixel(8, 0), Some(WHITE));

    let next = surface.draw_text(0, 3, &Tiny, "h", RED);
    assert_eq!(next, 4);
    assert_eq!(surface.pixel(0, 3), Some(BLACK.blend(RED, 128)));
}

#[test]
fn text_starting_off_the_surface_still_advances_and_clips() {
    let mut words = vec![0; 8 * 4];
    let mut surface = Surface::new(&mut words, xrgb(), 8, 4, 32).unwrap();
    let next = surface.draw_text(-2, 0, &Tiny, "II", WHITE);
    assert_eq!(next, 6);
    assert_eq!(
        surface.pixel(0, 0),
        Some(WHITE),
        "the right of the first I shows"
    );
    assert_eq!(surface.pixel(1, 0), Some(BLACK), "the gap between cells");
    assert_eq!(surface.pixel(2, 0), Some(WHITE), "the second I starts at 2");
}

#[test]
fn every_row_of_a_glyph_must_be_as_wide_as_the_glyph() {
    use surface::Glyph;
    assert!(Glyph::new(2, &[&[0, 0], &[0]]).is_none());
    assert!(Glyph::new(2, &[&[0, 0, 0], &[0, 0]]).is_none());
    let glyph = Glyph::new(2, &[&[1, 2], &[3, 4]]).unwrap();
    assert_eq!((glyph.width(), glyph.height()), (2, 2));
    assert_eq!(glyph.coverage(1, 0), Some(2));
    assert_eq!(glyph.coverage(0, 1), Some(3));
    assert_eq!(glyph.coverage(2, 0), None);
    assert_eq!(glyph.coverage(0, 2), None);
    assert!(Glyph::new(5, &[]).is_some(), "no rows is an empty glyph");
}

#[test]
fn nothing_drawn_ever_lands_outside_the_clip_or_in_padding() {
    let (width, height, stride) = (16usize, 12usize, 20usize);
    let source: Vec<u32> = (1..=20).collect();
    let image = Image::new(&source, 5, 4).unwrap();
    let mut rng = Lcg(2026);
    for round in 0..4000 {
        let mut words: Vec<u32> = (0..stride * height)
            .map(|i| {
                if i % stride < width {
                    0x0001_0000 + i as u32
                } else {
                    SENTINEL
                }
            })
            .collect();
        let before = words.clone();
        let mut surface = Surface::new(&mut words, xrgb(), width, height, stride * 4).unwrap();
        let clip = Rect::new(
            rng.range(-5, 20),
            rng.range(-5, 16),
            rng.range(0, 24) as u32,
            rng.range(0, 18) as u32,
        );
        surface.set_clip(clip);
        let clip = surface.clip();
        for _ in 0..6 {
            let rect = Rect::new(
                rng.range(-10, 30),
                rng.range(-10, 24),
                rng.range(0, 30) as u32,
                rng.range(0, 24) as u32,
            );
            let colour = Rgb {
                red: rng.bits() as u8,
                green: rng.bits() as u8,
                blue: rng.bits() as u8,
            };
            match rng.bits() % 7 {
                0 => surface.fill_rect(rect, colour),
                1 => surface.blend_rect(rect, colour, rng.bits() as u8),
                2 => surface.stroke_rect(rect, rng.range(0, 6) as u32, colour),
                3 => surface.blit(image, rect.x, rect.y),
                4 => {
                    surface.draw_text(rect.x, rect.y, &Tiny, "IhI", colour);
                }
                5 => surface.put_pixel(rect.x, rect.y, colour),
                _ => surface.clear(colour),
            }
        }
        for y in 0..height {
            for x in 0..stride {
                let index = y * stride + x;
                let drawable = x < width && clip.contains(x as i32, y as i32);
                if !drawable {
                    assert_eq!(words[index], before[index], "round {round} at {x},{y}");
                }
            }
        }
    }
}

#[test]
fn a_word_below_the_last_row_is_not_on_the_surface_even_if_the_buffer_is_longer() {
    let mut words = vec![SENTINEL; STRIDE * H + STRIDE * 2];
    let surface = Surface::new(&mut words, xrgb(), W, H, PITCH).unwrap();
    assert_eq!(surface.word(0, H), None);
    assert_eq!(surface.word(W - 1, H - 1), Some(SENTINEL));
    assert_eq!(surface.pixel(0, H), None);
}

#[test]
fn put_pixel_changes_that_pixel_and_no_other() {
    for (x, y) in [(0, 0), (4, 3), (2, 1)] {
        let mut words = vec![0; W * H];
        let mut surface = Surface::new(&mut words, xrgb(), W, H, W * 4).unwrap();
        surface.put_pixel(x, y, WHITE);
        for py in 0..H {
            for px in 0..W {
                let expected = if (px as i32, py as i32) == (x, y) {
                    0x00ff_ffff
                } else {
                    0
                };
                assert_eq!(surface.word(px, py), Some(expected), "{x},{y} at {px},{py}");
            }
        }
    }
}

#[test]
fn stroke_rect_paints_exactly_the_pixels_within_the_thickness_of_an_edge() {
    let mut cases = 0;
    for x in -2..8 {
        for y in -2..8 {
            for width in 0..9u32 {
                for height in 0..9u32 {
                    for thickness in 0..6u32 {
                        let rect = Rect::new(x, y, width, height);
                        let mut words = vec![0; 6 * 6];
                        let mut surface = Surface::new(&mut words, xrgb(), 6, 6, 24).unwrap();
                        surface.stroke_rect(rect, thickness, WHITE);
                        for py in 0..6i32 {
                            for px in 0..6i32 {
                                let near_edge = (px - x) < thickness as i32
                                    || (x + width as i32 - 1 - px) < thickness as i32
                                    || (py - y) < thickness as i32
                                    || (y + height as i32 - 1 - py) < thickness as i32;
                                let painted = rect.contains(px, py) && near_edge;
                                let expected = if painted { 0x00ff_ffff } else { 0 };
                                assert_eq!(
                                    surface.word(px as usize, py as usize),
                                    Some(expected),
                                    "{rect:?} thickness {thickness} at {px},{py}"
                                );
                            }
                        }
                        cases += 1;
                    }
                }
            }
        }
    }
    assert_eq!(cases, 10 * 10 * 9 * 9 * 6);
}
