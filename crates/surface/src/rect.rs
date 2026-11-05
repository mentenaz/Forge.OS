//! Rectangles in surface coordinates.

/// A rectangle whose corner may lie off the surface on any side.
///
/// The origin is signed so a window can be dragged partly off screen; the size
/// is unsigned because a rectangle cannot have a negative extent. All edge
/// arithmetic is done in 64 bits, so no rectangle can overflow.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rect {
    /// Left edge.
    pub x: i32,
    /// Top edge.
    pub y: i32,
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
}

impl Rect {
    /// The rectangle with no area, which is what two disjoint rectangles meet in.
    pub const EMPTY: Self = Self::new(0, 0, 0, 0);

    /// A rectangle with its top left corner at `x`, `y`.
    pub const fn new(x: i32, y: i32, width: u32, height: u32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    /// The rectangle between four edges, or [`Rect::EMPTY`] if they enclose nothing.
    ///
    /// Edges beyond the `i32` range are pulled in to it. Nothing that far out
    /// is on any surface, so the part that matters is unchanged.
    pub const fn from_edges(left: i64, top: i64, right: i64, bottom: i64) -> Self {
        let left = clamp(left);
        let top = clamp(top);
        let right = clamp(right);
        let bottom = clamp(bottom);
        if right <= left || bottom <= top {
            return Self::EMPTY;
        }
        Self::new(
            left as i32,
            top as i32,
            (right - left) as u32,
            (bottom - top) as u32,
        )
    }

    /// Whether the rectangle covers no pixel.
    pub const fn is_empty(self) -> bool {
        self.width == 0 || self.height == 0
    }

    /// The first column to the right of the rectangle.
    pub const fn right(self) -> i64 {
        self.x as i64 + self.width as i64
    }

    /// The first row below the rectangle.
    pub const fn bottom(self) -> i64 {
        self.y as i64 + self.height as i64
    }

    /// Whether the pixel at `x`, `y` is inside the rectangle.
    pub const fn contains(self, x: i32, y: i32) -> bool {
        x as i64 >= self.x as i64
            && (x as i64) < self.right()
            && y as i64 >= self.y as i64
            && (y as i64) < self.bottom()
    }

    /// The smallest rectangle holding both. An empty rectangle holds nothing,
    /// so it adds nothing: the union with one is the other.
    pub const fn union(self, other: Self) -> Self {
        if self.is_empty() {
            return other;
        }
        if other.is_empty() {
            return self;
        }
        Self::from_edges(
            min(self.x as i64, other.x as i64),
            min(self.y as i64, other.y as i64),
            max(self.right(), other.right()),
            max(self.bottom(), other.bottom()),
        )
    }

    /// The pixels in both rectangles, or [`Rect::EMPTY`] if there are none.
    pub const fn intersect(self, other: Self) -> Self {
        Self::from_edges(
            max(self.x as i64, other.x as i64),
            max(self.y as i64, other.y as i64),
            min(self.right(), other.right()),
            min(self.bottom(), other.bottom()),
        )
    }
}

const fn clamp(value: i64) -> i64 {
    if value < i32::MIN as i64 {
        i32::MIN as i64
    } else if value > i32::MAX as i64 {
        i32::MAX as i64
    } else {
        value
    }
}

const fn max(a: i64, b: i64) -> i64 {
    if a > b { a } else { b }
}

const fn min(a: i64, b: i64) -> i64 {
    if a < b { a } else { b }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn edges_are_the_origin_plus_the_size_even_at_the_extremes() {
        let rect = Rect::new(i32::MAX, i32::MIN, u32::MAX, u32::MAX);
        assert_eq!(rect.right(), i32::MAX as i64 + u32::MAX as i64);
        assert_eq!(rect.bottom(), i32::MIN as i64 + u32::MAX as i64);
    }

    #[test]
    fn a_rectangle_with_no_width_or_no_height_is_empty() {
        assert!(Rect::new(0, 0, 0, 5).is_empty());
        assert!(Rect::new(0, 0, 5, 0).is_empty());
        assert!(!Rect::new(0, 0, 1, 1).is_empty());
    }

    #[test]
    fn contains_includes_the_first_edge_and_excludes_the_last() {
        let rect = Rect::new(2, 3, 4, 5);
        assert!(rect.contains(2, 3));
        assert!(rect.contains(5, 7));
        assert!(!rect.contains(6, 7));
        assert!(!rect.contains(5, 8));
        assert!(!rect.contains(1, 3));
        assert!(!rect.contains(2, 2));
    }

    #[test]
    fn intersection_is_exactly_the_pixels_in_both_for_every_small_pair() {
        let mut checked = 0;
        for ax in -3..3 {
            for ay in -3..3 {
                for aw in 0..4 {
                    for ah in 0..4 {
                        for bx in -3..3 {
                            for by in -3..3 {
                                for bw in 0..4 {
                                    for bh in 0..4 {
                                        let a = Rect::new(ax, ay, aw, ah);
                                        let b = Rect::new(bx, by, bw, bh);
                                        let both = a.intersect(b);
                                        for x in -6..8 {
                                            for y in -6..8 {
                                                assert_eq!(
                                                    both.contains(x, y),
                                                    a.contains(x, y) && b.contains(x, y),
                                                    "{a:?} {b:?} at {x},{y}"
                                                );
                                            }
                                        }
                                        assert_eq!(both, b.intersect(a));
                                        checked += 1;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        assert_eq!(checked, 6 * 6 * 4 * 4 * 6 * 6 * 4 * 4);
    }

    #[test]
    fn disjoint_rectangles_meet_in_the_empty_rectangle() {
        let a = Rect::new(0, 0, 2, 2);
        let b = Rect::new(2, 0, 2, 2);
        assert_eq!(a.intersect(b), Rect::EMPTY);
    }

    #[test]
    fn intersecting_the_largest_rectangles_does_not_overflow() {
        let huge = Rect::new(i32::MIN, i32::MIN, u32::MAX, u32::MAX);
        let small = Rect::new(-1, -1, 3, 3);
        assert_eq!(huge.intersect(small), small);
        assert_eq!(huge.intersect(huge), huge);
    }

    #[test]
    fn edges_beyond_the_i32_range_are_pulled_in() {
        let rect = Rect::from_edges(-10_000_000_000, -5, 10_000_000_000, 5);
        assert_eq!(rect, Rect::new(i32::MIN, -5, u32::MAX, 10));
        assert_eq!(Rect::from_edges(3, 3, 3, 9), Rect::EMPTY);
        assert_eq!(Rect::from_edges(3, 3, 9, 3), Rect::EMPTY);
        assert_eq!(Rect::from_edges(3, 5, 9, 2), Rect::EMPTY);
        assert_eq!(Rect::from_edges(5, 0, 2, 9), Rect::EMPTY);
    }

    #[test]
    fn the_union_is_the_bounding_box_of_two_rectangles() {
        let a = Rect::new(0, 0, 2, 2);
        let b = Rect::new(5, 3, 2, 4);
        assert_eq!(a.union(b), Rect::new(0, 0, 7, 7));
        assert_eq!(b.union(a), a.union(b));
        assert_eq!(a.union(a), a);
        let inner = Rect::new(1, 1, 1, 1);
        assert_eq!(a.union(inner), a, "a rectangle inside another adds nothing");
        assert_eq!(
            Rect::new(-4, -4, 2, 2).union(Rect::new(1, 1, 2, 2)),
            Rect::new(-4, -4, 7, 7)
        );
    }

    #[test]
    fn an_empty_rectangle_adds_nothing_to_a_union_wherever_it_is() {
        let a = Rect::new(3, 3, 2, 2);
        assert_eq!(a.union(Rect::new(100, 100, 0, 5)), a);
        assert_eq!(Rect::new(-100, -100, 5, 0).union(a), a);
        assert_eq!(Rect::EMPTY.union(Rect::EMPTY), Rect::EMPTY);
    }

    #[test]
    fn every_pixel_of_either_rectangle_is_in_the_union_for_every_small_pair() {
        for ax in -2..3 {
            for ay in -2..3 {
                for aw in 0..3 {
                    for ah in 0..3 {
                        for bx in -2..3 {
                            for by in -2..3 {
                                for bw in 0..3 {
                                    for bh in 0..3 {
                                        let a = Rect::new(ax, ay, aw, ah);
                                        let b = Rect::new(bx, by, bw, bh);
                                        let both = a.union(b);
                                        for x in -4..6 {
                                            for y in -4..6 {
                                                if a.contains(x, y) || b.contains(x, y) {
                                                    assert!(both.contains(x, y), "{a:?} {b:?}");
                                                }
                                            }
                                        }
                                        let area =
                                            |r: Rect| i64::from(r.width) * i64::from(r.height);
                                        assert!(area(both) >= area(a).max(area(b)));
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn the_union_of_the_largest_rectangles_does_not_overflow() {
        let low = Rect::new(i32::MIN, i32::MIN, 1, 1);
        let high = Rect::new(i32::MAX, i32::MAX, 1, 1);
        let both = low.union(high);
        assert_eq!(both.x, i32::MIN);
        assert_eq!(both.y, i32::MIN);
        assert_eq!(
            both.width,
            u32::MAX,
            "pulled in to what a rectangle can hold"
        );
    }
}
