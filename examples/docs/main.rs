//! Generates `docs/`: one Markdown page per module with every public item, its
//! signature, its documentation from the source, and a picture of what it draws.
//!
//! ```text
//! cargo run --example docs                    # (re)write docs/
//! cargo run --example docs -- --check         # exit 1 if docs/ is out of date
//! cargo run --example docs -- --preview DIR   # also write every picture as a PNG into DIR
//! cargo run --example docs -- --missing       # list the public items that have no picture
//! ```

mod demos;
mod parse;

use std::collections::{BTreeMap, HashMap};
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

use cobra::{Canvas, export};
use demos::{Demo, Theme};
use parse::{Item, Kind, Module};

/// The pages, in reading order: the module name, its source, and a line for the index.
const PAGES: &[(&str, &str, &str)] = &[
    ("canvas", "src/canvas.rs", "The dot grid: cells, dots, colours per dot."),
    ("draw", "src/draw.rs", "Shapes, strokes, paints, patterns, gradients, shaders and masks."),
    ("path", "src/path.rs", "Outlines from lines, curves and arcs, filled or stroked."),
    ("mask", "src/mask.rs", "Shapes as things: bit masks to build, combine, move, and paint through."),
    ("transform", "src/transform.rs", "Affine transforms: drawing in local coordinates."),
    ("rig", "src/rig.rs", "Figures as parts in a tree: pose them, name points on them, rasterise once."),
    ("layer", "src/layer.rs", "Stacked canvases, mattes, and effects around silhouettes."),
    ("bubble", "src/bubble.rs", "Text boxes and speech bubbles that aim at a speaker."),
    ("font", "src/font.rs", "Bitmap fonts drawn as dots."),
    ("text", "src/text.rs", "Real characters on a text layer over the dots."),
    ("color", "src/color.rs", "RGB, palette and default-foreground colours, depths, palettes."),
    ("render", "src/render/mod.rs", "Frames for kitty, iTerm2, sixel and plain text."),
    ("term", "src/term/mod.rs", "What the terminal can do, detected once."),
    ("export", "src/export.rs", "PNG and SVG files with the terminal's geometry."),
    ("ratatui", "src/ratatui.rs", "The ratatui widget and overlay."),
];

