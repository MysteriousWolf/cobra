//! Figures as things you pose: a [`Rig`] is a tree of [`Part`]s, each a
//! [`Path`] in its own coordinates hung off a parent by a [`Transform`], with named
//! points that follow the pose. Pose a rig by setting a handful of transforms,
//! then rasterise it once.
//!
//! This is the whole of rigging here and on purpose: a part is rigid, hangs off
//! one parent, and is posed by one transform. No inverse kinematics, weights,
//! skinning, timelines or easing; a pose is data, a frame is the caller's clock,
//! and `t` in [`Transform::mix`] and [`Path::mix`] is the caller's number.
//!
//! ```
//! use cobra::{Canvas, Mask, Part, Path, Rgb, Rig, Transform};
//!
//! let mut body = Path::new();
//! body.ellipse(0.0, 0.0, 8.0, 5.0);
//! let mut wing = Path::new();
//! wing.polygon(&[(0.0, 0.0), (7.0, -1.0), (9.0, 3.0), (2.0, 3.0)]);
//!
//! let mut bird = Rig::new();
//! bird.add("body", None, Part::new(body));
//! bird.add("wing", Some("body"), Part::new(wing).at(Transform::at(-2.0, -1.0)));
//! bird.mark("beak", "body", (8.0, 0.0));
//!
//! bird.place(Transform::at(20.0, 10.0));                   // the whole figure
//! bird.pose("wing", Transform::IDENTITY.rotate(-0.4));   // one joint
//! let beak = bird.point("beak");                          // follows the pose
//!
//! let mut canvas = Canvas::new(20, 5);
//! canvas.stencil(bird.mask(20, 5), Rgb::hex(0x56d364));
//! assert!((beak.0 - 28.0).abs() < 1e-3 && (beak.1 - 10.0).abs() < 1e-3);
//! ```

use crate::{Canvas, Mask, Paint, Path, Point, Transform};

/// One rigid piece of a figure: a path drawn about its own joint, and where that
/// joint sits on its parent.
#[derive(Clone, Debug, PartialEq)]
pub struct Part {
    /// The shape, in the part's own coordinates with the origin at its joint.
    pub path: Path,
    /// Where the joint sits on the parent, in the parent's coordinates: the part's
    /// resting place. A [pose](Rig::pose) happens before this, about the joint.
    pub at: Transform,
}

impl Part {
    /// A part with its joint at the parent's origin.
    pub fn new(path: Path) -> Self {
        Self { path, at: Transform::IDENTITY }
    }

    /// Sets where the joint sits on the parent (builder style).
    pub fn at(mut self, at: Transform) -> Self {
        self.at = at;
        self
    }
}

/// A part in the tree, with its pose and the raster it last produced.
#[derive(Clone, Debug)]
struct Node {
    name: String,
    parent: Option<usize>,
    part: Part,
    pose: Transform,
    /// The mask of the part at the transform it was last rasterised under, and
    /// that transform: reused while neither the part nor anything above it moves.
    raster: Option<(Transform, Mask)>,
}

/// A figure: parts in a tree, named points on them, and a pose per part. See the
/// [module docs](self).
///
/// Parts are drawn in the order they were added, so add what is behind first.
/// A rig is cheap to clone: `rig.clone()` with another pose is the same figure
/// again, which is how one description stamps out a crowd.
#[derive(Clone, Debug, Default)]
pub struct Rig {
    nodes: Vec<Node>,
    /// `(name, part, point in the part's coordinates)`.
    points: Vec<(String, usize, Point)>,
    /// Where the whole figure sits on the canvas.
    root: Transform,
    /// The union of the parts' rasters, for the size it was made at.
    whole: Option<(u16, u16, Mask)>,
}

impl Rig {
    /// An empty rig.
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds `part` as `name`, hung off `parent` (`None` for the figure itself).
    /// Names are how parts are posed and marked, so each must be unique.
    ///
    /// # Panics
    ///
    /// If `parent` names no part, or `name` is already taken: a rig is built from
    /// literals, and a typo there is a bug worth stopping on.
    pub fn add(&mut self, name: &str, parent: Option<&str>, part: Part) -> &mut Self {
        assert!(self.index(name).is_none(), "rig already has a part named {name:?}");
        let parent = parent.map(|p| self.index(p).unwrap_or_else(|| panic!("rig has no part named {p:?}")));
        self.nodes.push(Node { name: name.to_string(), parent, part, pose: Transform::IDENTITY, raster: None });
        self
    }

