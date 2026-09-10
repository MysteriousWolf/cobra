//! A flight recorder for frames: draws every scene in sequence at the real terminal
//! and writes down, before each frame leaves the process, exactly what is about to be
//! sent.
//!
//! The point is what survives a terminal that dies mid-frame. Every line is flushed
//! and `fsync`ed before the bytes reach the tty, so the last block in the log names
//! the scene, the phase, the protocol, the geometry and the wire structure of the
//! frame that killed it — and `--dump` leaves that frame on disk as a file anyone can
//! `cat` back, which is what an upstream bug report needs.
//!
//! ```text
//! cargo run --release --example soak                       # everything, once, to this terminal
//! cargo run --release --example soak -- --list             # the scene table, no drawing
//! cargo run --release --example soak -- --dump /tmp/soak   # keep every frame as a .bin
//! cargo run --release --example soak -- --only kitty-ish --step   # one at a time, on Enter
//! cargo run --release --example soak -- --protocol kitty --repeat 30 --fps 15
//! cargo run --release --example soak -- --dry --protocol all       # encode only, no tty
//! ```
//!
//! Options:
//!
//! ```text
//!   --log <path>        where the record goes (default cobra-soak.log)
//!   --dump <dir>        also write each frame's exact bytes to <dir>/NNNN-scene.bin
//!   --list              print the scene table and exit
//!   --only <substr>     only scenes whose name contains this (repeatable)
//!   --skip <substr>     drop scenes whose name contains this (repeatable)
//!   --from <n>          start at scene index n
//!   --count <n>         stop after n scenes
//!   --repeat <n>        frames per scene, phase 0..1 across them (default 1)
//!   --loops <n>         run the whole sequence n times (default 1, 0 = forever)
//!   --protocol <p>      text | kitty | iterm2 | sixel | auto | all (default auto)
//!   --placement <p>     flow | at (default flow)
//!   --copy-text         also emit the braille text under image frames
//!   --fps <n>           cap frames per second (default 0, uncapped)
//!   --pause <ms>        wait this long after each frame
//!   --step              wait for Enter after each frame
//!   --dry               encode and log, but never write to the terminal
//!   --echo              copy the record to stderr as well
//!   --hexdump <n>       log the first and last n bytes of every frame
//!   --no-sync           do not fsync the log after each line (faster, less safe)
//! ```
//!
//! `COBRA_PROTOCOL`, `COBRA_CELL`, `COBRA_COLORS` and `COBRA_DOT` are read by
//! detection as usual and are written into the log's header.

#[path = "common/scenes.rs"]
mod scenes;

use std::fmt::Write as _;
use std::fs::{self, File};
use std::io::{self, BufRead, Write};
use std::path::PathBuf;
use std::time::{Duration, Instant};

use cobra::{Canvas, Options, Placement, Protocol, Renderer, Terminal};
use scenes::{SCENES, Scene};

fn main() -> io::Result<()> {
    let args = match Args::parse() {
        Ok(args) => args,
        Err(msg) => {
            eprintln!("soak: {msg}");
            eprintln!("soak: see the header of examples/soak.rs for the options");
            std::process::exit(2);
        }
    };
    if args.list {
        for (i, scene) in SCENES.iter().enumerate() {
            println!("{i:2}  {:<10} {:>3}x{:<3}  {}", scene.name, scene.cols, scene.rows, scene.about);
        }
        return Ok(());
    }

    let selected = args.select();
    if selected.is_empty() {
        eprintln!("soak: no scene matches");
        std::process::exit(2);
    }
    let mut rec = Recorder::open(&args)?;
    let protocols = args.protocols();
    let base = Terminal::detect();
    rec.header(&args, &base, &selected, &protocols);

    let mut totals = Totals::default();
    let mut loop_n = 0u64;
    'outer: while args.loops == 0 || loop_n < args.loops {
        loop_n += 1;
        for protocol in &protocols {
            let term = with_protocol(base, *protocol);
            let opts = Options { copy_text: args.copy_text, ..Options::from_env() };
            let mut renderer = Renderer::with_options(term, opts);
            rec.note(&format!(
                "run loop={loop_n} protocol={:?} image_id={} cell={}x{} screen={}x{} passthrough={}",
                term.protocol,
                renderer.image_id(),
                term.cell.width,
                term.cell.height,
                term.cols,
                term.rows,
                term.passthrough
            ));
            for (index, scene) in &selected {
                for rep in 0..args.repeat.max(1) {
                    let phase = rep as f32 / args.repeat.max(1) as f32;
                    if !frame(&args, &mut rec, &mut renderer, *index, scene, rep, phase, &mut totals)? {
                        break 'outer;
                    }
                }
            }
        }
    }

    rec.note(&format!(
        "done frames={} bytes={} biggest={} ({}) — the sequence ran to the end",
        totals.frames, totals.bytes, totals.biggest, totals.biggest_scene
    ));
    eprintln!("soak: {} frames, {} bytes written; record in {}", totals.frames, totals.bytes, args.log.display());
    Ok(())
}

