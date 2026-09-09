//! Prints what `Terminal::detect` decided, and the environment it decided from.
//!
//! Detection is one function call and normally invisible; when a terminal draws
//! nothing, draws in braille where it could draw dots, or is sent a sequence it does
//! not understand, this is the report to read and to paste into a bug report:
//!
//! ```text
//! cargo run --example detect
//! ```
//!
//! The line that matters most is `passthrough`: it wraps every image in a `DCS` that
//! only tmux may unwrap, so it must be on in a tmux pane and off everywhere else. The
//! pane is settled by asking tmux which tty its pane draws on, printed here beside
//! this process's own; `COBRA_PASSTHROUGH` overrides the verdict either way.

use std::env::var;
use std::process::{Command, Stdio};

use cobra::Terminal;

fn main() {
    let term = Terminal::detect();
    println!("protocol      {:?}", term.protocol);
    println!("cell          {}×{} px", term.cell.width, term.cell.height);
    println!("screen        {}×{} cells", term.cols, term.rows);
    println!("text depth    {:?}", term.depth);
    println!("palette       {}", if term.palette_queried { "asked the terminal" } else { "xterm defaults" });
    println!("passthrough   {} (images wrapped for tmux)", term.passthrough);

    println!("\nenvironment");
    for key in ["TERM", "TERM_PROGRAM", "TMUX", "TMUX_PANE", "COBRA_PROTOCOL", "COBRA_CELL", "COBRA_PASSTHROUGH"] {
        println!("  {key:<18}{}", var(key).unwrap_or_else(|_| "(unset)".into()));
    }

    // The question behind `passthrough`, asked the way detection asks it: a terminal
    // started from inside tmux inherits `TMUX` and `TMUX_PANE`, and differs from a
    // pane only in the tty it draws on.
    if let Some(tmux) = var("TMUX").ok().filter(|v| !v.is_empty()) {
        let socket = tmux.split(',').next().unwrap_or(&tmux).to_owned();
        println!("\ntmux");
        println!("  pane tty          {}", ask(&["-S", &socket, "display-message", "-p", "#{pane_tty}"]));
        println!("  this process      {}", ask(&["-c", "ps -o tty= -p $$ | tr -d ' '"]).trim().to_owned());
    }
}

/// Runs `tmux` (or `sh`, for the last query) and returns its first line.
fn ask(args: &[&str]) -> String {
    let program = if args[0] == "-c" { "sh" } else { "tmux" };
    let out = Command::new(program).args(args).stdin(Stdio::null()).stderr(Stdio::null()).output();
    match out {
        Ok(out) if out.status.success() => match String::from_utf8_lossy(&out.stdout).trim() {
            "" => "(no answer)".into(),
            s => s.to_owned(),
        },
        Ok(_) => "(tmux knows no such pane)".into(),
        Err(_) => format!("({program} could not be run)"),
    }
}
