/// Nearest closed rectangular cell around a text block, inset to protect borders.
/// If no small closed cell exists, retain the actual text bounds.
pub fn enclosing_box(text: [f64; 4], rules: &[[f64; 4]]) -> [f64; 4] {
    let [x0, y0, x1, y1] = text;
    let mut left = f64::NEG_INFINITY;
    let mut right = f64::INFINITY;
    let mut bottom = f64::NEG_INFINITY;
    let mut top = f64::INFINITY;
    for &[a, b, c, d] in rules {
        if c - a <= 2.0 && b <= y0 && d >= y1 {
            if c <= x0 {
                left = left.max(c);
            }
            if a >= x1 {
                right = right.min(a);
            }
        }
        if d - b <= 2.0 && a <= x0 && c >= x1 {
            if d <= y0 {
                bottom = bottom.max(d);
            }
            if b >= y1 {
                top = top.min(b);
            }
        }
    }
    let vertical = |x: f64| {
        rules.iter().any(|r| {
            r[2] - r[0] <= 2.0
                && (r[0] - x).abs() <= 2.0
                && r[1] <= bottom + 2.0
                && r[3] >= top - 2.0
        })
    };
    let horizontal = |y: f64| {
        rules.iter().any(|r| {
            r[3] - r[1] <= 2.0
                && (r[1] - y).abs() <= 2.0
                && r[0] <= left + 2.0
                && r[2] >= right - 2.0
        })
    };
    if right - left <= ((x1 - x0) * 5.0).max(120.0)
        && top - bottom <= ((y1 - y0) * 4.0).max(50.0)
        && vertical(left)
        && vertical(right)
        && horizontal(bottom)
        && horizontal(top)
        && right - left > 3.0
        && top - bottom > 3.0
    {
        [left + 1.5, bottom + 1.5, right - 1.5, top - 1.5]
    } else {
        text
    }
}
pub fn centered_offset(viewport: f32, page: f32) -> f32 {
    ((viewport - page) * 0.5).max(0.0)
}
pub fn wheel_zoom(zoom: f32, delta: f32) -> f32 {
    (zoom * (delta * 0.003).exp()).clamp(0.1, 8.0)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn center_and_zoom() {
        assert_eq!(centered_offset(1000.0, 600.0), 200.0);
        assert_eq!(centered_offset(500.0, 600.0), 0.0);
        assert!(wheel_zoom(1.0, 100.0) > 1.0);
        assert!((wheel_zoom(wheel_zoom(1.0, 100.0), -100.0) - 1.0).abs() < 0.001);
    }
    #[test]
    fn closed_cell_only() {
        let text = [10.0, 10.0, 30.0, 20.0];
        let rules = [
            [0.0, 0.0, 40.0, 0.0],
            [0.0, 30.0, 40.0, 30.0],
            [0.0, 0.0, 0.0, 30.0],
            [40.0, 0.0, 40.0, 30.0],
        ];
        assert_eq!(enclosing_box(text, &rules), [1.5, 1.5, 38.5, 28.5]);
        assert_eq!(enclosing_box(text, &rules[..3]), text);
    }
}
