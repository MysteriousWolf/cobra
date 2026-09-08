# `rig`

[Index](README.md) · [canvas](canvas.md) · [draw](draw.md) · [path](path.md) · [mask](mask.md) · [transform](transform.md) · **rig** · [layer](layer.md) · [bubble](bubble.md) · [font](font.md) · [text](text.md) · [color](color.md) · [render](render.md) · [term](term.md) · [export](export.md) · [ratatui](ratatui.md)

Figures as things you pose: a [`Rig`](rig.md#rig) is a tree of [`Part`](rig.md#part)s, each a
[`Path`](path.md#path) in its own coordinates hung off a parent by a [`Transform`](transform.md#transform), with named
points that follow the pose. Pose a rig by setting a handful of transforms,
then rasterise it once.

This is the whole of rigging here and on purpose: a part is rigid, hangs off
one parent, and is posed by one transform. No inverse kinematics, weights,
skinning, timelines or easing; a pose is data, a frame is the caller's clock,
and `t` in [`Transform::mix`](transform.md#transformmix) and [`Path::mix`](path.md#pathmix) is the caller's number.

```rust
use cobra::{Canvas, Mask, Part, Path, Rgb, Rig, Transform};

let mut body = Path::new();
body.ellipse(0.0, 0.0, 8.0, 5.0);
let mut wing = Path::new();
wing.polygon(&[(0.0, 0.0), (7.0, -1.0), (9.0, 3.0), (2.0, 3.0)]);

let mut bird = Rig::new();
bird.add("body", None, Part::new(body));
bird.add("wing", Some("body"), Part::new(wing).at(Transform::at(-2.0, -1.0)));
bird.mark("beak", "body", (8.0, 0.0));

bird.place(Transform::at(20.0, 10.0));                   // the whole figure
bird.pose("wing", Transform::IDENTITY.rotate(-0.4));   // one joint
let beak = bird.point("beak");                          // follows the pose

let mut canvas = Canvas::new(20, 5);
canvas.stencil(bird.mask(20, 5), Rgb::hex(0x56d364));
assert!((beak.0 - 28.0).abs() < 1e-3 && (beak.1 - 10.0).abs() < 1e-3);
```

## Contents

- [`Part`](#part)
- [`Rig`](#rig)
- `Part`: [`Part::new`](rig.md#partnew), [`Part::at`](rig.md#partat)
- `Rig`: [`Rig::new`](rig.md#rignew), [`Rig::add`](rig.md#rigadd), [`Rig::mark`](rig.md#rigmark), [`Rig::pose`](rig.md#rigpose), [`Rig::posed`](rig.md#rigposed), [`Rig::place`](rig.md#rigplace), [`Rig::placed`](rig.md#rigplaced), [`Rig::part`](rig.md#rigpart), [`Rig::part_mut`](rig.md#rigpart_mut), [`Rig::parts`](rig.md#rigparts), [`Rig::world`](rig.md#rigworld), [`Rig::point`](rig.md#rigpoint), [`Rig::each`](rig.md#rigeach), [`Rig::draw`](rig.md#rigdraw), [`Rig::mask`](rig.md#rigmask)

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
  <img src="img/part-light.svg" alt="Part, Part::new, Part::at, Rig::part_mut" width="384">
</picture>

```rust
// A part is a path about its joint, and where the joint sits on the parent.
let mut arm = Path::new();
arm.round_rect(0.0, -1.5, 12.0, 3.0, 1.5);
let mut figure = Rig::new();
let mut torso = Path::new();
torso.rect(-4.0, -6.0, 8.0, 12.0);
figure.add("torso", None, Part::new(torso));
figure.add("arm", Some("torso"), Part::new(arm).at(Transform::at(4.0, -4.0)));
figure.place(Transform::at(12.0, 8.0));
figure.draw(c, t.panel);
c.disc(16.0, 4.0, 1.0, RED); // the joint
figure.place(Transform::at(34.0, 8.0));
figure.pose("arm", Transform::IDENTITY.rotate(0.9));
figure.part_mut("arm").unwrap().path.ellipse(12.0, 0.0, 2.5, 2.5); // a hand, added to the part
figure.draw(c, PURPLE);
```

**`at`**

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="img/part.svg">
  <img src="img/part-light.svg" alt="Part, Part::new, Part::at, Rig::part_mut" width="384">
</picture>

```rust
// A part is a path about its joint, and where the joint sits on the parent.
let mut arm = Path::new();
arm.round_rect(0.0, -1.5, 12.0, 3.0, 1.5);
let mut figure = Rig::new();
let mut torso = Path::new();
torso.rect(-4.0, -6.0, 8.0, 12.0);
figure.add("torso", None, Part::new(torso));
figure.add("arm", Some("torso"), Part::new(arm).at(Transform::at(4.0, -4.0)));
figure.place(Transform::at(12.0, 8.0));
figure.draw(c, t.panel);
c.disc(16.0, 4.0, 1.0, RED); // the joint
figure.place(Transform::at(34.0, 8.0));
figure.pose("arm", Transform::IDENTITY.rotate(0.9));
figure.part_mut("arm").unwrap().path.ellipse(12.0, 0.0, 2.5, 2.5); // a hand, added to the part
figure.draw(c, PURPLE);
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
  <img src="img/rig-light.svg" alt="Rig, Rig::new, Rig::add, Rig::pose, Rig::place, Rig::mask" width="512">
</picture>

```rust
// One bird, described once; three poses of it, stamped.
let mut body = Path::new();
body.ellipse(0.0, 0.0, 7.0, 4.5);
let mut head = Path::new();
head.ellipse(0.0, 0.0, 3.0, 2.8);
let mut wing = Path::new();
wing.polygon(&[(0.0, 0.0), (8.0, -1.0), (9.0, 2.5), (1.0, 3.0)]);
let mut bird = Rig::new();
bird.add("body", None, Part::new(body));
bird.add("head", Some("body"), Part::new(head).at(Transform::at(6.5, -4.5)));
bird.add("wing", Some("body"), Part::new(wing).at(Transform::at(-3.0, -1.0)));
for (i, lift) in [0.0, -0.5, -1.0].into_iter().enumerate() {
    let mut posed = bird.clone();
    posed.place(Transform::at(12.0 + 20.0 * i as f32, 12.0));
    posed.pose("wing", Transform::IDENTITY.rotate(lift));
    posed.pose("head", Transform::IDENTITY.rotate(0.2 * i as f32));
    c.stencil(posed.mask(32, 5), Paint::cel(GREEN.dim(0.5), (-1.0, -1.0), &[(-0.1, GREEN)]).per_cell());
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
  <img src="img/part-light.svg" alt="Part, Part::new, Part::at, Rig::part_mut" width="384">
</picture>

```rust
// A part is a path about its joint, and where the joint sits on the parent.
let mut arm = Path::new();
arm.round_rect(0.0, -1.5, 12.0, 3.0, 1.5);
let mut figure = Rig::new();
let mut torso = Path::new();
torso.rect(-4.0, -6.0, 8.0, 12.0);
figure.add("torso", None, Part::new(torso));
figure.add("arm", Some("torso"), Part::new(arm).at(Transform::at(4.0, -4.0)));
figure.place(Transform::at(12.0, 8.0));
figure.draw(c, t.panel);
c.disc(16.0, 4.0, 1.0, RED); // the joint
figure.place(Transform::at(34.0, 8.0));
figure.pose("arm", Transform::IDENTITY.rotate(0.9));
figure.part_mut("arm").unwrap().path.ellipse(12.0, 0.0, 2.5, 2.5); // a hand, added to the part
figure.draw(c, PURPLE);
```

## `Part::at`

```rust
pub fn at(mut self, at: Transform) -> Self
```

Sets where the joint sits on the parent (builder style).

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="img/part.svg">
  <img src="img/part-light.svg" alt="Part, Part::new, Part::at, Rig::part_mut" width="384">
</picture>

```rust
// A part is a path about its joint, and where the joint sits on the parent.
let mut arm = Path::new();
arm.round_rect(0.0, -1.5, 12.0, 3.0, 1.5);
let mut figure = Rig::new();
let mut torso = Path::new();
torso.rect(-4.0, -6.0, 8.0, 12.0);
figure.add("torso", None, Part::new(torso));
figure.add("arm", Some("torso"), Part::new(arm).at(Transform::at(4.0, -4.0)));
figure.place(Transform::at(12.0, 8.0));
figure.draw(c, t.panel);
c.disc(16.0, 4.0, 1.0, RED); // the joint
figure.place(Transform::at(34.0, 8.0));
figure.pose("arm", Transform::IDENTITY.rotate(0.9));
figure.part_mut("arm").unwrap().path.ellipse(12.0, 0.0, 2.5, 2.5); // a hand, added to the part
figure.draw(c, PURPLE);
```

## `Rig` methods

## `Rig::new`

```rust
pub fn new() -> Self
```

An empty rig.

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="img/rig.svg">
  <img src="img/rig-light.svg" alt="Rig, Rig::new, Rig::add, Rig::pose, Rig::place, Rig::mask" width="512">
</picture>

```rust
// One bird, described once; three poses of it, stamped.
let mut body = Path::new();
body.ellipse(0.0, 0.0, 7.0, 4.5);
let mut head = Path::new();
head.ellipse(0.0, 0.0, 3.0, 2.8);
let mut wing = Path::new();
wing.polygon(&[(0.0, 0.0), (8.0, -1.0), (9.0, 2.5), (1.0, 3.0)]);
let mut bird = Rig::new();
bird.add("body", None, Part::new(body));
bird.add("head", Some("body"), Part::new(head).at(Transform::at(6.5, -4.5)));
bird.add("wing", Some("body"), Part::new(wing).at(Transform::at(-3.0, -1.0)));
for (i, lift) in [0.0, -0.5, -1.0].into_iter().enumerate() {
    let mut posed = bird.clone();
    posed.place(Transform::at(12.0 + 20.0 * i as f32, 12.0));
    posed.pose("wing", Transform::IDENTITY.rotate(lift));
    posed.pose("head", Transform::IDENTITY.rotate(0.2 * i as f32));
    c.stencil(posed.mask(32, 5), Paint::cel(GREEN.dim(0.5), (-1.0, -1.0), &[(-0.1, GREEN)]).per_cell());
}
```

## `Rig::add`

```rust
pub fn add(&mut self, name: &str, parent: Option<&str>, part: Part) -> &mut Self
```

Adds `part` as `name`, hung off `parent` (`None` for the figure itself).
Names are how parts are posed and marked, so each must be unique.

### Panics

If `parent` names no part, or `name` is already taken: a rig is built from
literals, and a typo there is a bug worth stopping on.

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="img/rig.svg">
  <img src="img/rig-light.svg" alt="Rig, Rig::new, Rig::add, Rig::pose, Rig::place, Rig::mask" width="512">
</picture>

```rust
// One bird, described once; three poses of it, stamped.
let mut body = Path::new();
body.ellipse(0.0, 0.0, 7.0, 4.5);
let mut head = Path::new();
head.ellipse(0.0, 0.0, 3.0, 2.8);
let mut wing = Path::new();
wing.polygon(&[(0.0, 0.0), (8.0, -1.0), (9.0, 2.5), (1.0, 3.0)]);
let mut bird = Rig::new();
bird.add("body", None, Part::new(body));
bird.add("head", Some("body"), Part::new(head).at(Transform::at(6.5, -4.5)));
bird.add("wing", Some("body"), Part::new(wing).at(Transform::at(-3.0, -1.0)));
for (i, lift) in [0.0, -0.5, -1.0].into_iter().enumerate() {
    let mut posed = bird.clone();
    posed.place(Transform::at(12.0 + 20.0 * i as f32, 12.0));
    posed.pose("wing", Transform::IDENTITY.rotate(lift));
    posed.pose("head", Transform::IDENTITY.rotate(0.2 * i as f32));
    c.stencil(posed.mask(32, 5), Paint::cel(GREEN.dim(0.5), (-1.0, -1.0), &[(-0.1, GREEN)]).per_cell());
}
```

## `Rig::mark`

```rust
pub fn mark(&mut self, name: &str, part: &str, at: Point) -> &mut Self
```

Names point `at` (in `part`'s coordinates) `name`, so [`point`](rig.md#rigpoint)
can find it wherever the pose puts it: where a hat sits, where a hand is,
where a speech bubble's tail should aim.

### Panics

If `part` names no part.

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="img/rig-point.svg">
  <img src="img/rig-point-light.svg" alt="Rig::point, Rig::mark, Rig::draw" width="384">
</picture>

```rust
// A named point follows the pose, so a hat lands wherever the head went.
let mut body = Path::new();
body.ellipse(0.0, 0.0, 6.0, 4.0);
let mut head = Path::new();
head.ellipse(0.0, 0.0, 2.8, 2.8);
let mut figure = Rig::new();
figure.add("body", None, Part::new(body));
figure.add("head", Some("body"), Part::new(head).at(Transform::at(6.0, -4.0)));
figure.mark("crown", "head", (0.0, -2.8));
for (i, nod) in [-0.5, 0.0, 0.5].into_iter().enumerate() {
    figure.place(Transform::at(6.0 + 16.0 * i as f32, 11.0));
    figure.pose("head", Transform::IDENTITY.rotate(nod));
    figure.draw(c, BLUE);
    let (hx, hy) = figure.point("crown");
    c.fill_rect(hx - 2.5, hy - 3.0, 5.0, 3.0, RED); // the hat
}
```

## `Rig::pose`

```rust
pub fn pose(&mut self, part: &str, t: Transform) -> &mut Self
```

Poses `part`: `t` is applied about its joint, before its resting place, so
`Transform::IDENTITY.rotate(0.3)` swings it and `.translate(0.0, -2.0)` lifts
it. Everything hung off the part moves with it. Does nothing for an unknown
name.

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="img/rig.svg">
  <img src="img/rig-light.svg" alt="Rig, Rig::new, Rig::add, Rig::pose, Rig::place, Rig::mask" width="512">
</picture>

```rust
// One bird, described once; three poses of it, stamped.
let mut body = Path::new();
body.ellipse(0.0, 0.0, 7.0, 4.5);
let mut head = Path::new();
head.ellipse(0.0, 0.0, 3.0, 2.8);
let mut wing = Path::new();
wing.polygon(&[(0.0, 0.0), (8.0, -1.0), (9.0, 2.5), (1.0, 3.0)]);
let mut bird = Rig::new();
bird.add("body", None, Part::new(body));
bird.add("head", Some("body"), Part::new(head).at(Transform::at(6.5, -4.5)));
bird.add("wing", Some("body"), Part::new(wing).at(Transform::at(-3.0, -1.0)));
for (i, lift) in [0.0, -0.5, -1.0].into_iter().enumerate() {
    let mut posed = bird.clone();
    posed.place(Transform::at(12.0 + 20.0 * i as f32, 12.0));
    posed.pose("wing", Transform::IDENTITY.rotate(lift));
    posed.pose("head", Transform::IDENTITY.rotate(0.2 * i as f32));
    c.stencil(posed.mask(32, 5), Paint::cel(GREEN.dim(0.5), (-1.0, -1.0), &[(-0.1, GREEN)]).per_cell());
}
```

## `Rig::posed`

```rust
pub fn posed(&self, part: &str) -> Transform
```

The pose of `part`, the identity when unposed or unknown.

## `Rig::place`

```rust
pub fn place(&mut self, t: Transform) -> &mut Self
```

Places the whole figure: where its root sits on the canvas, facing which way,
at what size.

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="img/rig.svg">
  <img src="img/rig-light.svg" alt="Rig, Rig::new, Rig::add, Rig::pose, Rig::place, Rig::mask" width="512">
</picture>

```rust
// One bird, described once; three poses of it, stamped.
let mut body = Path::new();
body.ellipse(0.0, 0.0, 7.0, 4.5);
let mut head = Path::new();
head.ellipse(0.0, 0.0, 3.0, 2.8);
let mut wing = Path::new();
wing.polygon(&[(0.0, 0.0), (8.0, -1.0), (9.0, 2.5), (1.0, 3.0)]);
let mut bird = Rig::new();
bird.add("body", None, Part::new(body));
bird.add("head", Some("body"), Part::new(head).at(Transform::at(6.5, -4.5)));
bird.add("wing", Some("body"), Part::new(wing).at(Transform::at(-3.0, -1.0)));
for (i, lift) in [0.0, -0.5, -1.0].into_iter().enumerate() {
    let mut posed = bird.clone();
    posed.place(Transform::at(12.0 + 20.0 * i as f32, 12.0));
    posed.pose("wing", Transform::IDENTITY.rotate(lift));
    posed.pose("head", Transform::IDENTITY.rotate(0.2 * i as f32));
    c.stencil(posed.mask(32, 5), Paint::cel(GREEN.dim(0.5), (-1.0, -1.0), &[(-0.1, GREEN)]).per_cell());
}
```

## `Rig::placed`

```rust
pub fn placed(&self) -> Transform
```

Where the figure sits; see [`place`](rig.md#rigplace).

## `Rig::part`

```rust
pub fn part(&self, name: &str) -> Option<&Part>
```

The part called `name`.

## `Rig::part_mut`

```rust
pub fn part_mut(&mut self, name: &str) -> Option<&mut Part>
```

The part called `name`, to change. Changing a part's path or resting place
is what a pose cannot do (a beak opening is two paths, [`Path::mix`](path.md#pathmix)ed); the
part is rasterised anew.

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="img/part.svg">
  <img src="img/part-light.svg" alt="Part, Part::new, Part::at, Rig::part_mut" width="384">
</picture>

```rust
// A part is a path about its joint, and where the joint sits on the parent.
let mut arm = Path::new();
arm.round_rect(0.0, -1.5, 12.0, 3.0, 1.5);
let mut figure = Rig::new();
let mut torso = Path::new();
torso.rect(-4.0, -6.0, 8.0, 12.0);
figure.add("torso", None, Part::new(torso));
figure.add("arm", Some("torso"), Part::new(arm).at(Transform::at(4.0, -4.0)));
figure.place(Transform::at(12.0, 8.0));
figure.draw(c, t.panel);
c.disc(16.0, 4.0, 1.0, RED); // the joint
figure.place(Transform::at(34.0, 8.0));
figure.pose("arm", Transform::IDENTITY.rotate(0.9));
figure.part_mut("arm").unwrap().path.ellipse(12.0, 0.0, 2.5, 2.5); // a hand, added to the part
figure.draw(c, PURPLE);
```

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
above it applied; the identity for an unknown name.

## `Rig::point`

```rust
pub fn point(&self, name: &str) -> Point
```

Where the point named `name` is on the canvas, posed. `(0, 0)` for a name
that was never [marked](rig.md#rigmark).

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="img/rig-point.svg">
  <img src="img/rig-point-light.svg" alt="Rig::point, Rig::mark, Rig::draw" width="384">
</picture>

```rust
// A named point follows the pose, so a hat lands wherever the head went.
let mut body = Path::new();
body.ellipse(0.0, 0.0, 6.0, 4.0);
let mut head = Path::new();
head.ellipse(0.0, 0.0, 2.8, 2.8);
let mut figure = Rig::new();
figure.add("body", None, Part::new(body));
figure.add("head", Some("body"), Part::new(head).at(Transform::at(6.0, -4.0)));
figure.mark("crown", "head", (0.0, -2.8));
for (i, nod) in [-0.5, 0.0, 0.5].into_iter().enumerate() {
    figure.place(Transform::at(6.0 + 16.0 * i as f32, 11.0));
    figure.pose("head", Transform::IDENTITY.rotate(nod));
    figure.draw(c, BLUE);
    let (hx, hy) = figure.point("crown");
    c.fill_rect(hx - 2.5, hy - 3.0, 5.0, 3.0, RED); // the hat
}
```

## `Rig::each`

```rust
pub fn each(&self, mut f: impl FnMut(&str, &Path, &Transform))
```

Calls `f` for every part in drawing order with its name, its path and the
transform that puts the path on the canvas.

## `Rig::draw`

```rust
pub fn draw(&self, canvas: &mut Canvas, paint: impl Into<Paint>)
```

Fills every part with `paint`, in drawing order, through the canvas's
current transform. For one silhouette to shade or outline as a whole, use
[`mask`](rig.md#rigmask) and [`Canvas::stencil`](draw.md#canvasstencil) instead.

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="img/rig-point.svg">
  <img src="img/rig-point-light.svg" alt="Rig::point, Rig::mark, Rig::draw" width="384">
</picture>

```rust
// A named point follows the pose, so a hat lands wherever the head went.
let mut body = Path::new();
body.ellipse(0.0, 0.0, 6.0, 4.0);
let mut head = Path::new();
head.ellipse(0.0, 0.0, 2.8, 2.8);
let mut figure = Rig::new();
figure.add("body", None, Part::new(body));
figure.add("head", Some("body"), Part::new(head).at(Transform::at(6.0, -4.0)));
figure.mark("crown", "head", (0.0, -2.8));
for (i, nod) in [-0.5, 0.0, 0.5].into_iter().enumerate() {
    figure.place(Transform::at(6.0 + 16.0 * i as f32, 11.0));
    figure.pose("head", Transform::IDENTITY.rotate(nod));
    figure.draw(c, BLUE);
    let (hx, hy) = figure.point("crown");
    c.fill_rect(hx - 2.5, hy - 3.0, 5.0, 3.0, RED); // the hat
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
  <img src="img/rig-light.svg" alt="Rig, Rig::new, Rig::add, Rig::pose, Rig::place, Rig::mask" width="512">
</picture>

```rust
// One bird, described once; three poses of it, stamped.
let mut body = Path::new();
body.ellipse(0.0, 0.0, 7.0, 4.5);
let mut head = Path::new();
head.ellipse(0.0, 0.0, 3.0, 2.8);
let mut wing = Path::new();
wing.polygon(&[(0.0, 0.0), (8.0, -1.0), (9.0, 2.5), (1.0, 3.0)]);
let mut bird = Rig::new();
bird.add("body", None, Part::new(body));
bird.add("head", Some("body"), Part::new(head).at(Transform::at(6.5, -4.5)));
bird.add("wing", Some("body"), Part::new(wing).at(Transform::at(-3.0, -1.0)));
for (i, lift) in [0.0, -0.5, -1.0].into_iter().enumerate() {
    let mut posed = bird.clone();
    posed.place(Transform::at(12.0 + 20.0 * i as f32, 12.0));
    posed.pose("wing", Transform::IDENTITY.rotate(lift));
    posed.pose("head", Transform::IDENTITY.rotate(0.2 * i as f32));
    c.stencil(posed.mask(32, 5), Paint::cel(GREEN.dim(0.5), (-1.0, -1.0), &[(-0.1, GREEN)]).per_cell());
}
```

[Index](README.md) · [canvas](canvas.md) · [draw](draw.md) · [path](path.md) · [mask](mask.md) · [transform](transform.md) · **rig** · [layer](layer.md) · [bubble](bubble.md) · [font](font.md) · [text](text.md) · [color](color.md) · [render](render.md) · [term](term.md) · [export](export.md) · [ratatui](ratatui.md)