/// Public items that have no picture, and why: everything else must have one, and
/// `--check` fails when it does not. Each is plumbing between a canvas and a
/// terminal, so a drawing could only show a canvas, which says nothing about it.
const NO_PICTURE: &[(&str, &str)] = &[
    ("Options", "how a frame is sent, not what is in it"),
    ("Options::from_env", "reads `COBRA_DOT`"),
    ("Placement", "where the cursor is, which a file does not have"),
    ("Renderer::new", "constructs a renderer; see `Renderer` for what one draws"),
    ("Renderer::with_options", "constructs a renderer"),
    ("Renderer::terminal", "a getter"),
    ("Renderer::options", "a getter"),
    ("Renderer::set_options", "a setter"),
    ("Renderer::image_id", "a kitty protocol detail"),
    ("Renderer::invalidate", "forgets the last frame sent"),
    ("Renderer::render", "writes a frame to a terminal; `Renderer` shows what it looks like"),
    ("Renderer::render_at", "writes a frame at a cursor position"),
    ("Renderer::encode", "the bytes of a frame"),
    ("Renderer::encode_view", "the bytes of part of a frame"),
    ("Protocol", "which escape sequences a terminal speaks"),
    ("Protocol::parse", "parses a name"),
    ("Protocol::from_env", "reads environment variables"),
    ("CellSize", "pixels per cell, a property of the terminal"),
    ("CellSize::is_known", "a predicate"),
    ("CellSize::parse", "parses `WxH`"),
    ("Terminal", "what detection learned about the terminal"),
    ("Terminal::text", "a constructor"),
    ("Terminal::new", "a constructor"),
    ("Terminal::with_depth", "a builder; `Depth` shows what each depth looks like"),
    ("Terminal::with_palette", "a builder"),
    ("Terminal::with_passthrough", "a builder; wraps images for tmux"),
    ("Rig::posed", "a getter"),
    ("Rig::placed", "a getter"),
    ("Rig::part", "a getter"),
    ("Rig::parts", "lists names"),
    ("Rig::world", "a transform, which `Rig::point` shows applied"),
    ("Rig::each", "walks the parts; `Rig::draw` is it with a fill"),
    ("Terminal::is_graphical", "a predicate"),
    ("Terminal::detect", "talks to the terminal"),
    ("Braille", "draws into a ratatui buffer, which needs a terminal"),
    ("Braille::new", "constructs the widget"),
    ("overlay", "sends the image after a ratatui frame"),
];

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let check = args.iter().any(|a| a == "--check");
    let missing = args.iter().any(|a| a == "--missing");
    let preview = args.iter().position(|a| a == "--preview").and_then(|i| args.get(i + 1)).map(PathBuf::from);
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let out = root.join("docs");
    let modules: Vec<Module> = PAGES.iter().map(|(name, path, _)| parse::module(name, root.join(path))).collect();
    let crate_docs = parse::module("cobra", root.join("src/lib.rs")).docs;
    let demos = demos::all();
    let sources = demo_sources(&fs::read_to_string(root.join("examples/docs/demos.rs")).unwrap());
    for d in &demos {
        assert!(sources.contains_key(d.name()), "no source found for demo {:?}", d.name());
    }

    // Every public item has a picture or a reason not to.
    let mut uncovered = Vec::new();
    for m in &modules {
        for item in &m.items {
            let key = item.key();
            let pictured = demos.iter().any(|d| d.items.contains(&key.as_str()));
            let excused = NO_PICTURE.iter().any(|(k, _)| *k == key);
            if pictured && excused {
                panic!("{key} has a picture and is listed in NO_PICTURE");
            }
            if !pictured && !excused {
                uncovered.push(format!("{}: {key}", m.name));
            }
        }
    }
    for (key, _) in NO_PICTURE {
        assert!(
            modules.iter().any(|m| m.items.iter().any(|i| i.key() == *key)),
            "NO_PICTURE lists {key}, which is not an item"
        );
    }
    if missing {
        for line in &uncovered {
            println!("{line}");
        }
        println!("{} items without a picture", uncovered.len());
        return;
    }
    if !uncovered.is_empty() {
        eprintln!("every public item needs a picture in examples/docs/demos.rs, or a reason in NO_PICTURE:");
        for line in &uncovered {
            eprintln!("  {line}");
        }
        std::process::exit(1);
    }

    let index = anchors(&modules);
    let mut files: BTreeMap<PathBuf, Vec<u8>> = BTreeMap::new();
    let mut used = std::collections::HashSet::new();
    for module in &modules {
        let (page, images) = render_page(module, &index, &demos, &sources, &mut used);
        files.insert(PathBuf::from(format!("{}.md", module.name)), page.into_bytes());
        for (name, svg) in images {
            files.insert(PathBuf::from("img").join(name), svg.into_bytes());
        }
    }
    for d in &demos {
        for item in d.items {
            assert!(used.contains(item), "demo {:?} illustrates {item:?}, which the parser did not find", d.name());
        }
    }
    files.insert(PathBuf::from("README.md"), render_index(&crate_docs, &modules, &index).into_bytes());
    if let Some(dir) = preview {
        fs::create_dir_all(&dir).unwrap();
        for d in &demos {
            for (theme, suffix) in [(demos::DARK, ""), (demos::LIGHT, "-light")] {
                let mut canvas = Canvas::new(d.cols, d.rows);
                (d.draw)(&mut canvas, theme);
                let style = export_style(theme).scale(2);
                let name = format!("{}{suffix}.png", anchor(&d.name().replace("::", "-")));
                fs::write(dir.join(name), export::png(&canvas, &style)).unwrap();
            }
        }
        for (theme, suffix) in [(demos::DARK, ""), (demos::LIGHT, "-light")] {
            for (n, sheet) in sheets(&demos, theme).into_iter().enumerate() {
                let style = export_style(theme).scale(2);
                fs::write(dir.join(format!("sheet{n}{suffix}.png")), export::png(&sheet, &style)).unwrap();
            }
        }
    }

    if check {
        let mut stale = Vec::new();
        for (path, content) in &files {
            if fs::read(out.join(path)).ok().as_deref() != Some(content.as_slice()) {
                stale.push(path.display().to_string());
            }
        }
        for path in existing(&out) {
            if !files.contains_key(&path) {
                stale.push(format!("{} (stale)", path.display()));
            }
        }
        if stale.is_empty() {
            println!("docs/ is up to date");
        } else {
            eprintln!("docs/ is out of date; run `cargo run --example docs`:");
            for s in stale {
                eprintln!("  {s}");
            }
            std::process::exit(1);
        }
        return;
    }
    for path in existing(&out) {
        if !files.contains_key(&path) {
            fs::remove_file(out.join(&path)).unwrap();
        }
    }
    fs::create_dir_all(out.join("img")).unwrap();
    for (path, content) in &files {
        fs::write(out.join(path), content).unwrap();
    }
    println!("wrote {} files to {}", files.len(), out.display());
}