/// Draws, logs and sends one frame. `Ok(false)` means the caller should stop.
#[allow(clippy::too_many_arguments)]
fn frame(
    args: &Args,
    rec: &mut Recorder,
    renderer: &mut Renderer,
    index: usize,
    scene: &Scene,
    rep: u32,
    phase: f32,
    totals: &mut Totals,
) -> io::Result<bool> {
    let started = Instant::now();
    let mut canvas = Canvas::new(scene.cols, scene.rows);
    (scene.draw)(&mut canvas, phase);
    let drawn = started.elapsed();

    let placement = match args.placement {
        Place::Flow => Placement::Flow,
        Place::At => Placement::At(0, 0),
    };
    let encode_start = Instant::now();
    let bytes = renderer.encode(&canvas, placement).to_vec();
    let encoded = encode_start.elapsed();

    let term = renderer.terminal();
    let wire = Wire::of(&bytes);
    totals.frames += 1;
    totals.bytes += bytes.len() as u64;
    if bytes.len() as u64 > totals.biggest {
        totals.biggest = bytes.len() as u64;
        totals.biggest_scene = format!("{}#{rep}", scene.name);
    }

    rec.note(&format!(
        "frame scene={} index={index} rep={rep} phase={phase:.3} protocol={:?} placement={:?} \
         canvas={}x{} cells dots={}x{} text_layer={} bytes={} crc32={:#010x} draw={:.3}ms encode={:.3}ms",
        scene.name,
        term.protocol,
        scene.cols,
        scene.rows,
        canvas.width(),
        canvas.height(),
        canvas.has_text(),
        bytes.len(),
        crc32(&bytes),
        drawn.as_secs_f64() * 1e3,
        encoded.as_secs_f64() * 1e3,
    ));
    for line in wire.report(args.hexdump, &bytes) {
        rec.note(&format!("  {line}"));
    }

    if let Some(dir) = &args.dump {
        let path = dir.join(format!("{:04}-{}-{}.bin", totals.frames, scene.name, rep));
        let mut f = File::create(&path)?;
        f.write_all(&bytes)?;
        f.sync_all()?;
        rec.note(&format!("  dump {} — reproduce with: command cat {}", path.display(), path.display()));
    }

    if args.dry {
        rec.note("  dry: not written to the terminal");
    } else {
        rec.note("  write: about to hand the frame to the tty");
        let mut out = io::stdout().lock();
        if args.placement == Place::At {
            out.write_all(b"\x1b[2J\x1b[H")?;
        }
        let write_start = Instant::now();
        match out.write_all(&bytes).and_then(|()| out.flush()) {
            Ok(()) => rec.note(&format!("  write: flushed in {:.3}ms", write_start.elapsed().as_secs_f64() * 1e3)),
            Err(e) => {
                rec.note(&format!("  write: FAILED after {:.3}ms: {e}", write_start.elapsed().as_secs_f64() * 1e3));
                return Err(e);
            }
        }
    }

    if args.step {
        eprint!("soak: {} rep {rep} sent; Enter for the next frame, q to stop: ", scene.name);
        io::stderr().flush()?;
        let mut line = String::new();
        io::stdin().lock().read_line(&mut line)?;
        if line.trim().eq_ignore_ascii_case("q") {
            rec.note("stop: asked to stop at the prompt");
            return Ok(false);
        }
    }
    if args.pause > 0 {
        std::thread::sleep(Duration::from_millis(args.pause));
    }
    if args.fps > 0.0 {
        let budget = Duration::from_secs_f64(1.0 / args.fps as f64);
        if let Some(left) = budget.checked_sub(started.elapsed()) {
            std::thread::sleep(left);
        }
    }
    Ok(true)
}

fn with_protocol(base: Terminal, protocol: Option<Protocol>) -> Terminal {
    match protocol {
        None => base,
        Some(p) => Terminal { protocol: p, ..base },
    }
}

