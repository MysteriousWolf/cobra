# cobra reference

[Index](README.md) · [canvas](canvas.md) · [draw](draw.md) · [path](path.md) · [mask](mask.md) · [transform](transform.md) · [layer](layer.md) · [bubble](bubble.md) · [font](font.md) · [text](text.md) · [color](color.md) · [render](render.md) · [term](term.md) · [export](export.md) · [ratatui](ratatui.md)

Generated from the source by `cargo run --example docs`: every public item with its documentation and, where it draws something, a picture of what it draws and the code that drew it. The pictures follow your colour scheme.

## Pages

| Page | What is in it | Items |
|---|---|---|
| [`canvas`](canvas.md) | The dot grid: cells, dots, colours per dot. | 29 |
| [`draw`](draw.md) | Shapes, strokes, paints, patterns, gradients, shaders and masks. | 66 |
| [`path`](path.md) | Outlines from lines, curves and arcs, filled or stroked. | 22 |
| [`mask`](mask.md) | Shapes as things: bit masks to build, combine, move, and paint through. | 26 |
| [`transform`](transform.md) | Affine transforms: drawing in local coordinates. | 15 |
| [`layer`](layer.md) | Stacked canvases, mattes, and effects around silhouettes. | 43 |
| [`bubble`](bubble.md) | Text boxes and speech bubbles that aim at a speaker. | 29 |
| [`font`](font.md) | Bitmap fonts drawn as dots. | 24 |
| [`text`](text.md) | Real characters on a text layer over the dots. | 32 |
| [`color`](color.md) | RGB, palette and default-foreground colours, depths, palettes. | 16 |
| [`render`](render.md) | Frames for kitty, iTerm2, sixel and plain text. | 15 |
| [`term`](term.md) | What the terminal can do, detected once. | 13 |
| [`export`](export.md) | PNG and SVG files with the terminal's geometry. | 5 |
| [`ratatui`](ratatui.md) | The ratatui widget and overlay. | 3 |

## The crate


Per-dot coloured braille canvases for the terminal.

