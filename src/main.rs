use clap::Parser;
use crossterm::event::{self, Event, KeyCode};
use crossterm::terminal::{disable_raw_mode, enable_raw_mode};
use gstreamer::ClockTime;
use gstreamer_play::{Play, PlayVideoRenderer};
use std::fmt;
use std::fs;
use std::io;
use std::path::PathBuf;
use std::sync::Once;
use std::thread;
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
    /// Play file on startup. Overrides --random.
    #[arg(short = 'p', long = "play")]
    play: Option<String>,
    /// Exit when there are no songs left to play. Useful in scripts.
    #[arg(short = 'e', long = "no-remain")]
    no_remain: bool,
    /// Play random file on startup. Overridden by --play.
    #[arg(short = 'r', long = "random")]
    random: bool,
    /// Initial volume (float from 0.0 to 1.0).
    /// By default the last selected volume is restored when a file is played.
    #[arg(short = 'v', long = "volume")]
    volume: Option<f64>,
    /// Repeat the entire sequential list. Can be toggled from the TUI.
    #[arg(short = 'i', long = "repeat-list")]
    repeat_list: bool,
    /// Repeat the current song indefinitely. Can be toggled from the TUI.
    #[arg(short = 'R', long = "repeat")]
    repeat: bool,
    /// Play the list (directory) sequentially. Can be toggled from the TUI.
    #[arg(short = 'l', long = "sequential")]
    sequential: bool,
    /// Play the list (directory) randomly and indefinitely. Can be toggled from the TUI.
    #[arg(short = 's', long = "shuffle")]
    shuffle: bool,
    /// Don't create a directory listing.
    #[arg(short = 'n', long = "no-listing")]
    no_listing: bool,
}

#[derive(Debug)]
enum CursorState {
    MusicList,
    Volume,
    Control,
    Search,
}