/// Contact sheets of every demo, two per row with a label, for looking at them all
/// at once.
fn sheets(demos: &[Demo], theme: Theme) -> Vec<Canvas> {
    const PER_SHEET: usize = 24;
    const COL_W: u16 = 50;
    demos
        .chunks(PER_SHEET)
        .map(|chunk| {
            let rows: u16 = chunk.chunks(2).map(|pair| pair.iter().map(|d| d.rows).max().unwrap_or(0) + 2).sum();
            let mut sheet = Canvas::new(COL_W * 2, rows);
            let mut y = 0u16;
            for pair in chunk.chunks(2) {
                for (k, d) in pair.iter().enumerate() {
                    let (x0, y0) = (k as u16 * COL_W, y + 1);
                    sheet.print(x0 as i32, y as i32, &d.items.join(" | "), theme.ink);
                    let mut canvas = Canvas::new(d.cols, d.rows);
                    (d.draw)(&mut canvas, theme);
                    sheet.blit(&canvas, x0 as i32 * 2, y0 as i32 * 4);
                }
                y += pair.iter().map(|d| d.rows).max().unwrap_or(0) + 2;
            }
            sheet
        })
        .collect()
}

/// Every file under `docs/`, relative to it.
fn existing(out: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let mut stack = vec![out.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = fs::read_dir(&dir) else { continue };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else {
                found.push(path.strip_prefix(out).unwrap().to_path_buf());
            }
        }
    }
    found
}

/// Where every item is linked to: `Canvas::fill_rect` → `draw.md#canvasfill_rect`.
fn anchors(modules: &[Module]) -> HashMap<String, String> {
    let mut map = HashMap::new();
    for m in modules {
        map.insert(m.name.clone(), format!("{}.md", m.name));
        for item in &m.items {
            map.insert(item.key(), format!("{}.md#{}", m.name, anchor(&item.key())));
            for member in &item.members {
                let key = format!("{}::{}", item.key(), member.name);
                map.insert(key, format!("{}.md#{}", m.name, anchor(&item.key())));
            }
        }
    }
    map
}

/// GitHub's anchor for a heading.
fn anchor(heading: &str) -> String {
    heading
        .to_lowercase()
        .chars()
        .filter_map(|c| match c {
            ' ' => Some('-'),
            c if c.is_alphanumeric() || c == '-' || c == '_' => Some(c),
            _ => None,
        })
        .collect()
}