A braille glyph packs a 2×4 grid of dots into one character cell, but text can only
give the whole cell one colour. `cobra` keeps the braille *model* (a [`Canvas`](canvas.md#canvas) of
individually coloured dots) and picks the best way the current terminal can show it:

| [`Protocol`](term.md#protocol) | How the frame travels | Copyable as braille |
|---|---|---|
| `Kitty`  | zlib RGBA, chunked APC, one image id reused per canvas | with `copy_text` |
| `Iterm2` | PNG in OSC 1337                                         | with `copy_text` |
| `Sixel`  | palettised DCS with transparent background              | with `copy_text` |
| `Text`   | braille glyphs, dominant colour per cell, quantised to the terminal's [`Depth`](color.md#depth) | yes |

Image frames are rasterised on the terminal's real cell grid (queried once from the
terminal), so every dot lands exactly where the font would draw it and the picture
lines up with surrounding text.

```rust
use cobra::{Canvas, Renderer, Rgb, Terminal};

let term = Terminal::detect();                // once, at start-up
let mut renderer = Renderer::new(term);       // owns all scratch buffers
let mut canvas = Canvas::new(20, 5);          // 20×5 cells = 40×20 dots

for x in 0..canvas.width() {
    let hue = x as f32 / canvas.width() as f32;
    canvas.set(x, 10, Rgb::hex(0xff0055).lerp(Rgb::hex(0x00ccff), hue));
}
renderer.render(&canvas, &mut std::io::stdout()).unwrap();
```

#### Detection

[`Terminal::detect`](term.md#terminaldetect) reads `COBRA_PROTOCOL` (`text|kitty|iterm2|sixel`),
`COBRA_CELL` (`WxH` pixels) and `COBRA_COLORS` (`mono|16|256|true`) overrides, checks environment variables, and then
spends one short escape-sequence round trip on `/dev/tty` for what is still unknown
and for the colour scheme. Terminals do not expose font names or point sizes; the
cell box in pixels is what they publish and what alignment needs. The dot diameter
is a style choice, `COBRA_DOT`, that `cargo run --example calibrate` helps pick.
Inside tmux/screen, or when the cell size cannot be learned, the text protocol is
used.

#### Drawing

Besides single dots and Bresenham lines, [`Canvas`](canvas.md#canvas) has span-based vector
primitives (`fill_rect`, `fill_round_rect`, `fill_polygon`, `fill_ellipse`,
`fill_ngon`, `fill_star`, `fill_pie`, `ring`, `arrow`, stroked `rect`,
`round_rect`, `polygon`, `polyline`, `ellipse`, `ngon`, `star`, `arc`, `bezier`,
`spline`) and a [`Path`](path.md#path) of lines, curves and arcs. Every shape takes a
[`Paint`](draw.md#paint): a colour, a dither, a [`Pattern`](draw.md#pattern), a gradient, an edge gradient, cel
bands, a shader or [`Paint::erase`](draw.md#painterase); every stroke takes a [`Pen`](draw.md#pen), optionally
dashed. A [`Mask`](mask.md#mask) is a shape on its own, to combine, move with a [`Transform`](transform.md#transform)
and paint through ([`Canvas::stencil`](draw.md#canvasstencil), [`Canvas::clip`](draw.md#canvasclip), [`Canvas::cut`](draw.md#canvascut),
[`Canvas::clipped`](draw.md#canvasclipped), [`Canvas::effects`](layer.md#canvaseffects)); [`Canvas::with`](canvas.md#canvaswith) draws in local
coordinates. See the [`mask`](mask.md) module.

#### Text

Two kinds, for two jobs. [`Canvas::text`](font.md#canvastext) draws with a [`Font`](font.md#font), a tiny text-format
bitmap font that scales ([`Font::tiny`](font.md#fonttiny) is built in), so a label is dots like
everything else. [`Canvas::print`](text.md#canvasprint) puts a *real character* in a cell — any glyph the
terminal's font has, still copyable, sharp at any size — on a [text layer](text.md)
that every protocol and the exporters understand.

[`Bubble`](bubble.md#bubble) puts the two together: a text box, or a chat bubble with a tail that
[`Bubble::speak`](bubble.md#bubblespeak) aims at a speaker while dodging the rectangles you want kept
clear.

#### Layers

[`Layers`](layer.md#layers) stacks canvases and flattens them into one, bottom to top: what is in
front hides what is behind, and each [`Layer`](layer.md#layer) can carry [`Effect`](layer.md#effect)s around its
silhouette — a drop shadow, an outline, a cleared gap, a glow, a shaded rim, or a
shader of your own. A layer can be offset from the stack and wrap around it, so a
parallax is layers scrolling by different amounts. The result is a plain
[`Canvas`](canvas.md#canvas), which every protocol and exporter takes as usual; in the text
fallback a cell takes the colour of the topmost layer in it. See the [`layer`](layer.md)
module.

#### Colours

Dots take a [`Color`](color.md#color): an explicit [`Rgb`](color.md#rgb), a terminal palette index or the default
foreground. Palette colours follow the user's theme in every protocol: the text
fallback emits SGR indices, and the image protocols resolve them through the
[`Palette`](color.md#palette) that detection reads from the terminal. On terminals without true
colour the text fallback quantises each dot to the nearest colour of the detected
[`Depth`](color.md#depth) (256, 16 or none) before choosing a cell's dominant colour.

#### Export

`export::png` and `export::svg` write a canvas to a file with the same
cell-aligned geometry and a transparent background.

#### Performance

* [`Canvas`](canvas.md#canvas) is a flat `u32` per dot; [`Renderer`](render.md#renderer) and [`Layers`](layer.md#layers) reuse their
  buffers, so steady-state rendering does not allocate.
* Rasterisation is a table lookup per pixel using a mask built once per cell size.
* Frames are mostly transparent flat colour; the built-in zlib encoder exploits that
  (runs, cell-periodic patterns, repeated scanlines) so a kitty/iTerm2 frame is
  usually a few kilobytes. `cargo bench` prints frames per second per protocol.
* One kitty image id per renderer means updates replace in place, without flicker
  or accumulating placements.

#### Features

* `detect` (default) – terminal probing via `libc` on unix.
* `ratatui` – [`ratatui::Braille`](ratatui.md#braille) widget and `ratatui::overlay`.

## Every item

- [`canvas`](canvas.md): [`DOTS_X`](canvas.md#dots_x), [`DOTS_Y`](canvas.md#dots_y), [`Cell`](canvas.md#cell), [`Cell::glyph`](canvas.md#cellglyph), [`Cell::dot`](canvas.md#celldot), [`braille`](canvas.md#braille), [`bayer`](canvas.md#bayer), [`Canvas`](canvas.md#canvas), [`Canvas::new`](canvas.md#canvasnew), [`Canvas::with`](canvas.md#canvaswith), [`Canvas::transform`](canvas.md#canvastransform), [`Canvas::cols`](canvas.md#canvascols), [`Canvas::rows`](canvas.md#canvasrows), [`Canvas::width`](canvas.md#canvaswidth), [`Canvas::height`](canvas.md#canvasheight), [`Canvas::clear`](canvas.md#canvasclear), [`Canvas::set`](canvas.md#canvasset), [`Canvas::set_dithered`](canvas.md#canvasset_dithered), [`Canvas::unset`](canvas.md#canvasunset), [`Canvas::get`](canvas.md#canvasget), [`Canvas::line`](canvas.md#canvasline), [`Canvas::disc`](canvas.md#canvasdisc), [`Canvas::disc_dithered`](canvas.md#canvasdisc_dithered), [`Canvas::clear_disc`](canvas.md#canvasclear_disc), [`Canvas::cell`](canvas.md#canvascell), [`Canvas::cells`](canvas.md#canvascells), [`Canvas::fallback`](canvas.md#canvasfallback), [`Canvas::blit`](canvas.md#canvasblit), [`Canvas::to_text`](canvas.md#canvasto_text)

- [`draw`](draw.md): [`Pattern`](draw.md#pattern), [`Pattern::on`](draw.md#patternon), [`Shader`](draw.md#shader), [`Probe`](draw.md#probe), [`Probe::mix`](draw.md#probemix), [`Probe::lit`](draw.md#probelit), [`Paint`](draw.md#paint), [`Paint::new`](draw.md#paintnew), [`Paint::dithered`](draw.md#paintdithered), [`Paint::erase`](draw.md#painterase), [`Paint::pattern`](draw.md#paintpattern), [`Paint::linear`](draw.md#paintlinear), [`Paint::radial`](draw.md#paintradial), [`Paint::edge`](draw.md#paintedge), [`Paint::cel`](draw.md#paintcel), [`Paint::shader`](draw.md#paintshader), [`Paint::dither`](draw.md#paintdither), [`Paint::hashed`](draw.md#painthashed), [`Paint::per_cell`](draw.md#paintper_cell), [`Paint::soften`](draw.md#paintsoften), [`Paint::anchor`](draw.md#paintanchor), [`Paint::color`](draw.md#paintcolor), [`Paint::coverage`](draw.md#paintcoverage), [`Pen`](draw.md#pen), [`Pen::new`](draw.md#pennew), [`Pen::dash`](draw.md#pendash), [`Pen::dotted`](draw.md#pendotted), [`Pen::phase`](draw.md#penphase), [`Point`](draw.md#point), [`Rect`](draw.md#rect), [`Rect::new`](draw.md#rectnew), [`Rect::around`](draw.md#rectaround), [`Rect::right`](draw.md#rectright), [`Rect::bottom`](draw.md#rectbottom), [`Rect::center`](draw.md#rectcenter), [`Rect::contains`](draw.md#rectcontains), [`Rect::inset`](draw.md#rectinset), [`Rect::offset`](draw.md#rectoffset), [`Rect::overlap`](draw.md#rectoverlap), [`Canvas::span`](draw.md#canvasspan), [`Canvas::stencil`](draw.md#canvasstencil), [`Canvas::stencil_in`](draw.md#canvasstencil_in), [`Canvas::clip`](draw.md#canvasclip), [`Canvas::cut`](draw.md#canvascut), [`Canvas::clipped`](draw.md#canvasclipped), [`Canvas::fill_rect`](draw.md#canvasfill_rect), [`Canvas::rect`](draw.md#canvasrect), [`Canvas::fill_ellipse`](draw.md#canvasfill_ellipse), [`Canvas::ellipse`](draw.md#canvasellipse), [`Canvas::arc`](draw.md#canvasarc), [`Canvas::polyline`](draw.md#canvaspolyline), [`Canvas::polygon`](draw.md#canvaspolygon), [`Canvas::fill_polygon`](draw.md#canvasfill_polygon), [`Canvas::fill_path`](draw.md#canvasfill_path), [`Canvas::stroke_path`](draw.md#canvasstroke_path), [`Canvas::bezier`](draw.md#canvasbezier), [`Canvas::spline`](draw.md#canvasspline), [`Canvas::fill_round_rect`](draw.md#canvasfill_round_rect), [`Canvas::round_rect`](draw.md#canvasround_rect), [`Canvas::ring`](draw.md#canvasring), [`Canvas::fill_pie`](draw.md#canvasfill_pie), [`Canvas::fill_ngon`](draw.md#canvasfill_ngon), [`Canvas::ngon`](draw.md#canvasngon), [`Canvas::fill_star`](draw.md#canvasfill_star), [`Canvas::star`](draw.md#canvasstar), [`Canvas::arrow`](draw.md#canvasarrow)

- [`path`](path.md): [`Path`](path.md#path), [`Path::new`](path.md#pathnew), [`Path::is_empty`](path.md#pathis_empty), [`Path::current`](path.md#pathcurrent), [`Path::move_to`](path.md#pathmove_to), [`Path::line_to`](path.md#pathline_to), [`Path::quad_to`](path.md#pathquad_to), [`Path::cubic_to`](path.md#pathcubic_to), [`Path::arc_to`](path.md#patharc_to), [`Path::curve_through`](path.md#pathcurve_through), [`Path::close`](path.md#pathclose), [`Path::rect`](path.md#pathrect), [`Path::round_rect`](path.md#pathround_rect), [`Path::apply`](path.md#pathapply), [`Path::ellipse`](path.md#pathellipse), [`Path::polygon`](path.md#pathpolygon), [`Path::transform`](path.md#pathtransform), [`Path::translate`](path.md#pathtranslate), [`Path::scale`](path.md#pathscale), [`Path::rotate`](path.md#pathrotate), [`Path::bounds`](path.md#pathbounds), [`Path::subpaths`](path.md#pathsubpaths)

- [`mask`](mask.md): [`Silhouette`](mask.md#silhouette), [`Mask`](mask.md#mask), [`Mask::new`](mask.md#masknew), [`Mask::of`](mask.md#maskof), [`Mask::cols`](mask.md#maskcols), [`Mask::rows`](mask.md#maskrows), [`Mask::width`](mask.md#maskwidth), [`Mask::height`](mask.md#maskheight), [`Mask::contains`](mask.md#maskcontains), [`Mask::set`](mask.md#maskset), [`Mask::unset`](mask.md#maskunset), [`Mask::clear`](mask.md#maskclear), [`Mask::is_empty`](mask.md#maskis_empty), [`Mask::len`](mask.md#masklen), [`Mask::bounds`](mask.md#maskbounds), [`Mask::dots`](mask.md#maskdots), [`Mask::draw`](mask.md#maskdraw), [`Mask::erase`](mask.md#maskerase), [`Mask::union`](mask.md#maskunion), [`Mask::subtract`](mask.md#masksubtract), [`Mask::intersect`](mask.md#maskintersect), [`Mask::boundary_with`](mask.md#maskboundary_with), [`Mask::translate`](mask.md#masktranslate), [`Mask::flip_x`](mask.md#maskflip_x), [`Mask::flip_y`](mask.md#maskflip_y), [`Mask::transform`](mask.md#masktransform)

- [`transform`](transform.md): [`Transform`](transform.md#transform), [`Transform::IDENTITY`](transform.md#transformidentity), [`Transform::at`](transform.md#transformat), [`Transform::is_identity`](transform.md#transformis_identity), [`Transform::is_axis_aligned`](transform.md#transformis_axis_aligned), [`Transform::scale_factor`](transform.md#transformscale_factor), [`Transform::apply`](transform.md#transformapply), [`Transform::then`](transform.md#transformthen), [`Transform::inverse`](transform.md#transforminverse), [`Transform::translate`](transform.md#transformtranslate), [`Transform::scale`](transform.md#transformscale), [`Transform::flip_x`](transform.md#transformflip_x), [`Transform::flip_y`](transform.md#transformflip_y), [`Transform::rotate`](transform.md#transformrotate), [`Transform::rotate_about`](transform.md#transformrotate_about)

- [`layer`](layer.md): [`Sample`](layer.md#sample), [`Sample::inside`](layer.md#sampleinside), [`Sample::layer`](layer.md#samplelayer), [`Sample::lit`](layer.md#samplelit), [`Sample::covered`](layer.md#samplecovered), [`Effect`](layer.md#effect), [`Effect::shadow`](layer.md#effectshadow), [`Effect::outline`](layer.md#effectoutline), [`Effect::glow`](layer.md#effectglow), [`Effect::gap`](layer.md#effectgap), [`Effect::rim`](layer.md#effectrim), [`Effect::paint`](layer.md#effectpaint), [`Effect::shader`](layer.md#effectshader), [`Layer`](layer.md#layer), [`Layer::effect`](layer.md#layereffect), [`Layer::scroll`](layer.md#layerscroll), [`Layer::canvas`](layer.md#layercanvas), [`Layer::canvas_mut`](layer.md#layercanvas_mut), [`Layers`](layer.md#layers), [`Layers::new`](layer.md#layersnew), [`Layers::cols`](layer.md#layerscols), [`Layers::rows`](layer.md#layersrows), [`Layers::width`](layer.md#layerswidth), [`Layers::height`](layer.md#layersheight), [`Layers::len`](layer.md#layerslen), [`Layers::is_empty`](layer.md#layersis_empty), [`Layers::push`](layer.md#layerspush), [`Layers::insert`](layer.md#layersinsert), [`Layers::remove`](layer.md#layersremove), [`Layers::swap`](layer.md#layersswap), [`Layers::get`](layer.md#layersget), [`Layers::get_mut`](layer.md#layersget_mut), [`Layers::iter`](layer.md#layersiter), [`Layers::iter_mut`](layer.md#layersiter_mut), [`Layers::clear`](layer.md#layersclear), [`Layers::flat`](layer.md#layersflat), [`Layers::flatten`](layer.md#layersflatten), [`Field`](layer.md#field), [`Field::new`](layer.md#fieldnew), [`Field::effects`](layer.md#fieldeffects), [`Field::effects_in`](layer.md#fieldeffects_in), [`Canvas::effects`](layer.md#canvaseffects), [`Canvas::effects_in`](layer.md#canvaseffects_in)

- [`bubble`](bubble.md): [`Side`](bubble.md#side), [`Shape`](bubble.md#shape), [`TailKind`](bubble.md#tailkind), [`Tail`](bubble.md#tail), [`Tail::new`](bubble.md#tailnew), [`Tail::len`](bubble.md#taillen), [`Tail::width`](bubble.md#tailwidth), [`Bubble`](bubble.md#bubble), [`Bubble::new`](bubble.md#bubblenew), [`Bubble::speech`](bubble.md#bubblespeech), [`Bubble::thought`](bubble.md#bubblethought), [`Bubble::shout`](bubble.md#bubbleshout), [`Bubble::whisper`](bubble.md#bubblewhisper), [`Bubble::shape`](bubble.md#bubbleshape), [`Bubble::tail`](bubble.md#bubbletail), [`Bubble::no_tail`](bubble.md#bubbleno_tail), [`Bubble::fill`](bubble.md#bubblefill), [`Bubble::border`](bubble.md#bubbleborder), [`Bubble::ink`](bubble.md#bubbleink), [`Bubble::font`](bubble.md#bubblefont), [`Bubble::pad`](bubble.md#bubblepad), [`Bubble::align`](bubble.md#bubblealign), [`Bubble::wrap`](bubble.md#bubblewrap), [`Bubble::clear_behind`](bubble.md#bubbleclear_behind), [`Bubble::size`](bubble.md#bubblesize), [`Bubble::bounds`](bubble.md#bubblebounds), [`Bubble::draw`](bubble.md#bubbledraw), [`Bubble::speak`](bubble.md#bubblespeak), [`Bubble::place`](bubble.md#bubbleplace)

- [`font`](font.md): [`MAX_GLYPH_WIDTH`](font.md#max_glyph_width), [`Font`](font.md#font), [`Glyph`](font.md#glyph), [`Glyph::dot`](font.md#glyphdot), [`Glyph::row`](font.md#glyphrow), [`FontError`](font.md#fonterror), [`Font::empty`](font.md#fontempty), [`Font::tiny`](font.md#fonttiny), [`Font::parse`](font.md#fontparse), [`Font::add`](font.md#fontadd), [`Font::glyph`](font.md#fontglyph), [`Font::height`](font.md#fontheight), [`Font::spacing`](font.md#fontspacing), [`Font::line_gap`](font.md#fontline_gap), [`Font::line_height`](font.md#fontline_height), [`Font::len`](font.md#fontlen), [`Font::is_empty`](font.md#fontis_empty), [`Font::with_spacing`](font.md#fontwith_spacing), [`Font::with_line_gap`](font.md#fontwith_line_gap), [`Font::advance`](font.md#fontadvance), [`Font::measure`](font.md#fontmeasure), [`Font::scale`](font.md#fontscale), [`Font::scale_xy`](font.md#fontscale_xy), [`Canvas::text`](font.md#canvastext)

- [`text`](text.md): [`Attrs`](text.md#attrs), [`Attrs::NONE`](text.md#attrsnone), [`Attrs::BOLD`](text.md#attrsbold), [`Attrs::DIM`](text.md#attrsdim), [`Attrs::ITALIC`](text.md#attrsitalic), [`Attrs::UNDERLINE`](text.md#attrsunderline), [`Attrs::REVERSE`](text.md#attrsreverse), [`Attrs::has`](text.md#attrshas), [`TextStyle`](text.md#textstyle), [`TextStyle::new`](text.md#textstylenew), [`TextStyle::on`](text.md#textstyleon), [`TextStyle::with`](text.md#textstylewith), [`TextStyle::bold`](text.md#textstylebold), [`TextStyle::dim`](text.md#textstyledim), [`TextStyle::italic`](text.md#textstyleitalic), [`TextStyle::underline`](text.md#textstyleunderline), [`TextCell`](text.md#textcell), [`TextCell::CONTINUATION`](text.md#textcellcontinuation), [`TextCell::is_empty`](text.md#textcellis_empty), [`TextCell::is_continuation`](text.md#textcellis_continuation), [`Align`](text.md#align), [`char_width`](text.md#char_width), [`width`](text.md#width), [`measure`](text.md#measure), [`wrap`](text.md#wrap), [`Wrap`](text.md#wrap), [`Canvas::print`](text.md#canvasprint), [`Canvas::print_wrapped`](text.md#canvasprint_wrapped), [`Canvas::text_cell`](text.md#canvastext_cell), [`Canvas::erase_text`](text.md#canvaserase_text), [`Canvas::clear_text`](text.md#canvasclear_text), [`Canvas::has_text`](text.md#canvashas_text)

- [`color`](color.md): [`Rgb`](color.md#rgb), [`Rgb::new`](color.md#rgbnew), [`Rgb::hex`](color.md#rgbhex), [`Rgb::lerp`](color.md#rgblerp), [`Rgb::dim`](color.md#rgbdim), [`Rgb::luminance`](color.md#rgbluminance), [`Rgb::contrast`](color.md#rgbcontrast), [`Color`](color.md#color), [`Color::resolve`](color.md#colorresolve), [`Depth`](color.md#depth), [`Depth::parse`](color.md#depthparse), [`Depth::from_env`](color.md#depthfrom_env), [`Color::quantize`](color.md#colorquantize), [`Palette`](color.md#palette), [`Palette::is_light`](color.md#paletteis_light), [`Palette::nearest_ansi`](color.md#palettenearest_ansi)

- [`render`](render.md): [`Options`](render.md#options), [`Options::from_env`](render.md#optionsfrom_env), [`Placement`](render.md#placement), [`Renderer`](render.md#renderer), [`Renderer::new`](render.md#renderernew), [`Renderer::with_options`](render.md#rendererwith_options), [`Renderer::terminal`](render.md#rendererterminal), [`Renderer::options`](render.md#rendereroptions), [`Renderer::set_options`](render.md#rendererset_options), [`Renderer::image_id`](render.md#rendererimage_id), [`Renderer::invalidate`](render.md#rendererinvalidate), [`Renderer::render`](render.md#rendererrender), [`Renderer::render_at`](render.md#rendererrender_at), [`Renderer::encode`](render.md#rendererencode), [`Renderer::encode_view`](render.md#rendererencode_view)

- [`term`](term.md): [`Protocol`](term.md#protocol), [`Protocol::parse`](term.md#protocolparse), [`Protocol::from_env`](term.md#protocolfrom_env), [`CellSize`](term.md#cellsize), [`CellSize::is_known`](term.md#cellsizeis_known), [`CellSize::parse`](term.md#cellsizeparse), [`Terminal`](term.md#terminal), [`Terminal::text`](term.md#terminaltext), [`Terminal::new`](term.md#terminalnew), [`Terminal::with_depth`](term.md#terminalwith_depth), [`Terminal::with_palette`](term.md#terminalwith_palette), [`Terminal::is_graphical`](term.md#terminalis_graphical), [`Terminal::detect`](term.md#terminaldetect)

- [`export`](export.md): [`Style`](export.md#style), [`Style::scale`](export.md#stylescale), [`png`](export.md#png), [`svg`](export.md#svg), [`resolve`](export.md#resolve)

- [`ratatui`](ratatui.md): [`Braille`](ratatui.md#braille), [`Braille::new`](ratatui.md#braillenew), [`overlay`](ratatui.md#overlay)

## Without a picture

Every other item is illustrated. These are the plumbing between a canvas and a terminal, where a drawing would show a canvas and say nothing about the item:

- [`Options`](render.md#options): how a frame is sent, not what is in it
- [`Options::from_env`](render.md#optionsfrom_env): reads `COBRA_DOT`
- [`Placement`](render.md#placement): where the cursor is, which a file does not have
- [`Renderer::new`](render.md#renderernew): constructs a renderer; see `Renderer` for what one draws
- [`Renderer::with_options`](render.md#rendererwith_options): constructs a renderer
- [`Renderer::terminal`](render.md#rendererterminal): a getter
- [`Renderer::options`](render.md#rendereroptions): a getter
- [`Renderer::set_options`](render.md#rendererset_options): a setter
- [`Renderer::image_id`](render.md#rendererimage_id): a kitty protocol detail
- [`Renderer::invalidate`](render.md#rendererinvalidate): forgets the last frame sent
- [`Renderer::render`](render.md#rendererrender): writes a frame to a terminal; `Renderer` shows what it looks like
- [`Renderer::render_at`](render.md#rendererrender_at): writes a frame at a cursor position
- [`Renderer::encode`](render.md#rendererencode): the bytes of a frame
- [`Renderer::encode_view`](render.md#rendererencode_view): the bytes of part of a frame
- [`Protocol`](term.md#protocol): which escape sequences a terminal speaks
- [`Protocol::parse`](term.md#protocolparse): parses a name
- [`Protocol::from_env`](term.md#protocolfrom_env): reads environment variables
- [`CellSize`](term.md#cellsize): pixels per cell, a property of the terminal
- [`CellSize::is_known`](term.md#cellsizeis_known): a predicate
- [`CellSize::parse`](term.md#cellsizeparse): parses `WxH`
- [`Terminal`](term.md#terminal): what detection learned about the terminal
- [`Terminal::text`](term.md#terminaltext): a constructor
- [`Terminal::new`](term.md#terminalnew): a constructor
- [`Terminal::with_depth`](term.md#terminalwith_depth): a builder; `Depth` shows what each depth looks like
- [`Terminal::with_palette`](term.md#terminalwith_palette): a builder
- [`Terminal::is_graphical`](term.md#terminalis_graphical): a predicate
- [`Terminal::detect`](term.md#terminaldetect): talks to the terminal
- [`Braille`](ratatui.md#braille): draws into a ratatui buffer, which needs a terminal
- [`Braille::new`](ratatui.md#braillenew): constructs the widget
- [`overlay`](ratatui.md#overlay): sends the image after a ratatui frame
