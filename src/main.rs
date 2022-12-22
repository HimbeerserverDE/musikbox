use crossterm::terminal::{disable_raw_mode, enable_raw_mode};
use std::io;
use std::thread;
use std::time::Duration;
use tui::widgets::{Block, Borders};
use tui::{backend::CrosstermBackend, Terminal};

fn main() -> anyhow::Result<()> {
    enable_raw_mode()?;

    let stdout = io::stdout();
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    terminal.draw(|f| {
        let size = f.size();
        let block = Block::default().borders(Borders::ALL);

        f.render_widget(block, size);
    })?;

    thread::sleep(Duration::from_secs(3));

    disable_raw_mode()?;

    Ok(())
}
