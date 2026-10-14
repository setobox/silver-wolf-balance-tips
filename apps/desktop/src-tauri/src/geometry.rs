#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Bounds {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

pub fn anchored(
    bounds: Bounds,
    width: i32,
    height: i32,
    right: f64,
    bottom: f64,
    dpi: f64,
) -> (i32, i32) {
    clamp(
        bounds,
        width,
        height,
        bounds.right - width - (right * dpi).round() as i32,
        bounds.bottom - height - (bottom * dpi).round() as i32,
    )
}

pub fn clamp(bounds: Bounds, width: i32, height: i32, x: i32, y: i32) -> (i32, i32) {
    (
        x.clamp(bounds.left, (bounds.right - width).max(bounds.left)),
        y.clamp(bounds.top, (bounds.bottom - height).max(bounds.top)),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn follows_scaled_and_negative_coordinate_monitors() {
        let host = Bounds {
            left: -1920,
            top: 0,
            right: 0,
            bottom: 1080,
        };
        assert_eq!(anchored(host, 336, 490, 16.0, 16.0, 1.5), (-360, 566));
        assert_eq!(clamp(host, 336, 490, 5000, -1000), (-336, 0));
    }
    #[test]
    fn small_host_does_not_panic() {
        let host = Bounds {
            left: 100,
            top: 50,
            right: 200,
            bottom: 150,
        };
        assert_eq!(anchored(host, 336, 490, 16.0, 16.0, 1.0), (100, 50));
    }
}