// ---------------------------------------------------------------- the record

/// The log file, and the promise that what is in it reached the disk before the frame
/// it describes reached the terminal.
struct Recorder {
    file: File,
    start: Instant,
    seq: u64,
    echo: bool,
    sync: bool,
}

impl Recorder {
    fn open(args: &Args) -> io::Result<Self> {
        let file = File::create(&args.log)?;
        Ok(Self { file, start: Instant::now(), seq: 0, echo: args.echo, sync: !args.no_sync })
    }

    fn note(&mut self, text: &str) {
        self.seq += 1;
        let line = format!("[{:06} {:9.3}s] {text}\n", self.seq, self.start.elapsed().as_secs_f64());
        // A failure to record is worth knowing about, but never worth killing the run
        // for: the run is the experiment.
        let _ = self.file.write_all(line.as_bytes());
        let _ = self.file.flush();
        if self.sync {
            let _ = self.file.sync_data();
        }
        if self.echo {
            let _ = io::stderr().write_all(line.as_bytes());
        }
    }

    fn header(
        &mut self,
        args: &Args,
        term: &Terminal,
        selected: &[(usize, &'static Scene)],
        protocols: &[Option<Protocol>],
    ) {
        self.note(&format!("soak cobra {} — {}", env!("CARGO_PKG_VERSION"), now()));
        self.note(&format!(
            "detected protocol={:?} cell={}x{}px screen={}x{} cells depth={:?} passthrough={} palette_queried={}",
            term.protocol,
            term.cell.width,
            term.cell.height,
            term.cols,
            term.rows,
            term.depth,
            term.passthrough,
            term.palette_queried
        ));
        const WATCHED: [&str; 7] =
            ["TERM", "TERM_PROGRAM", "COLORTERM", "COBRA_PROTOCOL", "COBRA_CELL", "COBRA_COLORS", "COBRA_DOT"];
        let env: Vec<String> =
            WATCHED.iter().map(|k| format!("{k}={}", std::env::var(k).unwrap_or_else(|_| "-".into()))).collect();
        let tmux = if std::env::var_os("TMUX").is_some() { "set" } else { "-" };
        self.note(&format!("env {} TMUX={tmux}", env.join(" ")));
        self.note(&format!(
            "plan protocols={:?} placement={:?} repeat={} loops={} fps={} dry={} dump={}",
            protocols.iter().map(|p| p.map_or("auto".into(), |p| format!("{p:?}"))).collect::<Vec<_>>(),
            args.placement,
            args.repeat,
            args.loops,
            args.fps,
            args.dry,
            args.dump.as_ref().map_or("-".into(), |d| d.display().to_string())
        ));
        for (i, scene) in selected {
            self.note(&format!("scene {i:2} {:<10} {:>3}x{:<3} {}", scene.name, scene.cols, scene.rows, scene.about));
        }
    }
}

/// Seconds since the epoch: enough to line the log up against a terminal's own log
/// without pulling in a date library.
fn now() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0.0, |d| d.as_secs_f64());
    format!("unix {secs:.3}")
}

#[derive(Default)]
struct Totals {
    frames: u64,
    bytes: u64,
    biggest: u64,
    biggest_scene: String,
}

// ---------------------------------------------------------------- the wire

/// One escape sequence as it appears in a frame.
struct Seq {
    kind: &'static str,
    /// Control data: a kitty APC's keys, an OSC's header, a CSI's parameters.
    control: String,
    /// Payload bytes after the control data.
    body: usize,
    /// Whole sequence length on the wire.
    total: usize,
    /// Whether it arrived inside a tmux passthrough wrapper.
    wrapped: bool,
}

/// A frame taken apart: every escape sequence, in order, plus what the payload claims.
struct Wire {
    seqs: Vec<Seq>,
    /// Bytes that are not part of any escape sequence (printed characters, newlines).
    plain: usize,
}

impl Wire {
    fn of(bytes: &[u8]) -> Self {
        let mut wire = Wire { seqs: Vec::new(), plain: 0 };
        wire.scan(bytes, false);
        wire
    }