/// The block of each demo, as written in `demos.rs`, keyed by the first item it
/// illustrates.
fn demo_sources(src: &str) -> HashMap<String, String> {
    let mut out = HashMap::new();
    let mut rest = src;
    while let Some(k) = rest.find("=> |c, t| {") {
        // The head is everything since the previous block: the item literals and
        // the size. The first literal names the demo.
        let head = &rest[..k];
        let head = &head[head.rfind('}').map_or(0, |i| i + 1)..];
        let item = head.split('"').nth(1).filter(|s| !s.is_empty());
        let Some(item) = item else { break };
        let body_start = k + "=> |c, t| {".len();
        let mut depth = 1;
        let mut end = body_start;
        for (i, ch) in rest[body_start..].char_indices() {
            depth += (ch == '{') as i32 - (ch == '}') as i32;
            if depth == 0 {
                end = body_start + i;
                break;
            }
        }
        let body = dedent(&rest[body_start..end]);
        out.insert(item.to_string(), body);
        rest = &rest[end..];
    }
    out
}

/// Strips the common indentation and surrounding blank lines of a block.
fn dedent(block: &str) -> String {
    let lines: Vec<&str> = block.lines().collect();
    let indent = lines.iter().filter(|l| !l.trim().is_empty()).map(|l| l.len() - l.trim_start().len()).min();
    let indent = indent.unwrap_or(0);
    let body: Vec<&str> = lines.iter().map(|l| if l.len() >= indent { &l[indent..] } else { l.trim() }).collect();
    body.join("\n").trim_matches('\n').to_string()
}

/// One page, and the images it embeds.
fn render_page(
    module: &Module,
    index: &HashMap<String, String>,
    demos: &[Demo],
    sources: &HashMap<String, String>,
    used: &mut std::collections::HashSet<&'static str>,
) -> (String, Vec<(String, String)>) {
    let mut page = String::new();
    let mut images = Vec::new();
    let _ = writeln!(page, "# `{}`\n", module.name);
    page.push_str(&nav(Some(&module.name)));
    page.push_str(&docs_md(&module.docs, index, None));
    page.push_str("\n\n");

    // Types first, then the free functions and constants, then the methods of each
    // type in the order the types were met.
    let types: Vec<&Item> = module.items.iter().filter(|i| i.owner.is_none() && i.kind != Kind::Fn).collect();
    let free: Vec<&Item> = module.items.iter().filter(|i| i.owner.is_none() && i.kind == Kind::Fn).collect();
    let mut owners: Vec<String> = Vec::new();
    for item in module.items.iter().filter(|i| i.owner.is_some()) {
        let o = item.owner.clone().unwrap();
        if !owners.contains(&o) {
            owners.push(o);
        }
    }
    page.push_str("## Contents\n\n");
    for item in types.iter().chain(&free) {
        let _ = writeln!(page, "- [`{}`](#{})", item.key(), anchor(&item.key()));
    }
    for o in &owners {
        let methods: Vec<String> =
            module.items.iter().filter(|i| i.owner.as_deref() == Some(o)).map(|i| link(&i.key(), index)).collect();
        let _ = writeln!(page, "- `{o}`: {}", methods.join(", "));
    }
    page.push('\n');

    let mut show = |page: &mut String, images: &mut Vec<(String, String)>, demo: &Demo| {
        used.extend(demo.items);
        let slug = anchor(&demo.name().replace("::", "-"));
        let (dark, light) = (format!("{slug}.svg"), format!("{slug}-light.svg"));
        images.push((dark.clone(), picture(demo, demos::DARK)));
        images.push((light.clone(), picture(demo, demos::LIGHT)));
        let width = demo.cols as u32 * 16;
        let _ = writeln!(
            page,
            "<picture>\n  <source media=\"(prefers-color-scheme: dark)\" srcset=\"img/{dark}\">\n  <img src=\"img/{light}\" alt=\"{}\" width=\"{width}\">\n</picture>\n",
            demo.items.join(", ")
        );
        let _ = writeln!(page, "```rust\n{}\n```\n", sources[demo.name()]);
    };
    let mut section = |page: &mut String, item: &Item, images: &mut Vec<(String, String)>| {
        let key = item.key();
        let _ = writeln!(page, "## `{key}`\n");
        let _ = writeln!(page, "```rust\n{}\n```\n", item.sig);
        page.push_str(&docs_md(&item.docs, index, item.owner.as_deref().or(Some(&item.name))));
        page.push_str("\n\n");
        if !item.members.is_empty() {
            for m in &item.members {
                let indent = "  ".repeat(m.depth);
                let docs = docs_md(&m.docs, index, Some(&item.name)).replace('\n', " ");
                let _ = writeln!(page, "{indent}- `{}`{}{}", m.sig, if docs.is_empty() { "" } else { " — " }, docs);
            }
            page.push('\n');
        }
        for demo in demos.iter().filter(|d| d.items.contains(&key.as_str())) {
            show(page, images, demo);
        }
        for m in &item.members {
            let member = format!("{key}::{}", m.name);
            for demo in demos.iter().filter(|d| d.items.contains(&member.as_str())) {
                let _ = writeln!(page, "**`{}`**\n", m.name);
                show(page, images, demo);
            }
        }
    };
    for item in types.iter().chain(&free) {
        section(&mut page, item, &mut images);
    }
    for o in &owners {
        let _ = writeln!(page, "## `{o}` methods\n");
        for item in module.items.iter().filter(|i| i.owner.as_deref() == Some(o)) {
            section(&mut page, item, &mut images);
        }
    }
    page.push_str(&nav(Some(&module.name)));
    (page, images)
}