impl CursorState {
    fn overflowing_next(&mut self) {
        *self = match self {
            Self::MusicList => Self::Volume,
            Self::Volume => Self::Control,
            Self::Control => Self::Search,
            Self::Search => Self::MusicList,
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

struct Instance {
    args: Args,
    cursor_state: CursorState,
    autoplay_state: AutoplayState,
    play: Play,
    files: Vec<PathBuf>,
    list_state: ListState,
    search: String,
    volume_once: Once,
}

impl Instance {
    fn dir(&self) -> String {
        self.args.dir.clone().unwrap_or_else(|| String::from("."))
    }

    fn is_paused(&self) -> bool {
        match self.play.position() {
            Some(position) => match self.play.position() {
                Some(new_pos) => position.nseconds() == new_pos.nseconds(),
                None => true,
            },
            None => true,
        }
    }

    fn play_path<T: fmt::Display>(&self, path: T) {
        let uri = format!("file://{}", path);

        self.play.set_uri(Some(&uri));
        self.play.play();

        if let Some(init_volume) = self.args.volume {
            thread::sleep(Duration::from_millis(500));

            self.volume_once.call_once(|| {
                self.play.set_volume(init_volume);
            });
        }
    }

    /// Get the progress ratio of the current song.
    /// Returns 0.0 if no song is selected.
    fn current_progress(&self) -> f64 {
        if let Some(position) = self.play.position() {
            if let Some(duration) = self.play.duration() {
                position.seconds() as f64 / duration.seconds() as f64
            } else {
                0.0
            }
        } else {
            0.0
        }
    }

    fn new() -> anyhow::Result<Self> {
        let mut instance = Self {
            args: Args::parse(),
            cursor_state: CursorState::default(),
            autoplay_state: AutoplayState::default(),
            play: Play::new(PlayVideoRenderer::NONE),
            files: Vec::new(),
            list_state: ListState::default(),
            search: String::new(),
            volume_once: Once::new(),
        };

        if !instance.args.no_listing {
            instance.files = fs::read_dir(instance.dir())?
                .map(|e| e.unwrap().path())
                .collect();
            instance.files.sort();
        }

        instance.list_state.select(Some(0));

        instance.autoplay_state.repeat_list = instance.args.repeat_list;
        instance.autoplay_state.repeat = instance.args.repeat;
        instance.autoplay_state.sequential = instance.args.sequential;
        instance.autoplay_state.shuffle = instance.args.shuffle;

        Ok(instance)
    }

    fn run(&mut self) -> anyhow::Result<()> {
        enable_raw_mode()?;

        self.play = Play::new(PlayVideoRenderer::NONE);
        let mut terminal = Terminal::new(CrosstermBackend::new(io::stdout()))?;

        if let Some(initial) = &self.args.play {
            self.play_path(initial);
        } else if self.args.random {
            let track = rand::random::<usize>() % self.files.len();
            self.play_path(self.files[track].display());
        }

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

                let files: Vec<ListItem> = self.files
                    .iter()
                    .map(|e| ListItem::new(e.file_name().unwrap().to_str().unwrap()))
                    .collect();

                let highlight_base_style = match self.cursor_state {
                    CursorState::MusicList => focused_style,
                    _ => main_style,
                };

                let block = Block::default().title("Select music").borders(Borders::ALL);
                let listing = List::new(files)
                    .block(block)
                    .style(match self.cursor_state {
                        CursorState::MusicList => focused_style,
                        _ => main_style,
                    })
                    .highlight_style(
                        highlight_base_style
                            .bg(highlight_base_style.fg.unwrap())
                            .fg(Color::Black),
                    )
                    .highlight_symbol("> ");

                let status_title = match self.play.uri() {
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
                        Constraint::Length(status_size.height / 8),
                        Constraint::Min(0),
                    ])
                    .margin(1)
                    .split(status_size)[0];

                let volume_size = subsize(status_sizes, 0);
                let progress_size = subsize(status_sizes, 1);
                let control_size = subsize(status_sizes, 2);
                let search_size = subsize(status_sizes, 3);

                let block = Block::default().title("Volume").borders(Borders::ALL);
                let volume_gauge = Gauge::default()
                    .block(block)
                    .style(match self.cursor_state {
                        CursorState::Volume => focused_style,
                        _ => main_style,
                    })
                    .gauge_style(main_style.fg(Color::Blue))
                    .ratio(self.play.volume());

                let progress_label = match self.play.position() {
                    Some(position) => match self.play.duration() {
                        Some(duration) => {
                            let pos_m = position.minutes();
                            let pos_s = position.seconds() - pos_m * 60;
                            let total_m = duration.minutes();
                            let total_s = duration.seconds() - total_m * 60;

                            format!("{}:{:0>2} / {}:{:0>2}", pos_m, pos_s, total_m, total_s)
                        }
                        None => String::from("-:-- / -:--"),
                    }
                    None => String::from("-:-- / -:--"),
                };

                let block = Block::default().borders(Borders::ALL);
                let progress_gauge = Gauge::default()
                    .block(block)
                    .style(main_style)
                    .label(progress_label)
                    .gauge_style(main_style.fg(Color::Blue))
                    .ratio(self.current_progress());

                let control_buttons = if self.is_paused() {
                    String::from(
                        "[ 🔁 ]   [ 🔂 ]   [ ⏮ ]   [ ◀ ]   [ ▶ ]   [ ▶ ]   [ ⏭ ]   [ ⏬ ]   [ 🔀 ]\n\n",
                    )
                } else {
                    String::from(
                        "[ 🔁 ]   [ 🔂 ]   [ ⏮ ]   [ ◀ ]   [ ⏸ ]   [ ▶ ]   [ ⏭ ]   [ ⏬ ]   [ 🔀 ]\n\n",
                    )
                };

                let mut control_indicators = String::new();

                if self.autoplay_state.repeat_list {
                    control_indicators += " 🔁 ";
                }
                if self.autoplay_state.repeat {
                    control_indicators += " 🔂 ";
                }
                if self.autoplay_state.sequential {
                    control_indicators += " ⏬ ";
                }
                if self.autoplay_state.shuffle {
                    control_indicators += " 🔀 ";
                }

                let block = Block::default().borders(Borders::ALL);
                let control_paragraph = Paragraph::new(control_buttons + &control_indicators)
                    .block(block)
                    .alignment(Alignment::Center)
                    .style(match self.cursor_state {
                        CursorState::Control => focused_style,
                        _ => main_style,
                    });

                let block = Block::default().borders(Borders::ALL);
                let search_paragraph = Paragraph::new(format!("Search: {}", self.search))
                    .block(block)
                    .alignment(Alignment::Left)
                    .style(match self.cursor_state {
                        CursorState::Search => focused_style,
                        _ => main_style,
                    });

                f.render_stateful_widget(listing, listing_size, &mut self.list_state);
                f.render_widget(status_block, status_size);
                f.render_widget(volume_gauge, volume_size);
                f.render_widget(progress_gauge, progress_size);
                f.render_widget(control_paragraph, control_size);
                f.render_widget(search_paragraph, search_size);
            })?;

            if self.current_progress() == 1.0 {
                if self.autoplay_state.repeat {
                    self.play.play();
                } else if self.autoplay_state.sequential {
                    let mut track = self
                        .files
                        .iter()
                        .enumerate()
                        .find(|(_, file)| {
                            format!("file://{}", file.display()) == self.play.uri().unwrap()
                        })
                        .unwrap()
                        .0
                        + 1;

                    if track >= self.files.len() && self.autoplay_state.repeat_list {
                        track = 0
                    }

                    if track < self.files.len() {
                        self.play_path(self.files[track].display());
                    }
                } else if self.autoplay_state.shuffle {
                    let track = rand::random::<usize>() % self.files.len();
                    self.play_path(self.files[track].display());
                } else if self.args.no_remain {
                    break;
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
                        self.cursor_state.overflowing_next();
                    }
                    KeyCode::Char(' ') => {
                        if self.is_paused() {
                            self.play.play();
                        } else {
                            self.play.pause();
                        }
                    }
                    _ => match self.cursor_state {
                        CursorState::MusicList => match key.code {
                            KeyCode::Down => match self.list_state.selected() {
                                Some(i) => {
                                    if self.files.len() > 1 {
                                        self.list_state.select(Some((i + 1) % self.files.len()));
                                    }
                                }
                                None => self.list_state.select(Some(0)),
                            },
                            KeyCode::Up => match self.list_state.selected() {
                                Some(i) => {
                                    if self.files.len() > 1 {
                                        self.list_state.select(Some(if i > 0 {
                                            (i - 1) % self.files.len()
                                        } else {
                                            self.files.len() - 1
                                        }))
                                    }
                                }
                                None => self.list_state.select(Some(self.files.len() - 1)),
                            },
                            KeyCode::Left => match self.list_state.selected() {
                                Some(i) => {
                                    if self.files.len() > 5 {
                                        self.list_state.select(Some(if i > 4 {
                                            (i - 5) % self.files.len()
                                        } else {
                                            self.files.len() - 1
                                        }))
                                    }
                                }
                                None => self.list_state.select(Some(self.files.len() - 1)),
                            },
                            KeyCode::Right => match self.list_state.selected() {
                                Some(i) => {
                                    if self.files.len() > 5 {
                                        self.list_state.select(Some((i + 5) % self.files.len()));
                                    }
                                }
                                None => self.list_state.select(Some(0)),
                            },
                            KeyCode::Home => self.list_state.select(Some(0)),
                            KeyCode::End => self.list_state.select(Some(self.files.len() - 1)),
                            KeyCode::Char('r') => {
                                let track = rand::random::<usize>() % self.files.len();
                                self.list_state.select(Some(track));
                            }
                            KeyCode::Char('R') => {
                                let track = rand::random::<usize>() % self.files.len();
                                self.list_state.select(Some(track));

                                self.play_path(self.files[track].display());
                            }
                            KeyCode::Enter => {
                                let track = match self.list_state.selected() {
                                    Some(i) => i,
                                    None => {
                                        continue;
                                    }
                                };

                                self.play_path(self.files[track].display());
                            }
                            _ => {}
                        },
                        CursorState::Volume => match key.code {
                            KeyCode::Left => {
                                self.play.set_volume(0.0_f64.max(self.play.volume() - 0.01))
                            }
                            KeyCode::Right => {
                                self.play.set_volume(1.0_f64.min(self.play.volume() + 0.01))
                            }
                            KeyCode::Home => self.play.set_volume(0.0),
                            KeyCode::End => self.play.set_volume(1.0),
                            KeyCode::Down => {
                                self.play.set_volume(0.0_f64.max(self.play.volume() - 0.05))
                            }
                            KeyCode::Up => {
                                self.play.set_volume(1.0_f64.min(self.play.volume() + 0.05))
                            }
                            _ => {}
                        },
                        CursorState::Control => match key.code {
                            KeyCode::Left => {
                                if let Some(position) = self.play.position() {
                                    self.play.seek(ClockTime::from_seconds(
                                        0_u64.max(position.seconds().saturating_sub(1)),
                                    ));
                                }
                            }
                            KeyCode::Right => {
                                if let Some(position) = self.play.position() {
                                    if let Some(duration) = self.play.duration() {
                                        self.play.seek(ClockTime::from_seconds(
                                            duration
                                                .seconds()
                                                .min(position.seconds().saturating_add(1)),
                                        ));
                                    }
                                }
                            }
                            KeyCode::Down => {
                                if let Some(position) = self.play.position() {
                                    self.play.seek(ClockTime::from_seconds(
                                        0_u64.max(position.seconds().saturating_sub(15)),
                                    ));
                                }
                            }
                            KeyCode::Up => {
                                if let Some(position) = self.play.position() {
                                    if let Some(duration) = self.play.duration() {
                                        self.play.seek(ClockTime::from_seconds(
                                            duration
                                                .seconds()
                                                .min(position.seconds().saturating_add(15)),
                                        ));
                                    }
                                }
                            }
                            KeyCode::Home => {
                                self.play.seek(ClockTime::ZERO);
                            }
                            KeyCode::End => {
                                if let Some(duration) = self.play.duration() {
                                    self.play.seek(duration);
                                }
                            }
                            KeyCode::Char('r') => {
                                self.autoplay_state.repeat = !self.autoplay_state.repeat;
                            }
                            KeyCode::Char('s') => {
                                self.autoplay_state.shuffle = !self.autoplay_state.shuffle;
                            }
                            KeyCode::Char('l') => {
                                self.autoplay_state.sequential = !self.autoplay_state.sequential;
                            }
                            KeyCode::Char('i') => {
                                self.autoplay_state.repeat_list = !self.autoplay_state.repeat_list;
                            }
                            _ => {}
                        },
                        CursorState::Search => match key.code {
                            KeyCode::Char(c) => self.search.push(c),
                            KeyCode::Backspace => {
                                self.search.pop();
                            }
                            KeyCode::Delete => self.search.clear(),
                            KeyCode::Enter => {
                                if let Some(selected) = self.list_state.selected() {
                                    if let Some(fmatch) = self
                                        .files
                                        .iter()
                                        .enumerate()
                                        .cycle()
                                        .skip(selected + 1)
                                        .find(|(_, file)| {
                                            file.to_str()
                                                .unwrap()
                                                .to_lowercase()
                                                .contains(&self.search.to_lowercase())
                                        })
                                    {
                                        self.list_state.select(Some(fmatch.0));
                                    }
                                }
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
}

fn subsize(area: Rect, i: u16) -> Rect {
    let mut new_area = area;
    new_area.y += i * area.height;

    new_area
}

fn main() -> anyhow::Result<()> {
    gstreamer::init()?;
    Instance::new()?.run()?;

    Ok(())
}