    fn scan(&mut self, bytes: &[u8], wrapped: bool) {
        let mut i = 0;
        while i < bytes.len() {
            if bytes[i] != 0x1b {
                self.plain += 1;
                i += 1;
                continue;
            }
            let rest = &bytes[i..];
            let (kind, len) = classify(rest);
            match kind {
                "tmux" => {
                    // The wrapper doubles every escape inside it; undo that and look at
                    // what tmux will actually hand on.
                    let inner = unwrap_tmux(&rest[..len]);
                    let control = String::new();
                    self.seqs.push(Seq { kind: "tmux-passthrough", control, body: inner.len(), total: len, wrapped });
                    self.scan(&inner, true);
                }
                _ => {
                    let (control, body) = split_control(kind, &rest[..len]);
                    self.seqs.push(Seq { kind, control, body, total: len, wrapped });
                }
            }
            i += len;
        }
    }

    /// The lines that go under a frame's entry in the log.
    fn report(&self, hexdump: usize, bytes: &[u8]) -> Vec<String> {
        let mut lines = Vec::new();
        let mut counts: Vec<(&str, usize, usize)> = Vec::new();
        for seq in &self.seqs {
            match counts.iter().position(|(k, _, _)| *k == seq.kind) {
                Some(i) => {
                    counts[i].1 += 1;
                    counts[i].2 += seq.total;
                }
                None => counts.push((seq.kind, 1, seq.total)),
            }
        }
        let summary = counts.iter().map(|(k, n, b)| format!("{k}x{n}={b}B")).collect::<Vec<_>>().join(" ");
        lines.push(format!("wire plain={}B {summary}", self.plain));

        let image_kinds = ["kitty-apc", "iterm2-osc", "sixel-dcs"];
        let images: Vec<&Seq> = self.seqs.iter().filter(|s| image_kinds.contains(&s.kind)).collect();
        if let Some(first) = images.first() {
            let payload: usize = images.iter().map(|s| s.body).sum();
            let biggest = images.iter().map(|s| s.body).max().unwrap_or(0);
            lines.push(format!(
                "image kind={} packets={} payload={}B largest_packet={}B wrapped={}",
                first.kind,
                images.len(),
                payload,
                biggest,
                first.wrapped
            ));
            lines.push(format!("control {}", first.control));
            if first.kind == "kitty-apc" {
                lines.extend(kitty_notes(&first.control, payload));
            }
            let sizes = images.iter().map(|s| s.body.to_string()).collect::<Vec<_>>();
            let shown = match sizes.len() > 12 {
                true => format!("{} … ({} packets)", sizes[..12].join(","), sizes.len()),
                false => sizes.join(","),
            };
            lines.push(format!("packets {shown}"));
        }

        // Cursor movement matters as much as the image: a frame that lands in the
        // wrong place is a frame drawn partly off screen.
        let csi: Vec<String> = self
            .seqs
            .iter()
            .filter(|s| ["csi", "decsc", "decrc"].contains(&s.kind))
            .map(|s| if s.control.is_empty() { s.kind.to_string() } else { format!("{}{}", s.kind, s.control) })
            .collect();
        if !csi.is_empty() {
            lines.push(format!("cursor {}", csi.join(" ")));
        }
        if hexdump > 0 {
            let n = hexdump.min(bytes.len());
            lines.push(format!("head {}", hex(&bytes[..n])));
            lines.push(format!("tail {}", hex(&bytes[bytes.len() - n..])));
        }
        lines
    }
}

/// What a kitty control block claims, against what it actually carries: the ratio and
/// the declared raster are the first two numbers to look at when a terminal chokes.
fn kitty_notes(control: &str, payload_b64: usize) -> Vec<String> {
    let keys: Vec<(&str, &str)> = control.split(',').filter_map(|p| p.split_once('=')).collect();
    let get = |k: &str| keys.iter().find(|(key, _)| *key == k).map(|(_, v)| *v);
    let num = |k: &str| get(k).and_then(|v| v.parse::<u64>().ok()).unwrap_or(0);
    let (w, h) = (num("s"), num("v"));
    let raw = w * h * 4;
    let compressed = payload_b64 / 4 * 3;
    let ratio = if compressed > 0 { raw as f64 / compressed as f64 } else { 0.0 };
    vec![
        format!(
            "kitty id={} action={} format={} compression={}",
            get("i").unwrap_or("-"),
            get("a").unwrap_or("-"),
            get("f").unwrap_or("-"),
            get("o").unwrap_or("none"),
        ),
        format!("kitty raster={w}x{h}px raw={raw}B compressed~{compressed}B ratio={ratio:.1}x"),
        format!(
            "kitty cells={}x{} z={} quiet={} keep_cursor={} virtual={}",
            get("c").unwrap_or("-"),
            get("r").unwrap_or("-"),
            get("z").unwrap_or("0"),
            get("q").unwrap_or("-"),
            get("C").unwrap_or("-"),
            get("U").unwrap_or("-"),
        ),
    ]
}

