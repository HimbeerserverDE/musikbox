use clap::Parser;
use crossterm::event::{self, Event, KeyCode};
use crossterm::terminal::{disable_raw_mode, enable_raw_mode};
use gstreamer_play::{Play, PlayVideoRenderer};
use std::fs;
use std::io;
use std::path::PathBuf;
use std::time::Duration;
use tui::layout::{Constraint, Direction, Layout, Rect};
use tui::style::{Color, Style};
use tui::widgets::{Block, Borders, Gauge, List, ListItem, ListState};
use tui::{backend::CrosstermBackend, Terminal};

#[derive(Debug, Parser)]
#[command(author = "Himbeer", version = "v0.1.0", about = "A custom music player for the command line, written in Rust.", long_about = None)]
struct Args {
    /// Playlist directory. Defaults to current directory.
    #[arg(short = 'd', long = "dir")]
    dir: Option<String>,
}

#[derive(Debug)]
enum CursorState {
    MusicList,
    Volume,
}

impl CursorState {
    fn overflowing_next(&mut self) {
        *self = match self {
            Self::MusicList => Self::Volume,
            Self::Volume => Self::MusicList,
        };
    }
}

impl Default for CursorState {
    fn default() -> Self {
        Self::MusicList
    }
}

fn subsize(area: Rect, i: u16) -> Rect {
    let mut new_area = area;
    new_area.y += i * area.height;

    new_area
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let mut cursor_state = CursorState::default();

    gstreamer::init()?;
    enable_raw_mode()?;

    let play = Play::new(PlayVideoRenderer::NONE);

    let stdout = io::stdout();
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut files: Vec<PathBuf> = fs::read_dir(args.dir.unwrap_or_else(|| String::from(".")))?
        .map(|e| e.unwrap().path())
        .collect();

    files.sort();

    let mut list_state = ListState::default();
    list_state.select(Some(0));

    loop {
        terminal.draw(|f| {
            let main_style = Style::default().bg(Color::Reset).fg(Color::Magenta);
            let focused_style = main_style.fg(Color::Cyan);

            let sizes = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Length(f.size().width / 2), Constraint::Min(0)])
                .split(f.size());

            let listing_size = sizes[0];
            let status_size = sizes[1];

            let files: Vec<ListItem> = files
                .iter()
                .map(|e| ListItem::new(e.file_name().unwrap().to_str().unwrap()))
                .collect();

            let highlight_base_style = match cursor_state {
                CursorState::MusicList => focused_style,
                _ => main_style,
            };

            let block = Block::default().title("Select music").borders(Borders::ALL);
            let listing = List::new(files)
                .block(block)
                .style(match cursor_state {
                    CursorState::MusicList => focused_style,
                    _ => main_style,
                })
                .highlight_style(
                    highlight_base_style
                        .bg(highlight_base_style.fg.unwrap())
                        .fg(Color::Black),
                )
                .highlight_symbol("> ");

            let status_block = Block::default()
                .title("Now playing")
                .borders(Borders::ALL)
                .style(main_style);
            let status_sizes = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(status_size.height / 10),
                    Constraint::Min(0),
                ])
                .margin(1)
                .split(status_size)[0];

            let volume_size = subsize(status_sizes, 0);
            let progress_size = subsize(status_sizes, 1);

            let block = Block::default().title("Volume").borders(Borders::ALL);
            let volume_gauge = Gauge::default()
                .block(block)
                .style(match cursor_state {
                    CursorState::Volume => focused_style,
                    _ => main_style,
                })
                .gauge_style(main_style.fg(Color::Blue))
                .ratio(play.volume());

            let block = Block::default().borders(Borders::ALL);
            let progress_gauge = Gauge::default()
                .block(block)
                .style(main_style)
                .gauge_style(main_style.fg(Color::Blue))
                .ratio(if let Some(position) = play.position() {
                    if let Some(duration) = play.duration() {
                        position.seconds() as f64 / duration.seconds() as f64
                    } else {
                        0.0
                    }
                } else {
                    0.0
                });

            f.render_stateful_widget(listing, listing_size, &mut list_state);
            f.render_widget(status_block, status_size);
            f.render_widget(volume_gauge, volume_size);
            f.render_widget(progress_gauge, progress_size);
        })?;

        if !event::poll(Duration::from_secs(1))? {
            continue;
        }

        if let Event::Key(key) = event::read()? {
            match key.code {
                KeyCode::Esc | KeyCode::Char('q') => {
                    break;
                }
                KeyCode::Tab => {
                    cursor_state.overflowing_next();
                }
                _ => match cursor_state {
                    CursorState::MusicList => match key.code {
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
                    },
                    CursorState::Volume => match key.code {
                        KeyCode::Left => play.set_volume(0.0_f64.max(play.volume() - 0.01)),
                        KeyCode::Right => play.set_volume(1.0_f64.min(play.volume() + 0.01)),
                        KeyCode::Home => play.set_volume(0.0),
                        KeyCode::End => play.set_volume(1.0),
                        KeyCode::Down => play.set_volume(0.0_f64.max(play.volume() - 0.05)),
                        KeyCode::Up => play.set_volume(1.0_f64.min(play.volume() + 0.05)),
                        _ => {}
                    },
                },
            }
        }
    }

    disable_raw_mode()?;
    terminal.clear()?;
    terminal.set_cursor(0, 0)?;

    Ok(())
}
