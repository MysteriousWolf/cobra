# `rig`

[Index](README.md) · [canvas](canvas.md) · [draw](draw.md) · [path](path.md) · [mask](mask.md) · [transform](transform.md) · **rig** · [layer](layer.md) · [bubble](bubble.md) · [font](font.md) · [text](text.md) · [color](color.md) · [render](render.md) · [term](term.md) · [export](export.md) · [ratatui](ratatui.md)

Figures as things you pose: a [`Rig`](rig.md#rig) is a tree of [`Part`](rig.md#part)s, each a
[`Path`](path.md#path) in its own coordinates hung off a parent by a [`Transform`](transform.md#transform), with named
points that follow the pose. Pose a rig by setting a handful of transforms,
then rasterise it once.

This is the whole of rigging here and on purpose: a part is rigid, hangs off
one parent, and is posed by one transform. No inverse kinematics, weights,
skinning, timelines or easing; a pose is data, a frame is the caller's clock,
and `t` in [`Rig::mix`](rig.md#rigmix), [`Transform::mix`](transform.md#transformmix) and [`Path::mix`](path.md#pathmix) is the caller's
number.

```rust
use cobra::{Canvas, Part, Path, Rgb, Rig, Transform};

let mut body = Path::new();
body.ellipse(0.0, 0.0, 8.0, 5.0);
let mut wing = Path::new();
wing.polygon(&[(0.0, 0.0), (7.0, -1.0), (9.0, 3.0), (2.0, 3.0)]);

let mut bird = Rig::new();
bird.add("body", None, Part::new(body));
bird.add("wing", Some("body"), Part::new(wing).at(Transform::at(-2.0, -1.0)));
bird.mark("beak", "body", (8.0, 0.0));

bird.place(Transform::at(20.0, 10.0));       // the whole figure
bird.pose("wing", Transform::rotation(-0.4)); // one joint
let beak = bird.point("beak");                // follows the pose

let mut canvas = Canvas::new(20, 5);
canvas.stencil(bird.mask(20, 5), Rgb::hex(0x56d364));
assert!((beak.0 - 28.0).abs() < 1e-3 && (beak.1 - 10.0).abs() < 1e-3);
```

Names are literals, so a name the rig does not have is a typo, and every
method that takes one panics on it rather than draw the wrong figure quietly.

## Contents

- [`Part`](#part)
- [`Rig`](#rig)
- `Part`: [`Part::new`](rig.md#partnew), [`Part::at`](rig.md#partat)
- `Rig`: [`Rig::new`](rig.md#rignew), [`Rig::add`](rig.md#rigadd), [`Rig::mark`](rig.md#rigmark), [`Rig::pose`](rig.md#rigpose), [`Rig::posed`](rig.md#rigposed), [`Rig::place`](rig.md#rigplace), [`Rig::placed`](rig.md#rigplaced), [`Rig::mix`](rig.md#rigmix), [`Rig::part`](rig.md#rigpart), [`Rig::part_mut`](rig.md#rigpart_mut), [`Rig::has`](rig.md#righas), [`Rig::parts`](rig.md#rigparts), [`Rig::world`](rig.md#rigworld), [`Rig::point`](rig.md#rigpoint), [`Rig::each`](rig.md#rigeach), [`Rig::draw`](rig.md#rigdraw), [`Rig::mask`](rig.md#rigmask)

## `Part`

```rust
pub struct Part
```

One rigid piece of a figure: a path drawn about its own joint, and where that
joint sits on its parent.

- `pub path: Path` — The shape, in the part's own coordinates with the origin at its joint.
- `pub at: Transform` — Where the joint sits on the parent, in the parent's coordinates: the part's resting place. A [pose](rig.md#rigpose) happens before this, about the joint.

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="img/part.svg">
  <img src="img/part-light.svg" alt="Part, Part::new, Part::at, Rig::part_mut, Rig::each" width="384">
</picture>

```rust
// A part is a path about its joint, and where the joint sits on the parent.
let mut torso = Path::new();
torso.round_rect(-4.0, -6.0, 8.0, 12.0, 2.0);
let mut arm = Path::new();
arm.round_rect(0.0, -1.5, 11.0, 3.0, 1.5); // along +x from its joint
let mut figure = Rig::new();
figure.add("torso", None, Part::new(torso));
figure.add("arm", Some("torso"), Part::new(arm).at(Transform::at(4.0, -4.5))); // the shoulder
for (i, swing) in [0.0, 0.9].into_iter().enumerate() {
    if i == 1 {
        figure.part_mut("arm").path.ellipse(11.0, 0.0, 2.5, 2.5); // a hand, added to the part
    }
    figure.place(Transform::at(10.0 + 24.0 * i as f32, 8.0));
    figure.pose("arm", Transform::rotation(swing));
    figure.each(|name, path, world| {
        c.with(*world, |c| c.fill_path(path, if name == "arm" { CYAN } else { BLUE }));
    });
    let (jx, jy) = figure.world("arm").apply((0.0, 0.0));
    c.disc(jx, jy, 1.0, RED); // the joint
}
```

**`at`**

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="img/part.svg">
  <img src="img/part-light.svg" alt="Part, Part::new, Part::at, Rig::part_mut, Rig::each" width="384">
</picture>

```rust
// A part is a path about its joint, and where the joint sits on the parent.
let mut torso = Path::new();
torso.round_rect(-4.0, -6.0, 8.0, 12.0, 2.0);
let mut arm = Path::new();
arm.round_rect(0.0, -1.5, 11.0, 3.0, 1.5); // along +x from its joint
let mut figure = Rig::new();
figure.add("torso", None, Part::new(torso));
figure.add("arm", Some("torso"), Part::new(arm).at(Transform::at(4.0, -4.5))); // the shoulder
for (i, swing) in [0.0, 0.9].into_iter().enumerate() {
    if i == 1 {
        figure.part_mut("arm").path.ellipse(11.0, 0.0, 2.5, 2.5); // a hand, added to the part
    }
    figure.place(Transform::at(10.0 + 24.0 * i as f32, 8.0));
    figure.pose("arm", Transform::rotation(swing));
    figure.each(|name, path, world| {
        c.with(*world, |c| c.fill_path(path, if name == "arm" { CYAN } else { BLUE }));
    });
    let (jx, jy) = figure.world("arm").apply((0.0, 0.0));
    c.disc(jx, jy, 1.0, RED); // the joint
}
```

## `Rig`

```rust
pub struct Rig
```

A figure: parts in a tree, named points on them, and a pose per part. See the
module docs.

Parts are drawn in the order they were added, so add what is behind first.
A rig is cheap to clone: `rig.clone()` with another pose is the same figure
again, which is how one description stamps out a crowd.

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="img/rig.svg">
  <img src="img/rig-light.svg" alt="Rig, Rig::new, Rig::add, Rig::pose, Rig::place, Rig::mask, Transform::rotation" width="512">
</picture>

```rust
// One puppet, described once about its hip; three poses of it, stamped.
let limb = |w: f32, len: f32| {
    let mut p = Path::new();
    p.round_rect(-w / 2.0, 0.0, w, len, w / 2.0); // hangs down from its joint
    p
};
let mut torso = Path::new();
torso.round_rect(-2.5, -7.0, 5.0, 7.5, 1.5);
let mut head = Path::new();
head.ellipse(0.0, -3.0, 2.5, 2.8);
let mut puppet = Rig::new();
puppet.add("torso", None, Part::new(torso));
puppet.add("head", Some("torso"), Part::new(head).at(Transform::at(0.0, -7.0)));
for (side, x) in [("l", -1.0), ("r", 1.0)] {
    puppet.add(&format!("arm_{side}"), Some("torso"), Part::new(limb(2.0, 6.0)).at(Transform::at(2.5 * x, -6.5)));
    puppet.add(&format!("leg_{side}"), Some("torso"), Part::new(limb(2.4, 6.5)).at(Transform::at(1.3 * x, 0.0)));
}
let poses: [&[(&str, f32)]; 3] = [
    &[],
    &[("arm_l", 0.7), ("arm_r", -0.7), ("leg_l", -0.4), ("leg_r", 0.4)],
    &[("arm_l", 2.6), ("arm_r", -2.6), ("leg_l", -0.3), ("leg_r", 0.3), ("head", 0.2)],
];
for (i, pose) in poses.into_iter().enumerate() {
    let mut posed = puppet.clone();
    posed.place(Transform::at(10.0 + 22.0 * i as f32, 13.0)); // the hip
    for &(part, angle) in pose {
        posed.pose(part, Transform::rotation(angle)); // about its joint
    }
    c.stencil(posed.mask(32, 5), GREEN);
}
```

## `Part` methods

## `Part::new`

```rust
pub fn new(path: Path) -> Self
```

A part with its joint at the parent's origin.

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="img/part.svg">
  <img src="img/part-light.svg" alt="Part, Part::new, Part::at, Rig::part_mut, Rig::each" width="384">
</picture>

```rust
// A part is a path about its joint, and where the joint sits on the parent.
let mut torso = Path::new();
torso.round_rect(-4.0, -6.0, 8.0, 12.0, 2.0);
let mut arm = Path::new();
arm.round_rect(0.0, -1.5, 11.0, 3.0, 1.5); // along +x from its joint
let mut figure = Rig::new();
figure.add("torso", None, Part::new(torso));
figure.add("arm", Some("torso"), Part::new(arm).at(Transform::at(4.0, -4.5))); // the shoulder
for (i, swing) in [0.0, 0.9].into_iter().enumerate() {
    if i == 1 {
        figure.part_mut("arm").path.ellipse(11.0, 0.0, 2.5, 2.5); // a hand, added to the part
    }
    figure.place(Transform::at(10.0 + 24.0 * i as f32, 8.0));
    figure.pose("arm", Transform::rotation(swing));
    figure.each(|name, path, world| {
        c.with(*world, |c| c.fill_path(path, if name == "arm" { CYAN } else { BLUE }));
    });
    let (jx, jy) = figure.world("arm").apply((0.0, 0.0));
    c.disc(jx, jy, 1.0, RED); // the joint
}
```

## `Part::at`

```rust
pub fn at(mut self, at: Transform) -> Self
```

Sets where the joint sits on the parent (builder style).

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="img/part.svg">
  <img src="img/part-light.svg" alt="Part, Part::new, Part::at, Rig::part_mut, Rig::each" width="384">
</picture>

```rust
// A part is a path about its joint, and where the joint sits on the parent.
let mut torso = Path::new();
torso.round_rect(-4.0, -6.0, 8.0, 12.0, 2.0);
let mut arm = Path::new();
arm.round_rect(0.0, -1.5, 11.0, 3.0, 1.5); // along +x from its joint
let mut figure = Rig::new();
figure.add("torso", None, Part::new(torso));
figure.add("arm", Some("torso"), Part::new(arm).at(Transform::at(4.0, -4.5))); // the shoulder
for (i, swing) in [0.0, 0.9].into_iter().enumerate() {
    if i == 1 {
        figure.part_mut("arm").path.ellipse(11.0, 0.0, 2.5, 2.5); // a hand, added to the part
    }
    figure.place(Transform::at(10.0 + 24.0 * i as f32, 8.0));
    figure.pose("arm", Transform::rotation(swing));
    figure.each(|name, path, world| {
        c.with(*world, |c| c.fill_path(path, if name == "arm" { CYAN } else { BLUE }));
    });
    let (jx, jy) = figure.world("arm").apply((0.0, 0.0));
    c.disc(jx, jy, 1.0, RED); // the joint
}
```

## `Rig` methods

## `Rig::new`

```rust
pub fn new() -> Self
```

An empty rig.

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="img/rig.svg">
  <img src="img/rig-light.svg" alt="Rig, Rig::new, Rig::add, Rig::pose, Rig::place, Rig::mask, Transform::rotation" width="512">
</picture>

```rust
// One puppet, described once about its hip; three poses of it, stamped.
let limb = |w: f32, len: f32| {
    let mut p = Path::new();
    p.round_rect(-w / 2.0, 0.0, w, len, w / 2.0); // hangs down from its joint
    p
};
let mut torso = Path::new();
torso.round_rect(-2.5, -7.0, 5.0, 7.5, 1.5);
let mut head = Path::new();
head.ellipse(0.0, -3.0, 2.5, 2.8);
let mut puppet = Rig::new();
puppet.add("torso", None, Part::new(torso));
puppet.add("head", Some("torso"), Part::new(head).at(Transform::at(0.0, -7.0)));
for (side, x) in [("l", -1.0), ("r", 1.0)] {
    puppet.add(&format!("arm_{side}"), Some("torso"), Part::new(limb(2.0, 6.0)).at(Transform::at(2.5 * x, -6.5)));
    puppet.add(&format!("leg_{side}"), Some("torso"), Part::new(limb(2.4, 6.5)).at(Transform::at(1.3 * x, 0.0)));
}
let poses: [&[(&str, f32)]; 3] = [
    &[],
    &[("arm_l", 0.7), ("arm_r", -0.7), ("leg_l", -0.4), ("leg_r", 0.4)],
    &[("arm_l", 2.6), ("arm_r", -2.6), ("leg_l", -0.3), ("leg_r", 0.3), ("head", 0.2)],
];
for (i, pose) in poses.into_iter().enumerate() {
    let mut posed = puppet.clone();
    posed.place(Transform::at(10.0 + 22.0 * i as f32, 13.0)); // the hip
    for &(part, angle) in pose {
        posed.pose(part, Transform::rotation(angle)); // about its joint
    }
    c.stencil(posed.mask(32, 5), GREEN);
}
```

## `Rig::add`

```rust
pub fn add(&mut self, name: &str, parent: Option<&str>, part: Part) -> &mut Self
```

Adds `part` as `name`, hung off `parent` (`None` for the figure itself).
Names are how parts are posed and marked, so each must be unique.

### Panics

If `parent` names no part, or `name` is already taken.

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="img/rig.svg">
  <img src="img/rig-light.svg" alt="Rig, Rig::new, Rig::add, Rig::pose, Rig::place, Rig::mask, Transform::rotation" width="512">
</picture>

```rust
// One puppet, described once about its hip; three poses of it, stamped.
let limb = |w: f32, len: f32| {
    let mut p = Path::new();
    p.round_rect(-w / 2.0, 0.0, w, len, w / 2.0); // hangs down from its joint
    p
};
let mut torso = Path::new();
torso.round_rect(-2.5, -7.0, 5.0, 7.5, 1.5);
let mut head = Path::new();
head.ellipse(0.0, -3.0, 2.5, 2.8);
let mut puppet = Rig::new();
puppet.add("torso", None, Part::new(torso));
puppet.add("head", Some("torso"), Part::new(head).at(Transform::at(0.0, -7.0)));
for (side, x) in [("l", -1.0), ("r", 1.0)] {
    puppet.add(&format!("arm_{side}"), Some("torso"), Part::new(limb(2.0, 6.0)).at(Transform::at(2.5 * x, -6.5)));
    puppet.add(&format!("leg_{side}"), Some("torso"), Part::new(limb(2.4, 6.5)).at(Transform::at(1.3 * x, 0.0)));
}
let poses: [&[(&str, f32)]; 3] = [
    &[],
    &[("arm_l", 0.7), ("arm_r", -0.7), ("leg_l", -0.4), ("leg_r", 0.4)],
    &[("arm_l", 2.6), ("arm_r", -2.6), ("leg_l", -0.3), ("leg_r", 0.3), ("head", 0.2)],
];
for (i, pose) in poses.into_iter().enumerate() {
    let mut posed = puppet.clone();
    posed.place(Transform::at(10.0 + 22.0 * i as f32, 13.0)); // the hip
    for &(part, angle) in pose {
        posed.pose(part, Transform::rotation(angle)); // about its joint
    }
    c.stencil(posed.mask(32, 5), GREEN);
}
```

## `Rig::mark`

```rust
pub fn mark(&mut self, name: &str, part: &str, at: Point) -> &mut Self
```

Names point `at` (in `part`'s coordinates) `name`, so [`point`](rig.md#rigpoint)
can find it wherever the pose puts it: where a hat sits, where a hand is,
where a speech bubble's tail should aim. Marking a name again moves it.

### Panics

If `part` names no part.

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="img/rig-point.svg">
  <img src="img/rig-point-light.svg" alt="Rig::point, Rig::mark, Rig::draw, Rig::world" width="384">
</picture>

```rust
// A named point follows the pose, so a nose stays on the face; a hat drawn
// through the head's world transform tilts with it.
let mut torso = Path::new();
torso.rect(-3.0, -6.0, 6.0, 6.0);
let mut head = Path::new();
head.ellipse(0.0, -3.2, 2.8, 3.2);
let mut figure = Rig::new();
figure.add("torso", None, Part::new(torso));
figure.add("head", Some("torso"), Part::new(head).at(Transform::at(0.0, -6.0)));
figure.mark("nose", "head", (2.6, -3.0));
for (i, tilt) in [-0.5, 0.0, 0.5].into_iter().enumerate() {
    figure.place(Transform::at(8.0 + 16.0 * i as f32, 18.0));
    figure.pose("head", Transform::rotation(tilt));
    figure.draw(c, BLUE);
    let (nx, ny) = figure.point("nose");
    c.disc(nx, ny, 1.0, ORANGE);
    c.with(figure.world("head"), |c| {
        c.fill_rect(-3.8, -7.0, 7.6, 1.5, RED); // the brim
        c.fill_rect(-2.2, -10.5, 4.4, 3.5, RED); // the crown
    });
}
```

## `Rig::pose`

```rust
pub fn pose(&mut self, part: &str, t: Transform) -> &mut Self
```

Poses `part`: `t` is applied about its joint, before its resting place, so
`Transform::rotation(0.3)` swings it and `Transform::at(0.0, -2.0)` lifts
it. Everything hung off the part moves with it.

### Panics

If `part` names no part.

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="img/rig.svg">
  <img src="img/rig-light.svg" alt="Rig, Rig::new, Rig::add, Rig::pose, Rig::place, Rig::mask, Transform::rotation" width="512">
</picture>

```rust
// One puppet, described once about its hip; three poses of it, stamped.
let limb = |w: f32, len: f32| {
    let mut p = Path::new();
    p.round_rect(-w / 2.0, 0.0, w, len, w / 2.0); // hangs down from its joint
    p
};
let mut torso = Path::new();
torso.round_rect(-2.5, -7.0, 5.0, 7.5, 1.5);
let mut head = Path::new();
head.ellipse(0.0, -3.0, 2.5, 2.8);
let mut puppet = Rig::new();
puppet.add("torso", None, Part::new(torso));
puppet.add("head", Some("torso"), Part::new(head).at(Transform::at(0.0, -7.0)));
for (side, x) in [("l", -1.0), ("r", 1.0)] {
    puppet.add(&format!("arm_{side}"), Some("torso"), Part::new(limb(2.0, 6.0)).at(Transform::at(2.5 * x, -6.5)));
    puppet.add(&format!("leg_{side}"), Some("torso"), Part::new(limb(2.4, 6.5)).at(Transform::at(1.3 * x, 0.0)));
}
let poses: [&[(&str, f32)]; 3] = [
    &[],
    &[("arm_l", 0.7), ("arm_r", -0.7), ("leg_l", -0.4), ("leg_r", 0.4)],
    &[("arm_l", 2.6), ("arm_r", -2.6), ("leg_l", -0.3), ("leg_r", 0.3), ("head", 0.2)],
];
for (i, pose) in poses.into_iter().enumerate() {
    let mut posed = puppet.clone();
    posed.place(Transform::at(10.0 + 22.0 * i as f32, 13.0)); // the hip
    for &(part, angle) in pose {
        posed.pose(part, Transform::rotation(angle)); // about its joint
    }
    c.stencil(posed.mask(32, 5), GREEN);
}
```

## `Rig::posed`

```rust
pub fn posed(&self, part: &str) -> Transform
```

The pose of `part`, the identity when unposed.

### Panics

If `part` names no part.

## `Rig::place`

```rust
pub fn place(&mut self, t: Transform) -> &mut Self
```

Places the whole figure: where its root sits on the canvas, facing which way,
at what size.

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="img/rig.svg">
  <img src="img/rig-light.svg" alt="Rig, Rig::new, Rig::add, Rig::pose, Rig::place, Rig::mask, Transform::rotation" width="512">
</picture>

```rust
// One puppet, described once about its hip; three poses of it, stamped.
let limb = |w: f32, len: f32| {
    let mut p = Path::new();
    p.round_rect(-w / 2.0, 0.0, w, len, w / 2.0); // hangs down from its joint
    p
};
let mut torso = Path::new();
torso.round_rect(-2.5, -7.0, 5.0, 7.5, 1.5);
let mut head = Path::new();
head.ellipse(0.0, -3.0, 2.5, 2.8);
let mut puppet = Rig::new();
puppet.add("torso", None, Part::new(torso));
puppet.add("head", Some("torso"), Part::new(head).at(Transform::at(0.0, -7.0)));
for (side, x) in [("l", -1.0), ("r", 1.0)] {
    puppet.add(&format!("arm_{side}"), Some("torso"), Part::new(limb(2.0, 6.0)).at(Transform::at(2.5 * x, -6.5)));
    puppet.add(&format!("leg_{side}"), Some("torso"), Part::new(limb(2.4, 6.5)).at(Transform::at(1.3 * x, 0.0)));
}
let poses: [&[(&str, f32)]; 3] = [
    &[],
    &[("arm_l", 0.7), ("arm_r", -0.7), ("leg_l", -0.4), ("leg_r", 0.4)],
    &[("arm_l", 2.6), ("arm_r", -2.6), ("leg_l", -0.3), ("leg_r", 0.3), ("head", 0.2)],
];
for (i, pose) in poses.into_iter().enumerate() {
    let mut posed = puppet.clone();
    posed.place(Transform::at(10.0 + 22.0 * i as f32, 13.0)); // the hip
    for &(part, angle) in pose {
        posed.pose(part, Transform::rotation(angle)); // about its joint
    }
    c.stencil(posed.mask(32, 5), GREEN);
}
```

## `Rig::placed`

```rust
pub fn placed(&self) -> Transform
```

Where the figure sits; see [`place`](rig.md#rigplace).

## `Rig::mix`

```rust
pub fn mix(a: &Rig, b: &Rig, t: f32) -> Rig
```

The rig between `a` (at `t = 0`) and `b` (at `t = 1`): the same figure with
every pose, and its place, [mixed](transform.md#transformmix) between the two. Two
keyframes and one number is a walk cycle, a nod, a wave. The rasters cached
in `a` come along, so parts posed the same in both are not drawn again.

### Panics

If the two rigs are not the same figure: the same parts in the same order.

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="img/rig-mix.svg">
  <img src="img/rig-mix-light.svg" alt="Rig::mix" width="512">
</picture>

```rust
// Two keyframes of one rig, and the frames between them: every joint and
// the place tween together, so a wave walks across.
let mut torso = Path::new();
torso.round_rect(-3.0, -7.0, 6.0, 12.0, 2.0);
let mut arm = Path::new();
arm.round_rect(-1.2, -8.0, 2.4, 8.0, 1.2); // up from the shoulder
let mut down = Rig::new();
down.add("torso", None, Part::new(torso));
down.add("arm", Some("torso"), Part::new(arm).at(Transform::at(3.0, -5.5)));
down.pose("arm", Transform::rotation(2.3));
down.place(Transform::at(6.0, 10.0));
let mut up = down.clone();
up.pose("arm", Transform::rotation(0.4));
up.place(Transform::at(54.0, 10.0));
for i in 0..5 {
    let frame = Rig::mix(&down, &up, i as f32 / 4.0);
    frame.draw(c, BLUE.lerp(PURPLE, i as f32 / 4.0));
}
```

## `Rig::part`

```rust
pub fn part(&self, name: &str) -> &Part
```

The part called `name`.

### Panics

If `name` names no part.

## `Rig::part_mut`

```rust
pub fn part_mut(&mut self, name: &str) -> &mut Part
```

The part called `name`, to change. Changing a part's path or resting place
is what a pose cannot do (a beak opening is two paths, [`Path::mix`](path.md#pathmix)ed); the
part is rasterised anew.

### Panics

If `name` names no part.

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="img/part.svg">
  <img src="img/part-light.svg" alt="Part, Part::new, Part::at, Rig::part_mut, Rig::each" width="384">
</picture>

```rust
// A part is a path about its joint, and where the joint sits on the parent.
let mut torso = Path::new();
torso.round_rect(-4.0, -6.0, 8.0, 12.0, 2.0);
let mut arm = Path::new();
arm.round_rect(0.0, -1.5, 11.0, 3.0, 1.5); // along +x from its joint
let mut figure = Rig::new();
figure.add("torso", None, Part::new(torso));
figure.add("arm", Some("torso"), Part::new(arm).at(Transform::at(4.0, -4.5))); // the shoulder
for (i, swing) in [0.0, 0.9].into_iter().enumerate() {
    if i == 1 {
        figure.part_mut("arm").path.ellipse(11.0, 0.0, 2.5, 2.5); // a hand, added to the part
    }
    figure.place(Transform::at(10.0 + 24.0 * i as f32, 8.0));
    figure.pose("arm", Transform::rotation(swing));
    figure.each(|name, path, world| {
        c.with(*world, |c| c.fill_path(path, if name == "arm" { CYAN } else { BLUE }));
    });
    let (jx, jy) = figure.world("arm").apply((0.0, 0.0));
    c.disc(jx, jy, 1.0, RED); // the joint
}
```

## `Rig::has`

```rust
pub fn has(&self, name: &str) -> bool
```

Whether the rig has a part called `name`.

## `Rig::parts`

```rust
pub fn parts(&self) -> impl Iterator<Item = &str>
```

The names of the parts, in drawing order.

## `Rig::world`

```rust
pub fn world(&self, part: &str) -> Transform
```

The transform from `part`'s own coordinates to the canvas, with every pose
above it applied.

### Panics

If `part` names no part.

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="img/rig-point.svg">
  <img src="img/rig-point-light.svg" alt="Rig::point, Rig::mark, Rig::draw, Rig::world" width="384">
</picture>

```rust
// A named point follows the pose, so a nose stays on the face; a hat drawn
// through the head's world transform tilts with it.
let mut torso = Path::new();
torso.rect(-3.0, -6.0, 6.0, 6.0);
let mut head = Path::new();
head.ellipse(0.0, -3.2, 2.8, 3.2);
let mut figure = Rig::new();
figure.add("torso", None, Part::new(torso));
figure.add("head", Some("torso"), Part::new(head).at(Transform::at(0.0, -6.0)));
figure.mark("nose", "head", (2.6, -3.0));
for (i, tilt) in [-0.5, 0.0, 0.5].into_iter().enumerate() {
    figure.place(Transform::at(8.0 + 16.0 * i as f32, 18.0));
    figure.pose("head", Transform::rotation(tilt));
    figure.draw(c, BLUE);
    let (nx, ny) = figure.point("nose");
    c.disc(nx, ny, 1.0, ORANGE);
    c.with(figure.world("head"), |c| {
        c.fill_rect(-3.8, -7.0, 7.6, 1.5, RED); // the brim
        c.fill_rect(-2.2, -10.5, 4.4, 3.5, RED); // the crown
    });
}
```

## `Rig::point`

```rust
pub fn point(&self, name: &str) -> Point
```

Where the point named `name` is on the canvas, posed.

### Panics

If nothing was [marked](rig.md#rigmark) `name`.

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="img/rig-point.svg">
  <img src="img/rig-point-light.svg" alt="Rig::point, Rig::mark, Rig::draw, Rig::world" width="384">
</picture>

```rust
// A named point follows the pose, so a nose stays on the face; a hat drawn
// through the head's world transform tilts with it.
let mut torso = Path::new();
torso.rect(-3.0, -6.0, 6.0, 6.0);
let mut head = Path::new();
head.ellipse(0.0, -3.2, 2.8, 3.2);
let mut figure = Rig::new();
figure.add("torso", None, Part::new(torso));
figure.add("head", Some("torso"), Part::new(head).at(Transform::at(0.0, -6.0)));
figure.mark("nose", "head", (2.6, -3.0));
for (i, tilt) in [-0.5, 0.0, 0.5].into_iter().enumerate() {
    figure.place(Transform::at(8.0 + 16.0 * i as f32, 18.0));
    figure.pose("head", Transform::rotation(tilt));
    figure.draw(c, BLUE);
    let (nx, ny) = figure.point("nose");
    c.disc(nx, ny, 1.0, ORANGE);
    c.with(figure.world("head"), |c| {
        c.fill_rect(-3.8, -7.0, 7.6, 1.5, RED); // the brim
        c.fill_rect(-2.2, -10.5, 4.4, 3.5, RED); // the crown
    });
}
```

## `Rig::each`

```rust
pub fn each(&self, mut f: impl FnMut(&str, &Path, &Transform))
```

Calls `f` for every part in drawing order with its name, its path and the
transform that puts the path on the canvas.

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="img/part.svg">
  <img src="img/part-light.svg" alt="Part, Part::new, Part::at, Rig::part_mut, Rig::each" width="384">
</picture>

```rust
// A part is a path about its joint, and where the joint sits on the parent.
let mut torso = Path::new();
torso.round_rect(-4.0, -6.0, 8.0, 12.0, 2.0);
let mut arm = Path::new();
arm.round_rect(0.0, -1.5, 11.0, 3.0, 1.5); // along +x from its joint
let mut figure = Rig::new();
figure.add("torso", None, Part::new(torso));
figure.add("arm", Some("torso"), Part::new(arm).at(Transform::at(4.0, -4.5))); // the shoulder
for (i, swing) in [0.0, 0.9].into_iter().enumerate() {
    if i == 1 {
        figure.part_mut("arm").path.ellipse(11.0, 0.0, 2.5, 2.5); // a hand, added to the part
    }
    figure.place(Transform::at(10.0 + 24.0 * i as f32, 8.0));
    figure.pose("arm", Transform::rotation(swing));
    figure.each(|name, path, world| {
        c.with(*world, |c| c.fill_path(path, if name == "arm" { CYAN } else { BLUE }));
    });
    let (jx, jy) = figure.world("arm").apply((0.0, 0.0));
    c.disc(jx, jy, 1.0, RED); // the joint
}
```

## `Rig::draw`

```rust
pub fn draw(&self, canvas: &mut Canvas, paint: impl Into<Paint>)
```

Fills every part with `paint`, in drawing order, through the canvas's
current transform. For one silhouette to shade or outline as a whole, use
[`mask`](rig.md#rigmask) and [`Canvas::stencil`](draw.md#canvasstencil) instead.

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="img/rig-point.svg">
  <img src="img/rig-point-light.svg" alt="Rig::point, Rig::mark, Rig::draw, Rig::world" width="384">
</picture>

```rust
// A named point follows the pose, so a nose stays on the face; a hat drawn
// through the head's world transform tilts with it.
let mut torso = Path::new();
torso.rect(-3.0, -6.0, 6.0, 6.0);
let mut head = Path::new();
head.ellipse(0.0, -3.2, 2.8, 3.2);
let mut figure = Rig::new();
figure.add("torso", None, Part::new(torso));
figure.add("head", Some("torso"), Part::new(head).at(Transform::at(0.0, -6.0)));
figure.mark("nose", "head", (2.6, -3.0));
for (i, tilt) in [-0.5, 0.0, 0.5].into_iter().enumerate() {
    figure.place(Transform::at(8.0 + 16.0 * i as f32, 18.0));
    figure.pose("head", Transform::rotation(tilt));
    figure.draw(c, BLUE);
    let (nx, ny) = figure.point("nose");
    c.disc(nx, ny, 1.0, ORANGE);
    c.with(figure.world("head"), |c| {
        c.fill_rect(-3.8, -7.0, 7.6, 1.5, RED); // the brim
        c.fill_rect(-2.2, -10.5, 4.4, 3.5, RED); // the crown
    });
}
```

## `Rig::mask`

```rust
pub fn mask(&mut self, cols: u16, rows: u16) -> &Mask
```

The whole figure as one `cols × rows` mask: every part rasterised at its
posed place and unioned. A part whose transform has not changed since the
last call keeps its raster, so a crowd where three figures move costs three
figures; the union is rebuilt whenever anything moved.

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="img/rig.svg">
  <img src="img/rig-light.svg" alt="Rig, Rig::new, Rig::add, Rig::pose, Rig::place, Rig::mask, Transform::rotation" width="512">
</picture>

```rust
// One puppet, described once about its hip; three poses of it, stamped.
let limb = |w: f32, len: f32| {
    let mut p = Path::new();
    p.round_rect(-w / 2.0, 0.0, w, len, w / 2.0); // hangs down from its joint
    p
};
let mut torso = Path::new();
torso.round_rect(-2.5, -7.0, 5.0, 7.5, 1.5);
let mut head = Path::new();
head.ellipse(0.0, -3.0, 2.5, 2.8);
let mut puppet = Rig::new();
puppet.add("torso", None, Part::new(torso));
puppet.add("head", Some("torso"), Part::new(head).at(Transform::at(0.0, -7.0)));
for (side, x) in [("l", -1.0), ("r", 1.0)] {
    puppet.add(&format!("arm_{side}"), Some("torso"), Part::new(limb(2.0, 6.0)).at(Transform::at(2.5 * x, -6.5)));
    puppet.add(&format!("leg_{side}"), Some("torso"), Part::new(limb(2.4, 6.5)).at(Transform::at(1.3 * x, 0.0)));
}
let poses: [&[(&str, f32)]; 3] = [
    &[],
    &[("arm_l", 0.7), ("arm_r", -0.7), ("leg_l", -0.4), ("leg_r", 0.4)],
    &[("arm_l", 2.6), ("arm_r", -2.6), ("leg_l", -0.3), ("leg_r", 0.3), ("head", 0.2)],
];
for (i, pose) in poses.into_iter().enumerate() {
    let mut posed = puppet.clone();
    posed.place(Transform::at(10.0 + 22.0 * i as f32, 13.0)); // the hip
    for &(part, angle) in pose {
        posed.pose(part, Transform::rotation(angle)); // about its joint
    }
    c.stencil(posed.mask(32, 5), GREEN);
}
```

[Index](README.md) · [canvas](canvas.md) · [draw](draw.md) · [path](path.md) · [mask](mask.md) · [transform](transform.md) · **rig** · [layer](layer.md) · [bubble](bubble.md) · [font](font.md) · [text](text.md) · [color](color.md) · [render](render.md) · [term](term.md) · [export](export.md) · [ratatui](ratatui.md)