/// The kind of escape sequence at the start of `rest`, and how many bytes it takes.
fn classify(rest: &[u8]) -> (&'static str, usize) {
    if rest.starts_with(b"\x1bPtmux;") {
        return ("tmux", tmux_len(rest));
    }
    if rest.starts_with(b"\x1b_G") {
        return ("kitty-apc", until(rest, 3, b"\x1b\\"));
    }
    if rest.starts_with(b"\x1b]1337;") {
        return ("iterm2-osc", until_osc(rest));
    }
    if rest.starts_with(b"\x1b]") {
        return ("osc", until_osc(rest));
    }
    if rest.starts_with(b"\x1bP") {
        return ("sixel-dcs", until(rest, 2, b"\x1b\\"));
    }
    if rest.starts_with(b"\x1b7") {
        return ("decsc", 2);
    }
    if rest.starts_with(b"\x1b8") {
        return ("decrc", 2);
    }
    if rest.starts_with(b"\x1b[") {
        let end = rest.iter().skip(2).position(|&b| (0x40..=0x7e).contains(&b)).map_or(rest.len(), |i| i + 3);
        return ("csi", end);
    }
    ("esc", 2.min(rest.len()))
}

/// Length of a sequence that ends at `needle`, searched from `from`.
fn until(rest: &[u8], from: usize, needle: &[u8]) -> usize {
    match rest[from..].windows(needle.len()).position(|w| w == needle) {
        Some(i) => from + i + needle.len(),
        None => rest.len(),
    }
}

/// An OSC ends at BEL or at ST.
fn until_osc(rest: &[u8]) -> usize {
    for i in 2..rest.len() {
        if rest[i] == 0x07 {
            return i + 1;
        }
        if rest[i] == b'\\' && rest[i - 1] == 0x1b {
            return i + 1;
        }
    }
    rest.len()
}

/// A tmux wrapper ends at the first *single* ESC `\`: escapes belonging to the payload
/// are doubled, so a doubled pair is skipped whole.
fn tmux_len(rest: &[u8]) -> usize {
    let mut i = 7; // past "\x1bPtmux;"
    while i < rest.len() {
        if rest[i] != 0x1b {
            i += 1;
        } else if rest.get(i + 1) == Some(&0x1b) {
            i += 2;
        } else if rest.get(i + 1) == Some(&b'\\') {
            return i + 2;
        } else {
            i += 1;
        }
    }
    rest.len()
}

/// The bytes tmux will hand to its own terminal: the wrapper removed, escapes undoubled.
fn unwrap_tmux(seq: &[u8]) -> Vec<u8> {
    let body = &seq[7..seq.len().saturating_sub(2)];
    let mut out = Vec::with_capacity(body.len());
    let mut i = 0;
    while i < body.len() {
        out.push(body[i]);
        i += if body[i] == 0x1b && body.get(i + 1) == Some(&0x1b) { 2 } else { 1 };
    }
    out
}

/// Splits a sequence into its control data and the length of its payload.
fn split_control(kind: &'static str, seq: &[u8]) -> (String, usize) {
    let (head, terminator) = match kind {
        "kitty-apc" => (3, 2),
        "iterm2-osc" | "osc" => (2, 1),
        "sixel-dcs" => (2, 2),
        _ => (2, 0),
    };
    let body = &seq[head.min(seq.len())..seq.len().saturating_sub(terminator)];
    // An iTerm2 frame's control data is the whole `1337;File=…` header, which ends at
    // the colon before the base64; everything else splits at its first `;`.
    let split = match kind {
        "iterm2-osc" => body.iter().position(|&b| b == b':'),
        _ => body.iter().position(|&b| b == b';' || b == b':'),
    };
    match split {
        Some(i) => (String::from_utf8_lossy(&body[..i]).into_owned(), body.len() - i - 1),
        None => (String::from_utf8_lossy(body).into_owned(), 0),
    }
}

fn hex(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 3);
    for &b in bytes {
        let _ = write!(s, "{b:02x}");
    }
    s
}

/// CRC-32 (the PNG/zip polynomial), so two runs of the same frame can be compared.
fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = 0xffff_ffffu32;
    for &b in bytes {
        crc ^= u32::from(b);
        for _ in 0..8 {
            crc = if crc & 1 != 0 { (crc >> 1) ^ 0xedb8_8320 } else { crc >> 1 };
        }
    }
    !crc
}

