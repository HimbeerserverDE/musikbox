use crossterm::event::{self, Event, KeyCode};
use crossterm::terminal::{disable_raw_mode, enable_raw_mode};
use gstreamer_play::{Play, PlayVideoRenderer};
use std::fs;
use std::io;
use std::path::PathBuf;
use tui::style::{Color, Style};
use tui::widgets::{Block, Borders, List, ListItem, ListState};
use tui::{backend::CrosstermBackend, Terminal};

fn main() -> anyhow::Result<()> {
    gstreamer::init()?;
    enable_raw_mode()?;

    let play = Play::new(PlayVideoRenderer::NONE);

    let stdout = io::stdout();
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let files: Vec<PathBuf> = fs::read_dir("/home/himbeer/music")?
        .map(|e| e.unwrap().path())
        .collect();

    let mut list_state = ListState::default();
    list_state.select(Some(0));

    loop {
        terminal.draw(|f| {
            let size = f.size();

            let files: Vec<ListItem> = files
                .iter()
                .map(|e| ListItem::new(e.file_name().unwrap().to_str().unwrap()))
                .collect();

            let block = Block::default().title("Files").borders(Borders::ALL);
            let listing = List::new(files)
                .block(block)
                .style(Style::default().bg(Color::Reset).fg(Color::Green))
                .highlight_style(Style::default().bg(Color::Green).fg(Color::Black))
                .highlight_symbol("> ");

            f.render_stateful_widget(listing, size, &mut list_state);
        })?;

        if let Event::Key(key) = event::read()? {
            match key.code {
                KeyCode::Esc => {
                    break;
                }
                KeyCode::Down => match list_state.selected() {
                    Some(i) => list_state.select(Some((i + 1) % files.len())),
                    None => list_state.select(Some(0)),
                },
                KeyCode::Up => match list_state.selected() {
                    Some(i) => list_state.select(Some(if i > 0 {
                        (i - 1) % files.len()
                    } else {
                        files.len() - 1
                    })),
                    None => list_state.select(Some(files.len() - 1)),
                },
                KeyCode::Char('r') => {
                    let track = rand::random::<usize>() % files.len();
                    list_state.select(Some(track));
                }
                KeyCode::Enter => {
                    let track = match list_state.selected() {
                        Some(i) => i,
                        None => {
                            continue;
                        }
                    };

                    let file_path = &files[track];
                    let uri = format!("file://{}", file_path.display());

                    play.set_uri(Some(&uri));
                    play.play();
                }
                _ => {}
            }
        }
    }

    disable_raw_mode()?;
    terminal.clear()?;
    terminal.set_cursor(0, 0)?;

    Ok(())
}
