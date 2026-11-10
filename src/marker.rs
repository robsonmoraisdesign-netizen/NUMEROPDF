//! Conservative candidate detection for purple outlined squares with a purple X.
//! Raster candidates must be reviewed; colour alone is never sufficient.
use crate::Area;
use image::RgbaImage;
use std::collections::VecDeque;
fn purple(p: &image::Rgba<u8>) -> bool {
    let [r, g, b, a] = p.0;
    a > 180 && r > 70 && b > 100 && (r as i16 - g as i16) > 35 && (b as i16 - g as i16) > 55
}
pub fn find(img: &RgbaImage) -> Vec<Area> {
    let (w, h) = img.dimensions();
    let Some(len) = (w as usize)
        .checked_mul(h as usize)
        .filter(|n| *n <= 32_000_000)
    else {
        return vec![];
    };
    let mut seen = vec![false; len];
    let mut results = Vec::new();
    for y in 0..h {
        for x in 0..w {
            let i = (y * w + x) as usize;
            if seen[i] || !purple(img.get_pixel(x, y)) {
                continue;
            }
            let mut q = VecDeque::from([(x, y)]);
            seen[i] = true;
            let (mut x0, mut y0, mut x1, mut y1) = (x, y, x, y);
            let mut count = 0;
            while let Some((a, b)) = q.pop_front() {
                count += 1;
                x0 = x0.min(a);
                y0 = y0.min(b);
                x1 = x1.max(a);
                y1 = y1.max(b);
                for (dx, dy) in [
                    (-1, 0),
                    (1, 0),
                    (0, -1),
                    (0, 1),
                    (-1, -1),
                    (1, 1),
                    (-1, 1),
                    (1, -1),
                ] {
                    let (nx, ny) = (a as i64 + dx, b as i64 + dy);
                    if nx < 0 || ny < 0 || nx >= w as i64 || ny >= h as i64 {
                        continue;
                    }
                    let (nx, ny) = (nx as u32, ny as u32);
                    let j = (ny * w + nx) as usize;
                    if !seen[j] && purple(img.get_pixel(nx, ny)) {
                        seen[j] = true;
                        q.push_back((nx, ny));
                    }
                }
            }
            let (bw, bh) = (x1 - x0 + 1, y1 - y0 + 1);
            if bw < 12 || bh < 12 || bw > 600 || bh > 600 {
                continue;
            }
            let ratio = bw as f64 / bh as f64;
            let density = count as f64 / (bw as f64 * bh as f64);
            if !(0.8..=1.25).contains(&ratio) || !(0.08..=0.65).contains(&density) {
                continue;
            }
            let near = |px: u32, py: u32| -> bool {
                let radius = (bw / 25).max(1);
                (py.saturating_sub(radius)..=(py + radius).min(h - 1)).any(|yy| {
                    (px.saturating_sub(radius)..=(px + radius).min(w - 1))
                        .any(|xx| purple(img.get_pixel(xx, yy)))
                })
            };
            let mut hits = 0;
            for k in 1..10 {
                let xx = x0 + (bw - 1) * k / 10;
                let yy = y0 + (bh - 1) * k / 10;
                let yr = y1 - (bh - 1) * k / 10;
                if near(xx, yy)
                    && near(xx, yr)
                    && near(xx, y0)
                    && near(xx, y1)
                    && near(x0, yy)
                    && near(x1, yy)
                {
                    hits += 1;
                }
            }
            if hits >= 7 {
                results.push(Area {
                    x: x0 as f64,
                    y: y0 as f64,
                    width: bw as f64,
                    height: bh as f64,
                });
            }
            if results.len() >= 100 {
                return results;
            }
        }
    }
    results
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn square_requires_cross() {
        let mut im = RgbaImage::from_pixel(100, 100, image::Rgba([255; 4]));
        let ink = image::Rgba([150, 30, 200, 255]);
        for i in 20..=60 {
            im.put_pixel(i, 20, ink);
            im.put_pixel(i, 60, ink);
            im.put_pixel(20, i, ink);
            im.put_pixel(60, i, ink);
        }
        assert!(find(&im).is_empty());
        for i in 20..=60 {
            im.put_pixel(i, i, ink);
            im.put_pixel(i, 80 - i, ink);
        }
        assert_eq!(find(&im).len(), 1);
    }
    #[test]
    fn solid_square_is_not_marker() {
        let im = RgbaImage::from_pixel(40, 40, image::Rgba([150, 30, 200, 255]));
        assert!(find(&im).is_empty());
    }
}
