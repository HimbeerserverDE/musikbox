use clap::Parser;
use crossterm::event::{self, Event, KeyCode};
use crossterm::terminal::{disable_raw_mode, enable_raw_mode};
use gstreamer::ClockTime;
use gstreamer_play::{Play, PlayVideoRenderer};
use std::fs;
use std::io;
use std::path::PathBuf;
use std::time::Duration;
use tui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use tui::style::{Color, Style};
use tui::widgets::{Block, Borders, Gauge, List, ListItem, ListState, Paragraph};
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
    Control,
}

impl CursorState {
    fn overflowing_next(&mut self) {
        *self = match self {
            Self::MusicList => Self::Volume,
            Self::Volume => Self::Control,
            Self::Control => Self::MusicList,
        };
    }
}

impl Default for CursorState {
    fn default() -> Self {
        Self::MusicList
    }
}

#[derive(Debug, Default)]
struct AutoplayState {
    repeat_list: bool,
    repeat: bool,
    sequential: bool,
    shuffle: bool,
}

fn subsize(area: Rect, i: u16) -> Rect {
    let mut new_area = area;
    new_area.y += i * area.height;

    new_area
}

fn is_paused(play: &Play) -> bool {
    match play.position() {
        Some(position) => match play.position() {
            Some(new_pos) => position.nseconds() == new_pos.nseconds(),
            None => true,
        },
        None => true,
    }
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    let mut cursor_state = CursorState::default();
    let mut autoplay_state = AutoplayState::default();

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

            let status_title = match play.uri() {
                Some(uri) => {
                    String::from("Now playing: ") + uri.as_str().split('/').next_back().unwrap()
                }
                None => String::from("Idle"),
            };

            let status_block = Block::default()
                .title(status_title)
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
            let control_size = subsize(status_sizes, 2);

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

            let control_buttons = if is_paused(&play) {
                String::from(
                    "[ 🔁 ]   [ 🔂 ]   [ ⏮ ]   [ ◀ ]   [ ▶ ]   [ ▶ ]   [ ⏭ ]   [ ⏬ ]   [ 🔀 ]\n\n",
                )
            } else {
                String::from(
                    "[ 🔁 ]   [ 🔂 ]   [ ⏮ ]   [ ◀ ]   [ ⏸ ]   [ ▶ ]   [ ⏭ ]   [ ⏬ ]   [ 🔀 ]\n\n",
                )
            };

            let mut control_indicators = String::new();

            if autoplay_state.repeat_list {
                control_indicators += " 🔁 ";
            }
            if autoplay_state.repeat {
                control_indicators += " 🔂 ";
            }
            if autoplay_state.sequential {
                control_indicators += " ⏬ ";
            }
            if autoplay_state.shuffle {
                control_indicators += " 🔀 ";
            }

            let block = Block::default().borders(Borders::ALL);
            let control_paragraph = Paragraph::new(control_buttons + &control_indicators)
                .block(block)
                .alignment(Alignment::Center)
                .style(match cursor_state {
                    CursorState::Control => focused_style,
                    _ => main_style,
                });

            f.render_stateful_widget(listing, listing_size, &mut list_state);
            f.render_widget(status_block, status_size);
            f.render_widget(volume_gauge, volume_size);
            f.render_widget(progress_gauge, progress_size);
            f.render_widget(control_paragraph, control_size);
        })?;

        let progress_ratio = if let Some(position) = play.position() {
            if let Some(duration) = play.duration() {
                position.seconds() as f64 / duration.seconds() as f64
            } else {
                0.0
            }
        } else {
            0.0
        };

        if progress_ratio == 1.0 {
            if autoplay_state.repeat {
                play.play();
            } else if autoplay_state.sequential {
                let mut track = files
                    .iter()
                    .enumerate()
                    .find(|(_, file)| format!("file://{}", file.display()) == play.uri().unwrap())
                    .unwrap()
                    .0
                    + 1;

                if track >= files.len() && autoplay_state.repeat_list {
                    track = 0
                }

                if track < files.len() {
                    let file_path = &files[track];
                    let uri = format!("file://{}", file_path.display());

                    play.set_uri(Some(&uri));
                    play.play();
                }
            } else if autoplay_state.shuffle {
                let track = rand::random::<usize>() & files.len();

                let file_path = &files[track];
                let uri = format!("file://{}", file_path.display());

                play.set_uri(Some(&uri));
                play.play();
            }
        }

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
                KeyCode::Char(' ') => {
                    if is_paused(&play) {
                        play.play();
                    } else {
                        play.pause();
                    }
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
                    CursorState::Control => match key.code {
                        KeyCode::Left => {
                            if let Some(position) = play.position() {
                                play.seek(ClockTime::from_seconds(
                                    0_u64.max(position.seconds().saturating_sub(1)),
                                ));
                            }
                        }
                        KeyCode::Right => {
                            if let Some(position) = play.position() {
                                if let Some(duration) = play.duration() {
                                    play.seek(ClockTime::from_seconds(
                                        duration
                                            .seconds()
                                            .min(position.seconds().saturating_add(1)),
                                    ));
                                }
                            }
                        }
                        KeyCode::Down => {
                            if let Some(position) = play.position() {
                                play.seek(ClockTime::from_seconds(
                                    0_u64.max(position.seconds().saturating_sub(15)),
                                ));
                            }
                        }
                        KeyCode::Up => {
                            if let Some(position) = play.position() {
                                if let Some(duration) = play.duration() {
                                    play.seek(ClockTime::from_seconds(
                                        duration
                                            .seconds()
                                            .min(position.seconds().saturating_add(15)),
                                    ));
                                }
                            }
                        }
                        KeyCode::Char('r') => {
                            autoplay_state.repeat = !autoplay_state.repeat;
                        }
                        KeyCode::Char('s') => {
                            autoplay_state.shuffle = !autoplay_state.shuffle;
                        }
                        KeyCode::Char('l') => {
                            autoplay_state.sequential = !autoplay_state.sequential;
                        }
                        KeyCode::Char('i') => {
                            autoplay_state.repeat_list = !autoplay_state.repeat_list;
                        }
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
