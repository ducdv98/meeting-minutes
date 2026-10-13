//! Place a restored window inside a monitor's usable work area.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

impl Rect {
    fn right(self) -> i64 {
        i64::from(self.x) + i64::from(self.width)
    }
    fn bottom(self) -> i64 {
        i64::from(self.y) + i64::from(self.height)
    }

    fn overlap(self, other: Self) -> i64 {
        let width = (self.right().min(other.right()) - i64::from(self.x.max(other.x))).max(0);
        let height = (self.bottom().min(other.bottom()) - i64::from(self.y.max(other.y))).max(0);
        width * height
    }

    fn contains(self, other: Self) -> bool {
        self.x <= other.x
            && self.y <= other.y
            && self.right() >= other.right()
            && self.bottom() >= other.bottom()
    }
}

/// Return a corrected rectangle when no single work area contains the whole title bar.
pub fn fit_window(window: Rect, work_areas: &[Rect], primary_index: usize) -> Option<Rect> {
    let title_bar = Rect {
        height: window.height.min(32),
        ..window
    };
    if work_areas.iter().any(|area| area.contains(title_bar)) {
        return None;
    }

    let best_overlap = work_areas.iter().map(|area| window.overlap(*area)).max()?;
    let target = if best_overlap == 0 {
        work_areas.get(primary_index).unwrap_or(&work_areas[0])
    } else {
        work_areas
            .iter()
            .find(|area| window.overlap(**area) == best_overlap)?
    };
    let width = window.width.min(target.width);
    let height = window.height.min(target.height);
    let x =
        i64::from(window.x).clamp(i64::from(target.x), target.right() - i64::from(width)) as i32;
    let y =
        i64::from(window.y).clamp(i64::from(target.y), target.bottom() - i64::from(height)) as i32;
    Some(Rect {
        x,
        y,
        width,
        height,
    })
}

#[cfg(test)]
mod tests {
    use super::{fit_window, Rect};

    const PRIMARY: Rect = Rect {
        x: 0,
        y: 0,
        width: 1920,
        height: 1040,
    };

    #[test]
    fn keeps_a_window_with_its_whole_title_bar_visible() {
        let window = Rect {
            x: 100,
            y: 100,
            width: 1200,
            height: 1100,
        };
        assert_eq!(fit_window(window, &[PRIMARY], 0), None);
    }

    #[test]
    fn moves_a_window_from_a_removed_monitor_to_the_primary() {
        let window = Rect {
            x: 1900,
            y: 300,
            width: 1200,
            height: 800,
        };
        assert_eq!(
            fit_window(window, &[PRIMARY], 0),
            Some(Rect {
                x: 720,
                y: 240,
                width: 1200,
                height: 800
            })
        );
    }

    #[test]
    fn chooses_the_monitor_with_the_largest_overlap() {
        let secondary = Rect {
            x: 1920,
            y: 0,
            width: 1280,
            height: 1000,
        };
        let window = Rect {
            x: 1800,
            y: -20,
            width: 1000,
            height: 800,
        };
        assert_eq!(
            fit_window(window, &[PRIMARY, secondary], 0),
            Some(Rect {
                x: 1920,
                y: 0,
                width: 1000,
                height: 800
            })
        );
    }

    #[test]
    fn uses_the_primary_when_the_window_overlaps_no_monitor() {
        let secondary = Rect {
            x: 1920,
            y: 0,
            width: 1280,
            height: 1000,
        };
        let window = Rect {
            x: 5000,
            y: 200,
            width: 1000,
            height: 800,
        };
        assert_eq!(
            fit_window(window, &[PRIMARY, secondary], 0),
            Some(Rect {
                x: 920,
                y: 200,
                width: 1000,
                height: 800
            })
        );
    }

    #[test]
    fn shrinks_an_oversized_window_to_the_work_area() {
        let window = Rect {
            x: -500,
            y: -100,
            width: 2500,
            height: 1500,
        };
        assert_eq!(fit_window(window, &[PRIMARY], 0), Some(PRIMARY));
    }

    #[test]
    fn returns_none_without_monitors() {
        assert_eq!(fit_window(PRIMARY, &[], 0), None);
    }
}
