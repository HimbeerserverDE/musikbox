use crossterm::event::{self, Event, KeyCode};
use crossterm::terminal::{disable_raw_mode, enable_raw_mode};
use std::fs;
use std::io;
use tui::widgets::{Block, Borders, List, ListItem};
use tui::{backend::CrosstermBackend, Terminal};

fn main() -> anyhow::Result<()> {
    enable_raw_mode()?;

    let stdout = io::stdout();
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    terminal.draw(|f| {
        let size = f.size();

        let files: Vec<ListItem> = fs::read_dir("/home/himbeer/music")
            .expect("can't read music directory")
            .map(|entry| {
                let file = entry
                    .expect("faulty DirEntry")
                    .file_name()
                    .into_string()
                    .expect("file name contains invalid unicode");

                ListItem::new(file)
            })
            .collect();

        let block = Block::default().title("Files").borders(Borders::ALL);
        let listing = List::new(files).block(block);

        f.render_widget(listing, size);
    })?;

    loop {
        if let Event::Key(key) = event::read()? {
            match key.code {
                KeyCode::Esc => {
                    break;
                }
                _ => {}
            }
        }
    }

    disable_raw_mode()?;

    Ok(())
}
