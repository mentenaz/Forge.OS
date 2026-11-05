//! The display against a pixel-by-pixel model.

mod support;

use platform::{Cell, Display, Size};
use shell_surface::{NoSink, SurfaceDisplay};
use support::*;
use surface::Rect;

const W: usize = 26;
const H: usize = 20;
const AREA: Rect = Rect::new(3, 2, 20, 15);

fn grid() -> Size {
    Size::new(5, 3)
}

#[test]
fn the_grid_is_as_many_whole_cells_as_the_area_holds() {
    let mut words = buffer(W, H);
    let mut surface = surface(&mut words, W, H);
    let display = SurfaceDisplay::new(&mut surface, &Odd, CELL_H, AREA, PALETTE, NoSink);
    assert_eq!(display.size(), grid());
    assert_eq!(display.area(), AREA);

    let mut surface = surface_again(&mut words);
    for (area, size) in [
        (Rect::new(0, 0, 3, 20), Size::new(0, 4)),
        (Rect::new(0, 0, 20, 4), Size::new(5, 0)),
        (Rect::new(0, 0, 4, 5), Size::new(1, 1)),
        (Rect::new(0, 0, 0, 0), Size::new(0, 0)),
        (Rect::new(0, 0, 8, 10), Size::new(2, 2)),
    ] {
        let display = SurfaceDisplay::new(&mut surface, &Odd, CELL_H, area, PALETTE, NoSink);
        assert_eq!(display.size(), size, "{area:?}");
    }
}

fn surface_again(words: &mut [u32]) -> surface::Surface<'_> {
    surface(words, W, H)
}

#[test]
fn every_cell_looks_like_the_model_for_every_glyph_size_and_style() {
    let mut checked = 0;
    for col in 0..grid().cols {
        for row in 0..grid().rows {
            for ch in ['I', 'h', 'B', 'T', 'M', ' ', '?'] {
                for (bold, inverse) in [(false, false), (true, false), (false, true), (true, true)]
                {
                    let cell = styled(ch, bold, inverse);
                    let mut words = buffer(W, H);
                    let want = expected(
                        &words,
                        (W, H),
                        AREA,
                        Rect::new(0, 0, W as u32, H as u32),
                        &Odd,
                        &[(col, row, cell)],
                    );
                    let mut surface = surface(&mut words, W, H);
                    let mut display =
                        SurfaceDisplay::new(&mut surface, &Odd, CELL_H, AREA, PALETTE, NoSink);
                    display.put(col, row, cell);
                    display.present();
                    assert_eq!(
                        words, want,
                        "{ch:?} bold {bold} inverse {inverse} at {col},{row}"
                    );
                    checked += 1;
                }
            }
        }
    }
    assert_eq!(checked, 15 * 7 * 4);
}

#[test]
fn a_glyph_bigger_than_its_cell_does_not_reach_the_next_cell() {
    let mut words = buffer(W, H);
    let mut surface = surface(&mut words, W, H);
    let mut display = SurfaceDisplay::new(&mut surface, &Odd, CELL_H, AREA, PALETTE, NoSink);
    display.put(1, 1, plain('M'));
    display.put(1, 1, plain('T'));
    // The tall 'T' is seven rows in a five row cell, the wide 'M' six columns in four.
    for y in 0..H as i32 {
        for x in 0..W as i32 {
            let cell = Rect::new(AREA.x + 4, AREA.y + 5, 4, 5);
            if !cell.contains(x, y) {
                assert_eq!(surface.word(x as usize, y as usize), Some(BASE), "{x},{y}");
            }
        }
    }
}

#[test]
fn a_whole_grid_of_random_cells_matches_the_model() {
    let mut rng = Lcg(99);
    for round in 0..40 {
        let mut cells = Vec::new();
        for _ in 0..30 {
            let col = (rng.bits() % 5) as u16;
            let row = (rng.bits() % 3) as u16;
            let ch = ['I', 'h', 'B', 'T', 'M', ' ', '?'][(rng.bits() % 7) as usize];
            cells.push((
                col,
                row,
                styled(
                    ch,
                    rng.bits().is_multiple_of(2),
                    rng.bits().is_multiple_of(2),
                ),
            ));
        }
        let mut words = buffer(W, H);
        let want = expected(
            &words,
            (W, H),
            AREA,
            Rect::new(0, 0, W as u32, H as u32),
            &Odd,
            &cells,
        );
        let mut surface = surface(&mut words, W, H);
        let mut display = SurfaceDisplay::new(&mut surface, &Odd, CELL_H, AREA, PALETTE, NoSink);
        for (col, row, cell) in &cells {
            display.put(*col, *row, *cell);
        }
        display.present();
        assert_eq!(words, want, "round {round}");
    }
}

#[test]
fn a_cell_outside_the_grid_is_ignored_and_presents_nothing() {
    // Areas with leftover pixels, where a cell just past the grid would be
    // partly visible if it were drawn.
    for area in [AREA, Rect::new(3, 2, 22, 17), Rect::new(3, 2, 23, 19)] {
        let mut words = buffer(W, H);
        let before = words.clone();
        let mut surface = surface(&mut words, W, H);
        let mut display = SurfaceDisplay::new(
            &mut surface,
            &Odd,
            CELL_H,
            area,
            PALETTE,
            Recorder::default(),
        );
        let Size { cols, rows } = display.size();
        for (col, row) in [
            (cols, 0),
            (0, rows),
            (cols, rows),
            (cols, rows - 1),
            (cols - 1 + 1, 1),
            (u16::MAX, 0),
            (0, u16::MAX),
            (u16::MAX, u16::MAX),
        ] {
            display.put(col, row, plain('B'));
        }
        display.present();
        assert!(display.sink().calls.is_empty(), "{area:?}");
        assert_eq!(words, before, "{area:?}");
    }
}

