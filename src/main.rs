use crossterm::event::{self, Event, KeyCode};
use crossterm::terminal::{disable_raw_mode, enable_raw_mode};
use std::fs;
use std::io;
use tui::widgets::{Block, Borders, List, ListItem, ListState};
use tui::{backend::CrosstermBackend, Terminal};

fn main() -> anyhow::Result<()> {
    enable_raw_mode()?;

    let stdout = io::stdout();
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let files: Vec<String> = fs::read_dir("/home/himbeer/music")?
        .map(|e| e.unwrap().file_name().into_string().unwrap())
        .collect();

    let mut list_state = ListState::default();
    list_state.select(Some(0));

    loop {
        terminal.draw(|f| {
            let size = f.size();

            let files: Vec<ListItem> = files.iter().map(|e| ListItem::new(e.clone())).collect();

            let block = Block::default().title("Files").borders(Borders::ALL);
            let listing = List::new(files).block(block).highlight_symbol("> ");

            f.render_stateful_widget(listing, size, &mut list_state);
        })?;

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