    /// Names point `at` (in `part`'s coordinates) `name`, so [`point`](Self::point)
    /// can find it wherever the pose puts it: where a hat sits, where a hand is,
    /// where a speech bubble's tail should aim.
    ///
    /// # Panics
    ///
    /// If `part` names no part.
    pub fn mark(&mut self, name: &str, part: &str, at: Point) -> &mut Self {
        let i = self.index(part).unwrap_or_else(|| panic!("rig has no part named {part:?}"));
        match self.points.iter_mut().find(|(n, ..)| n == name) {
            Some(entry) => *entry = (name.to_string(), i, at),
            None => self.points.push((name.to_string(), i, at)),
        }
        self
    }

    /// Poses `part`: `t` is applied about its joint, before its resting place, so
    /// `Transform::IDENTITY.rotate(0.3)` swings it and `.translate(0.0, -2.0)` lifts
    /// it. Everything hung off the part moves with it. Does nothing for an unknown
    /// name.
    pub fn pose(&mut self, part: &str, t: Transform) -> &mut Self {
        if let Some(i) = self.index(part) {
            self.nodes[i].pose = t;
            self.whole = None;
        }
        self
    }

    /// The pose of `part`, the identity when unposed or unknown.
    pub fn posed(&self, part: &str) -> Transform {
        self.index(part).map_or(Transform::IDENTITY, |i| self.nodes[i].pose)
    }

    /// Places the whole figure: where its root sits on the canvas, facing which way,
    /// at what size.
    pub fn place(&mut self, t: Transform) -> &mut Self {
        self.root = t;
        self.whole = None;
        self
    }

    /// Where the figure sits; see [`place`](Self::place).
    pub fn placed(&self) -> Transform {
        self.root
    }

    /// The part called `name`.
    pub fn part(&self, name: &str) -> Option<&Part> {
        self.index(name).map(|i| &self.nodes[i].part)
    }

    /// The part called `name`, to change. Changing a part's path or resting place
    /// is what a pose cannot do (a beak opening is two paths, [`Path::mix`]ed); the
    /// part is rasterised anew.
    pub fn part_mut(&mut self, name: &str) -> Option<&mut Part> {
        let i = self.index(name)?;
        self.nodes[i].raster = None;
        self.whole = None;
        Some(&mut self.nodes[i].part)
    }

    /// The names of the parts, in drawing order.
    pub fn parts(&self) -> impl Iterator<Item = &str> {
        self.nodes.iter().map(|n| n.name.as_str())
    }

    /// The transform from `part`'s own coordinates to the canvas, with every pose
    /// above it applied; the identity for an unknown name.
    pub fn world(&self, part: &str) -> Transform {
        self.index(part).map_or(self.root, |i| self.world_of(i))
    }

    /// Where the point named `name` is on the canvas, posed. `(0, 0)` for a name
    /// that was never [marked](Self::mark).
    pub fn point(&self, name: &str) -> Point {
        self.points.iter().find(|(n, ..)| n == name).map_or((0.0, 0.0), |&(_, i, p)| self.world_of(i).apply(p))
    }

    /// Calls `f` for every part in drawing order with its name, its path and the
    /// transform that puts the path on the canvas.
    pub fn each(&self, mut f: impl FnMut(&str, &Path, &Transform)) {
        for (i, node) in self.nodes.iter().enumerate() {
            f(&node.name, &node.part.path, &self.world_of(i));
        }
    }

    /// Fills every part with `paint`, in drawing order, through the canvas's
    /// current transform. For one silhouette to shade or outline as a whole, use
    /// [`mask`](Self::mask) and [`Canvas::stencil`] instead.
    pub fn draw(&self, canvas: &mut Canvas, paint: impl Into<Paint>) {
        let paint = paint.into();
        self.each(|_, path, world| canvas.with(*world, |c| c.fill_path(path, paint)));
    }

