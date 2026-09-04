//! ratatui integration: the snake inside a bordered block, animated. `t` toggles
//! between RGB and the terminal's palette colours, `q` quits.
//!
//! ```text
//! cargo run --release --features ratatui --example tui
//! ```

#[path = "common/mod.rs"]
mod common;

use std::io;
use std::time::Duration;

use cobra::ratatui::{Braille, overlay};
use cobra::{Canvas, Renderer, Terminal};
use crossterm::event::{self, Event, KeyCode};
use ratatui::layout::Rect;
use ratatui::widgets::{Block, Borders, Paragraph};

fn main() -> io::Result<()> {
    let term = Terminal::detect();
    let mut renderer = Renderer::new(term);
    let mut canvas = Canvas::new(common::COLS, common::ROWS);
    let mut tui = ratatui::init();
    let mut phase = 0.0f32;
    let mut themed = false;
    let result = loop {
        if themed {
            common::draw_themed(&mut canvas, phase);
        } else {
            common::draw(&mut canvas, phase);
        }
        phase += 0.15;
        let mut inner = Rect::default();
        let frame = tui.draw(|f| {
            let block = Block::default().borders(Borders::ALL).title(format!(
                " cobra · {:?} · {} · t toggles · q quits ",
                term.protocol,
                if themed { "palette" } else { "rgb" }
            ));
            let area = Rect::new(2, 1, common::COLS + 2, common::ROWS + 2).intersection(f.area());
            inner = block.inner(area);
            f.render_widget(block, area);
            f.render_widget(Braille::new(&canvas, &renderer), inner);
            let below = Rect::new(area.x, area.bottom(), 40, 1).intersection(f.area());
            f.render_widget(Paragraph::new("Text around the canvas stays aligned."), below);
        });
        if let Err(e) = frame {
            break Err(e);
        }
        if let Err(e) = overlay(&mut renderer, &canvas, inner, tui.backend_mut()) {
            break Err(e);
        }
        if event::poll(Duration::from_millis(33))?
            && let Event::Key(k) = event::read()?
        {
            match k.code {
                KeyCode::Char('q') | KeyCode::Esc => break Ok(()),
                KeyCode::Char('t') => themed = !themed,
                _ => {}
            }
        }
    };
    ratatui::restore();
    result
}