#[test]
fn an_area_partly_off_the_surface_is_cut_at_the_surface() {
    for area in [
        Rect::new(-6, -4, 20, 15),
        Rect::new(14, 10, 20, 15),
        Rect::new(-100, -100, 400, 400),
        Rect::new(5, 5, 60, 4),
    ] {
        let mut display_words = buffer(W, H);
        let mut probe = buffer(W, H);
        let size = {
            let mut surface = surface(&mut probe, W, H);
            SurfaceDisplay::new(&mut surface, &Odd, CELL_H, area, PALETTE, NoSink).size()
        };
        let mut cells = Vec::new();
        for row in 0..size.rows.min(40) {
            for col in 0..size.cols.min(40) {
                cells.push((
                    col,
                    row,
                    styled(
                        if (col + row) % 2 == 0 { 'B' } else { 'I' },
                        false,
                        row % 2 == 0,
                    ),
                ));
            }
        }
        let want = expected(
            &display_words,
            (W, H),
            area,
            Rect::new(0, 0, W as u32, H as u32),
            &Odd,
            &cells,
        );
        let mut surface = surface(&mut display_words, W, H);
        let mut display = SurfaceDisplay::new(
            &mut surface,
            &Odd,
            CELL_H,
            area,
            PALETTE,
            Recorder::default(),
        );
        for (col, row, cell) in &cells {
            display.put(*col, *row, *cell);
        }
        display.present();
        let calls = display.sink().calls.clone();
        assert_eq!(display_words, want, "{area:?}");
        let bounds = Rect::new(0, 0, W as u32, H as u32);
        for (dirty, _) in calls {
            assert_eq!(
                dirty.intersect(bounds),
                dirty,
                "{area:?}: dirty rectangle left the surface"
            );
        }
    }
}

#[test]
fn the_surface_clip_is_respected_and_given_back() {
    let clip = Rect::new(1, 1, 14, 10);
    let mut words = buffer(W, H);
    let mut cells = Vec::new();
    for row in 0..3 {
        for col in 0..5 {
            cells.push((col, row, styled('B', false, false)));
        }
    }
    let want = expected(&words, (W, H), AREA, clip, &Odd, &cells);
    let mut surface = surface(&mut words, W, H);
    surface.set_clip(clip);
    let mut display = SurfaceDisplay::new(&mut surface, &Odd, CELL_H, AREA, PALETTE, NoSink);
    for (col, row, cell) in &cells {
        display.put(*col, *row, *cell);
    }
    display.clear();
    for (col, row, cell) in &cells {
        display.put(*col, *row, *cell);
    }
    assert_eq!(surface.clip(), clip, "the display gives the clip back");
    assert_eq!(words, want);
}

#[test]
fn clear_paints_the_area_and_nothing_else() {
    let mut words = buffer(W, H);
    let mut surface = surface(&mut words, W, H);
    let mut display = SurfaceDisplay::new(
        &mut surface,
        &Odd,
        CELL_H,
        AREA,
        PALETTE,
        Recorder::default(),
    );
    display.put(0, 0, plain('B'));
    display.clear();
    display.present();
    let calls = display.sink().calls.clone();
    assert_eq!(calls.len(), 1, "one present for everything since the last");
    assert_eq!(calls[0].0, AREA);
    for y in 0..H as i32 {
        for x in 0..W as i32 {
            let want = if AREA.contains(x, y) {
                0x0010_1820
            } else {
                BASE
            };
            assert_eq!(surface.word(x as usize, y as usize), Some(want), "{x},{y}");
        }
    }
    for y in 0..H {
        assert_eq!(words[y * (W + 2) + W], PAD);
        assert_eq!(words[y * (W + 2) + W + 1], PAD);
    }
}

#[test]
fn present_reports_the_smallest_rectangle_covering_what_changed_and_then_forgets_it() {
    let mut words = buffer(W, H);
    let mut surface = surface(&mut words, W, H);
    let mut display = SurfaceDisplay::new(
        &mut surface,
        &Odd,
        CELL_H,
        AREA,
        PALETTE,
        Recorder::default(),
    );
    display.present();
    assert!(
        display.sink().calls.is_empty(),
        "nothing written, nothing presented"
    );

    display.put(0, 0, plain('I'));
    display.put(2, 1, plain('I'));
    display.present();
    display.present();
    display.put(4, 2, Cell::BLANK);
    display.present();
    let calls = display.sink().calls.clone();

    assert_eq!(calls.len(), 2);
    // Cells (0,0) and (2,1) cover x 3..11 and y 2..12.
    assert_eq!(calls[0].0, Rect::new(3, 2, 12, 10));
    assert_eq!(
        calls[0].1,
        Some(0x0010_1820),
        "the sink sees the picture already drawn"
    );
    assert_eq!(calls[1].0, Rect::new(3 + 16, 2 + 10, 4, 5));
}