/// Renders a demo as an SVG for one theme.
fn picture(demo: &Demo, theme: Theme) -> String {
    let mut canvas = Canvas::new(demo.cols, demo.rows);
    (demo.draw)(&mut canvas, theme);
    export::svg(&canvas, &export_style(theme))
}

/// The export geometry and colours for one theme.
fn export_style(theme: Theme) -> export::Style {
    let mut style = export::Style { background: Some(theme.bg), dot_size: 0.85, ..export::Style::default() };
    style.palette.foreground = theme.ink;
    style.palette.background = theme.bg;
    style
}

/// The index page: the crate docs and the list of pages.
fn render_index(crate_docs: &str, modules: &[Module], index: &HashMap<String, String>) -> String {
    let mut page = String::from("# cobra reference\n\n");
    page.push_str(&nav(None));
    page.push_str(
        "Generated from the source by `cargo run --example docs`: every public item with its \
         documentation and, where it draws something, a picture of what it draws and the code \
         that drew it. The pictures follow your colour scheme.\n\n",
    );
    page.push_str("## Pages\n\n| Page | What is in it | Items |\n|---|---|---|\n");
    for (m, (_, _, blurb)) in modules.iter().zip(PAGES) {
        let _ = writeln!(page, "| [`{}`]({}.md) | {blurb} | {} |", m.name, m.name, m.items.len());
    }
    page.push_str("\n## The crate\n\n");
    let crate_docs = crate_docs.trim_start_matches("# cobra\n");
    page.push_str(&docs_md(crate_docs, index, None));
    page.push_str("\n\n## Every item\n\n");
    for m in modules {
        let items: Vec<String> = m.items.iter().map(|i| link(&i.key(), index)).collect();
        let _ = writeln!(page, "- [`{}`]({}.md): {}\n", m.name, m.name, items.join(", "));
    }
    page.push_str(
        "## Without a picture\n\nEvery other item is illustrated. These are the plumbing between a \
         canvas and a terminal, where a drawing would show a canvas and say nothing about the item:\n\n",
    );
    for (key, why) in NO_PICTURE {
        let _ = writeln!(page, "- {}: {why}", link(key, index));
    }
    page
}