// ---------------------------------------------------------------- arguments

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Place {
    Flow,
    At,
}

struct Args {
    log: PathBuf,
    dump: Option<PathBuf>,
    list: bool,
    only: Vec<String>,
    skip: Vec<String>,
    from: usize,
    count: usize,
    repeat: u32,
    loops: u64,
    protocol: Option<String>,
    placement: Place,
    copy_text: bool,
    fps: f32,
    pause: u64,
    step: bool,
    dry: bool,
    echo: bool,
    hexdump: usize,
    no_sync: bool,
}

impl Args {
    fn parse() -> Result<Self, String> {
        let mut a = Args {
            log: PathBuf::from("cobra-soak.log"),
            dump: None,
            list: false,
            only: Vec::new(),
            skip: Vec::new(),
            from: 0,
            count: usize::MAX,
            repeat: 1,
            loops: 1,
            protocol: None,
            placement: Place::Flow,
            copy_text: false,
            fps: 0.0,
            pause: 0,
            step: false,
            dry: false,
            echo: false,
            hexdump: 0,
            no_sync: false,
        };
        let argv: Vec<String> = std::env::args().skip(1).collect();
        let mut i = 0;
        while i < argv.len() {
            let arg = argv[i].as_str();
            // A value-taking option consumes the next word; `value()` advances past it.
            macro_rules! value {
                () => {{
                    i += 1;
                    argv.get(i).cloned().ok_or_else(|| format!("{arg} needs a value"))?
                }};
            }
            match arg {
                "--list" => a.list = true,
                "--step" => a.step = true,
                "--dry" => a.dry = true,
                "--echo" => a.echo = true,
                "--copy-text" => a.copy_text = true,
                "--no-sync" => a.no_sync = true,
                "--help" | "-h" => {
                    println!("see the header of examples/soak.rs for the options");
                    std::process::exit(0);
                }
                "--log" => a.log = PathBuf::from(value!()),
                "--dump" => a.dump = Some(PathBuf::from(value!())),
                "--only" => a.only.push(value!()),
                "--skip" => a.skip.push(value!()),
                "--protocol" => a.protocol = Some(value!()),
                "--placement" => {
                    a.placement = match value!().as_str() {
                        "flow" => Place::Flow,
                        "at" => Place::At,
                        other => return Err(format!("unknown placement {other}")),
                    }
                }
                "--from" => a.from = value!().parse().map_err(|_| "--from wants a number".to_string())?,
                "--count" => a.count = value!().parse().map_err(|_| "--count wants a number".to_string())?,
                "--repeat" => a.repeat = value!().parse().map_err(|_| "--repeat wants a number".to_string())?,
                "--loops" => a.loops = value!().parse().map_err(|_| "--loops wants a number".to_string())?,
                "--hexdump" => a.hexdump = value!().parse().map_err(|_| "--hexdump wants a number".to_string())?,
                "--pause" => a.pause = value!().parse().map_err(|_| "--pause wants milliseconds".to_string())?,
                "--fps" => a.fps = value!().parse().map_err(|_| "--fps wants a number".to_string())?,
                other => return Err(format!("unknown option {other}")),
            }
            i += 1;
        }
        if let Some(dir) = &a.dump {
            fs::create_dir_all(dir).map_err(|e| format!("cannot make {}: {e}", dir.display()))?;
        }
        Ok(a)
    }

    fn select(&self) -> Vec<(usize, &'static Scene)> {
        SCENES
            .iter()
            .enumerate()
            .filter(|(i, s)| {
                *i >= self.from
                    && (self.only.is_empty() || self.only.iter().any(|p| s.name.contains(p.as_str())))
                    && !self.skip.iter().any(|p| s.name.contains(p.as_str()))
            })
            .take(self.count)
            .collect()
    }

    /// `None` means "whatever detection found"; `all` walks every protocol.
    fn protocols(&self) -> Vec<Option<Protocol>> {
        match self.protocol.as_deref() {
            None | Some("auto") => vec![None],
            Some("all") => {
                vec![Some(Protocol::Text), Some(Protocol::Kitty), Some(Protocol::Iterm2), Some(Protocol::Sixel)]
            }
            Some(name) => match Protocol::parse(name) {
                Some(p) => vec![Some(p)],
                None => {
                    eprintln!("soak: unknown protocol {name}");
                    std::process::exit(2);
                }
            },
        }
    }
}
