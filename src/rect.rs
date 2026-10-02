//! Bounding rectangle and 2D layout primitives for UI controls.

/// 2D axis-aligned bounding box for UI control placement.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

/// Horizontal alignment options for text and control contents.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum TextAlign {
    #[default]
    Left,
    Center,
    Right,
}

impl Rect {
    /// Create a new rectangle from position `(x, y)` and size `(w, h)`.
    #[inline]
    pub const fn new(x: f32, y: f32, w: f32, h: f32) -> Self {
        Self { x, y, w, h }
    }

    /// Top-left position `[x, y]`.
    #[inline]
    pub const fn pos(&self) -> [f32; 2] {
        [self.x, self.y]
    }

    /// Extent `[w, h]`.
    #[inline]
    pub const fn size(&self) -> [f32; 2] {
        [self.w, self.h]
    }

    /// Right edge `x + w`.
    #[inline]
    pub fn right(&self) -> f32 {
        self.x + self.w
    }

    /// Bottom edge `y + h`.
    #[inline]
    pub fn bottom(&self) -> f32 {
        self.y + self.h
    }

    /// Center point `[x + w/2, y + h/2]`.
    #[inline]
    pub fn center(&self) -> [f32; 2] {
        [self.x + self.w * 0.5, self.y + self.h * 0.5]
    }

    /// Expand (or contract if negative) all edges by `padding`.
    #[inline]
    pub fn pad(&self, padding: f32) -> Self {
        Self {
            x: self.x - padding,
            y: self.y - padding,
            w: (self.w + padding * 2.0).max(0.0),
            h: (self.h + padding * 2.0).max(0.0),
        }
    }

    /// Pad with asymmetric horizontal and vertical amounts.
    #[inline]
    pub fn pad_xy(&self, pad_x: f32, pad_y: f32) -> Self {
        Self {
            x: self.x - pad_x,
            y: self.y - pad_y,
            w: (self.w + pad_x * 2.0).max(0.0),
            h: (self.h + pad_y * 2.0).max(0.0),
        }
    }

    /// Split horizontally into two sub-rectangles by `left_width`.
    pub fn split_h(&self, left_width: f32, gap: f32) -> (Self, Self) {
        let actual_left = left_width.min(self.w);
        let right_x = self.x + actual_left + gap;
        let right_w = (self.w - actual_left - gap).max(0.0);
        (
            Self::new(self.x, self.y, actual_left, self.h),
            Self::new(right_x, self.y, right_w, self.h),
        )
    }

    /// Split horizontally by a normalized fraction `t \in [0, 1]`.
    pub fn split_fraction(&self, frac: f32, gap: f32) -> (Self, Self) {
        let left_w = (self.w - gap).max(0.0) * frac.clamp(0.0, 1.0);
        self.split_h(left_w, gap)
    }

    /// Check if point `[x, y]` lies inside this rectangle (inclusive of boundaries).
    #[inline]
    pub fn contains(&self, pt: [f32; 2]) -> bool {
        pt[0] >= self.x && pt[0] <= self.right() && pt[1] >= self.y && pt[1] <= self.bottom()
    }

    /// Calculate the normalized horizontal position `[0.0, 1.0]` of `x` across this rectangle's
    /// width.
    #[inline]
    pub fn normalized_x(&self, x: f32) -> f32 {
        if self.w <= 0.0 {
            0.0
        } else {
            ((x - self.x) / self.w).clamp(0.0, 1.0)
        }
    }
}