/// The line of links at the top and bottom of every page.
fn nav(current: Option<&str>) -> String {
    let mut s = String::from("[Index](README.md)");
    for (name, _, _) in PAGES {
        if current == Some(name) {
            let _ = write!(s, " · **{name}**");
        } else {
            let _ = write!(s, " · [{name}]({name}.md)");
        }
    }
    s.push_str("\n\n");
    s
}

/// A link to an item if the reference has it, else just its name.
fn link(key: &str, index: &HashMap<String, String>) -> String {
    match index.get(key) {
        Some(target) => format!("[`{key}`]({target})"),
        None => format!("`{key}`"),
    }
}

/// Rustdoc Markdown to plain Markdown: intra-doc links become links into the
/// reference (or plain code when the target is not here), hidden doctest lines are
/// dropped, and Rust code fences are labelled.
fn docs_md(docs: &str, index: &HashMap<String, String>, owner: Option<&str>) -> String {
    let mut out = String::new();
    let mut in_code = false;
    for line in docs.lines() {
        if let Some(rest) = line.trim_start().strip_prefix("```") {
            in_code = !in_code;
            if in_code {
                let lang = rest.split(',').next().unwrap_or("").trim();
                let lang = if lang.is_empty() || lang == "no_run" || lang == "ignore" || lang == "should_panic" {
                    "rust"
                } else {
                    lang
                };
                let _ = writeln!(out, "```{lang}");
            } else {
                out.push_str("```\n");
            }
            continue;
        }
        if in_code {
            if line.starts_with("# ") || line == "#" {
                continue;
            }
            out.push_str(line);
            out.push('\n');
            continue;
        }
        // Headings sit under the page's and the item's, so they move down two levels.
        if line.starts_with('#') {
            out.push_str("##");
        }
        out.push_str(&relink(line, index, owner));
        out.push('\n');
    }
    out.trim_end().to_string()
}

/// Rewrites the `[text](target)` and `` [`Name`] `` links of one line.
fn relink(line: &str, index: &HashMap<String, String>, owner: Option<&str>) -> String {
    let mut out = String::new();
    let mut rest = line;
    while let Some(open) = rest.find('[') {
        let Some(close) = rest[open..].find(']') else { break };
        let close = open + close;
        let text = &rest[open + 1..close];
        let (target, after) = match rest[close + 1..].strip_prefix('(') {
            Some(tail) => match tail.find(')') {
                Some(end) => (Some(&tail[..end]), &tail[end + 1..]),
                None => (None, &rest[close + 1..]),
            },
            None => (None, &rest[close + 1..]),
        };
        out.push_str(&rest[..open]);
        match target {
            Some(t) if t.starts_with("http") || t.starts_with('#') => {
                let _ = write!(out, "[{text}]({t})");
            }
            _ => {
                let raw = target.unwrap_or(text).trim_matches('`');
                let key = resolve(raw, owner);
                match index.get(&key) {
                    Some(dest) => {
                        let _ = write!(out, "[{text}]({dest})");
                    }
                    None => out.push_str(text),
                }
            }
        }
        rest = after;
    }
    out.push_str(rest);
    out
}

/// The reference key a rustdoc path means, in the context of `owner`.
fn resolve(path: &str, owner: Option<&str>) -> String {
    let path = path.trim_start_matches("crate::");
    if let Some(rest) = path.strip_prefix("Self::") {
        return match owner {
            Some(o) => format!("{o}::{rest}"),
            None => rest.to_string(),
        };
    }
    // `layer::Effect` → `Effect`; `export::svg` stays, as the item is a free fn.
    match path.split_once("::") {
        Some((module, rest))
            if PAGES.iter().any(|(m, ..)| *m == module) && rest.chars().next().is_some_and(char::is_uppercase) =>
        {
            rest.to_string()
        }
        _ => path.to_string(),
    }
}