    /// The whole figure as one `cols × rows` mask: every part rasterised at its
    /// posed place and unioned. A part whose transform has not changed since the
    /// last call keeps its raster, so a crowd where three figures move costs three
    /// figures; the union is rebuilt whenever anything moved.
    pub fn mask(&mut self, cols: u16, rows: u16) -> &Mask {
        let fresh = self.whole.as_ref().is_some_and(|(c, r, _)| (*c, *r) == (cols, rows));
        if !fresh {
            let worlds: Vec<Transform> = (0..self.nodes.len()).map(|i| self.world_of(i)).collect();
            let mut whole = Mask::new(cols, rows);
            for (node, world) in self.nodes.iter_mut().zip(worlds) {
                let stale =
                    node.raster.as_ref().is_none_or(|(t, m)| *t != world || (m.cols(), m.rows()) != (cols, rows));
                if stale {
                    let mut placed = node.part.path.clone();
                    placed.apply(&world);
                    node.raster = Some((world, Mask::path(cols, rows, &placed)));
                }
                whole.union(&node.raster.as_ref().expect("just rasterised").1);
            }
            self.whole = Some((cols, rows, whole));
        }
        &self.whole.as_ref().expect("just built").2
    }

    fn index(&self, name: &str) -> Option<usize> {
        self.nodes.iter().position(|n| n.name == name)
    }

    /// `pose`, then `at`, then the parent's world, then the root.
    fn world_of(&self, i: usize) -> Transform {
        let node = &self.nodes[i];
        let own = node.pose.then(&node.part.at);
        match node.parent {
            Some(p) => own.then(&self.world_of(p)),
            None => own.then(&self.root),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Rgb;

    fn square() -> Path {
        let mut p = Path::new();
        p.rect(-2.0, -2.0, 4.0, 4.0);
        p
    }

    fn near(a: Point, b: Point) -> bool {
        (a.0 - b.0).abs() < 1e-3 && (a.1 - b.1).abs() < 1e-3
    }

    #[test]
    fn poses_compose_down_the_tree_and_points_follow() {
        let mut rig = Rig::new();
        rig.add("body", None, Part::new(square()));
        rig.add("arm", Some("body"), Part::new(square()).at(Transform::at(4.0, 0.0)));
        rig.add("hand", Some("arm"), Part::new(square()).at(Transform::at(4.0, 0.0)));
        rig.mark("finger", "hand", (2.0, 0.0));
        rig.place(Transform::at(10.0, 10.0));
        assert!(near(rig.point("finger"), (20.0, 10.0)));
        rig.pose("arm", Transform::IDENTITY.rotate(std::f32::consts::FRAC_PI_2));
        assert!(near(rig.point("finger"), (14.0, 16.0)), "{:?}", rig.point("finger"));
        assert!(near(rig.world("hand").apply((0.0, 0.0)), (14.0, 14.0)));
        rig.pose("body", Transform::IDENTITY.flip_x());
        assert!(near(rig.point("finger"), (6.0, 16.0)), "a flipped body flips the arm's swing");
        assert!(near(rig.point("nothing"), (0.0, 0.0)));
        assert_eq!(rig.posed("arm").apply((1.0, 0.0)).1.round(), 1.0);
        assert_eq!(rig.parts().collect::<Vec<_>>(), ["body", "arm", "hand"]);
        assert!(rig.part("arm").is_some() && rig.part("leg").is_none());
    }

    #[test]
    fn masks_are_cached_per_part() {
        let mut rig = Rig::new();
        rig.add("body", None, Part::new(square()));
        rig.add("arm", Some("body"), Part::new(square()).at(Transform::at(6.0, 0.0)));
        rig.place(Transform::at(10.0, 10.0));
        let first = rig.mask(12, 5).clone();
        assert_eq!(first.len(), 32);
        assert!(first.contains(9, 9) && first.contains(15, 9));
        let ptr = rig.nodes[0].raster.as_ref().map(|(_, m)| m as *const Mask);
        rig.pose("arm", Transform::IDENTITY.translate(0.0, 5.0));
        let moved = rig.mask(12, 5).clone();
        assert!(moved.contains(15, 14) && !moved.contains(15, 9), "the arm moved");
        assert_eq!(rig.nodes[0].raster.as_ref().map(|(_, m)| m as *const Mask), ptr, "the body kept its raster");
        assert_ne!(first, moved);
        rig.part_mut("body").unwrap().path = Path::new();
        assert_eq!(rig.mask(12, 5).len(), 16, "a changed part is rasterised anew");
        let mut c = Canvas::new(12, 5);
        rig.draw(&mut c, Rgb::hex(0xffffff));
        assert!(c.get(15, 14).is_some() && c.get(9, 9).is_none());
    }

    #[test]
    #[should_panic(expected = "no part named")]
    fn an_unknown_parent_is_a_bug() {
        Rig::new().add("wing", Some("body"), Part::new(square()));
    }
}
