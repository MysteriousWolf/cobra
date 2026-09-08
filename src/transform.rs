//! Affine transforms: where a shape drawn in its own coordinates lands on the canvas.

use crate::Point;

/// An affine transform of dot coordinates: a translation, scale, flip, rotation, or
/// any combination, applied to a [`Path`](crate::Path), a [`Mask`](crate::Mask), or
/// to everything drawn inside [`Canvas::with`](crate::Canvas::with).
///
/// A chain of builder methods reads outer to inner, the way a shape is described:
/// `Transform::at(20.0, 8.0).scale(2.0, 2.0).flip_x()` is a shape flipped, then
/// scaled by two, then placed with its origin at `(20, 8)`. Each method adds a step
/// that happens *before* the ones already there. [`then`](Self::then) is the other
/// way round: `a.then(&b)` applies `a` first.
///
/// ```
/// use cobra::{Canvas, Rgb, Transform};
///
/// let mut canvas = Canvas::new(20, 5);
/// // A limb drawn about its own origin, placed at (30, 10), facing left.
/// canvas.with(Transform::at(30.0, 10.0).flip_x(), |c| {
///     c.fill_ellipse(4.0, 0.0, 6.0, 3.0, Rgb::hex(0xffa657));
/// });
/// assert!(canvas.get(22, 10).is_some() && canvas.get(38, 10).is_none());
/// ```
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Transform {
    /// The matrix `[a c e; b d f]`: `x' = a·x + c·y + e`, `y' = b·x + d·y + f`.
    pub m: [f32; 6],
}

impl Default for Transform {
    fn default() -> Self {
        Self::IDENTITY
    }
}

impl Transform {
    /// Leaves everything where it is.
    pub const IDENTITY: Self = Self { m: [1.0, 0.0, 0.0, 1.0, 0.0, 0.0] };

    /// A move by `(dx, dy)` dots; the usual start of a chain, so it is named for
    /// where the local origin ends up.
    pub const fn at(dx: f32, dy: f32) -> Self {
        Self { m: [1.0, 0.0, 0.0, 1.0, dx, dy] }
    }

    /// Whether this is the identity.
    #[inline]
    pub fn is_identity(&self) -> bool {
        self.m == Self::IDENTITY.m
    }

    /// Whether axes stay axes: no rotation or shear, so a rectangle maps to a
    /// rectangle and an ellipse to an ellipse (possibly flipped).
    #[inline]
    pub fn is_axis_aligned(&self) -> bool {
        self.m[1] == 0.0 && self.m[2] == 0.0
    }

    /// How much lengths grow on average: the square root of the area scale.
    #[inline]
    pub fn scale_factor(&self) -> f32 {
        (self.m[0] * self.m[3] - self.m[1] * self.m[2]).abs().sqrt()
    }

    /// Where `p` lands.
    #[inline]
    pub fn apply(&self, p: Point) -> Point {
        let [a, b, c, d, e, f] = self.m;
        (a * p.0 + c * p.1 + e, b * p.0 + d * p.1 + f)
    }

    /// The transform that applies `self`, then `next`.
    pub fn then(&self, next: &Transform) -> Self {
        let [a, b, c, d, e, f] = self.m;
        let [p, q, r, s, t, u] = next.m;
        Self { m: [p * a + r * b, q * a + s * b, p * c + r * d, q * c + s * d, p * e + r * f + t, q * e + s * f + u] }
    }

    /// The inverse, `None` when the transform flattens the plane.
    pub fn inverse(&self) -> Option<Self> {
        let [a, b, c, d, e, f] = self.m;
        let det = a * d - b * c;
        if det.abs() < 1e-12 {
            return None;
        }
        let (ia, ib, ic, id) = (d / det, -b / det, -c / det, a / det);
        Some(Self { m: [ia, ib, ic, id, -(ia * e + ic * f), -(ib * e + id * f)] })
    }

    /// A move by `(dx, dy)`, before everything so far.
    pub fn translate(self, dx: f32, dy: f32) -> Self {
        Self::at(dx, dy).then(&self)
    }

    /// A scale about the origin, before everything so far.
    pub fn scale(self, sx: f32, sy: f32) -> Self {
        Self { m: [sx, 0.0, 0.0, sy, 0.0, 0.0] }.then(&self)
    }

    /// A left-to-right mirror about the origin (`x` becomes `-x`), before
    /// everything so far.
    pub fn flip_x(self) -> Self {
        self.scale(-1.0, 1.0)
    }

    /// A top-to-bottom mirror about the origin, before everything so far.
    pub fn flip_y(self) -> Self {
        self.scale(1.0, -1.0)
    }

    /// A rotation by `angle` radians about the origin (clockwise on screen, since
    /// `y` grows downwards), before everything so far.
    pub fn rotate(self, angle: f32) -> Self {
        let (s, c) = angle.sin_cos();
        Self { m: [c, s, -s, c, 0.0, 0.0] }.then(&self)
    }

    /// A rotation by `angle` radians about `center`, before everything so far.
    pub fn rotate_about(self, angle: f32, center: Point) -> Self {
        self.translate(center.0, center.1).rotate(angle).translate(-center.0, -center.1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn near(a: Point, b: Point) -> bool {
        (a.0 - b.0).abs() < 1e-4 && (a.1 - b.1).abs() < 1e-4
    }

    #[test]
    fn composes_in_reading_order() {
        let t = Transform::at(10.0, 5.0).scale(2.0, 2.0);
        assert!(near(t.apply((1.0, 1.0)), (12.0, 7.0)), "scaled first, then placed");
        let t = Transform::IDENTITY.scale(2.0, 2.0).translate(10.0, 5.0);
        assert!(near(t.apply((1.0, 1.0)), (22.0, 12.0)), "moved first, then scaled");
        assert!(near(Transform::at(3.0, 0.0).flip_x().apply((1.0, 2.0)), (2.0, 2.0)));
        assert!(near(Transform::at(1.0, 0.0).then(&Transform::at(0.0, 1.0)).apply((0.0, 0.0)), (1.0, 1.0)));
        let r = Transform::IDENTITY.rotate(std::f32::consts::FRAC_PI_2);
        assert!(near(r.apply((1.0, 0.0)), (0.0, 1.0)), "clockwise on screen");
        assert!(!r.is_axis_aligned() && Transform::at(1.0, 2.0).flip_y().is_axis_aligned());
        let about = Transform::IDENTITY.rotate_about(std::f32::consts::PI, (5.0, 5.0));
        assert!(near(about.apply((6.0, 5.0)), (4.0, 5.0)));
    }

    #[test]
    fn inverse_and_scale_factor() {
        let t = Transform::at(3.0, -2.0).scale(2.0, 3.0).rotate(0.7);
        let inv = t.inverse().unwrap();
        assert!(near(inv.apply(t.apply((4.0, 9.0))), (4.0, 9.0)));
        assert!((t.scale_factor() - 6f32.sqrt()).abs() < 1e-4);
        assert!(Transform::IDENTITY.scale(0.0, 1.0).inverse().is_none());
        assert!(Transform::default().is_identity());
    }
}
