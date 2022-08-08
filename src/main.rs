use std::{net::SocketAddr, io::Stdout, sync::{Arc, Mutex}};

use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use std::{error::Error, io};
use tui::{
    backend::{Backend, CrosstermBackend},
    layout::{Constraint, Direction, Layout},
    style::Style,
    text::{Span, Spans, Text},
    widgets::{Block, Borders, List, ListItem, Paragraph},
    Frame, Terminal,
};
use unicode_width::UnicodeWidthStr;

use clap::Parser;
use peer::peer::Peer;

mod transport;
mod message;
mod peer;

struct App {
    input: String,
    messages: Arc<Mutex<Vec<String>>>,
}

impl Default for App {
    fn default() -> App {
        App {
            input: String::new(),
            messages: Arc::new(Mutex::new(Vec::new())),
        }
    }
}

#[derive(Clone, Parser, Debug)]
struct CliArgs {
    #[clap(long, value_parser, short = 'n')]
    name: String,

    #[clap(long, value_parser, short = 'g')]
    group: String,

    #[clap(long, value_parser, short = 'p')]
    port: u16,

    #[clap(long, value_parser, short = 'b')]
    bootstrap: Option<SocketAddr>,

    #[clap(long, value_parser, short = 's')]
    server_mode: Option<bool>,
}

fn setup_app() -> Result<(Terminal<CrosstermBackend<Stdout>>, App), Box<dyn Error>> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let terminal = Terminal::new(backend)?;
    let app = App::default();

    Ok((terminal, app))
}

fn main() -> Result<(), Box<dyn Error>> {
    let args = CliArgs::parse();
    
    // Run peer app
    let mut peer = Peer::new(args.name.clone(), args.group, args.port, args.bootstrap).unwrap();
    

    let peer_cmd = peer.tx();
    let peer_msg = peer.msg_rx();

    let peer_thread = std::thread::spawn(move||{
        peer.run();
    });

    let server_mode = args.server_mode.unwrap_or(false);

    if !server_mode {
        let (mut terminal, mut app) = setup_app()?;

        let thread_messages = app.messages.clone();
        std::thread::spawn(move || {
            loop {
                if let Ok((id, msg)) = peer_msg.recv() {
                    thread_messages.lock().unwrap().push(format!("{}: {}", id, msg));
                    // println!("{}", msg);
                }
            }
        });
        
        loop {
            terminal.draw(|f| draw_ui(f, &app))?;

            if event::poll(std::time::Duration::from_millis(100))? {
                if let Event::Key(key) = event::read()? {
                    match key.code {
                        KeyCode::Enter => {
                            let line: String = app.input.drain(..).collect();
                            peer_cmd.send(line.clone()).unwrap();
                            app.messages.lock().unwrap().push(format!("{}: {}", args.name.clone(), line));
                        },
                        KeyCode::Char(c) => {
                            app.input.push(c);
                        }
                        KeyCode::Backspace => {
                            app.input.pop();
                        }
                        KeyCode::Esc => {
                            break;
                        }
                        _ => {}
                    }
                }
            }
        }

        disable_raw_mode()?;
        execute!(
            terminal.backend_mut(),
            LeaveAlternateScreen,
            DisableMouseCapture
        )?;
        terminal.show_cursor()?;
    }
    
    if server_mode {
        peer_thread.join().unwrap();
    }
    Ok(())
}


fn draw_ui<B: Backend>(f: &mut Frame<B>, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(2)
        .constraints(
            [
                Constraint::Length(1),
                Constraint::Length(3),
                Constraint::Length(3),
                Constraint::Min(1),
            ]
            .as_ref(),
        )
        .split(f.size());

    let text = Text::from("Type a message and press Enter to send.");
    let help_message = Paragraph::new(text);
    f.render_widget(help_message, chunks[0]);

    let text = Text::from("Esc: exit");
    let help_message = Paragraph::new(text);
    f.render_widget(help_message, chunks[1]);

    let input = Paragraph::new(app.input.as_ref())
        .style(Style::default())
        .block(Block::default().borders(Borders::ALL).title("Input"));
    f.render_widget(input, chunks[2]);
    f.set_cursor(
        chunks[2].x + app.input.width() as u16 + 1,
        chunks[2].y + 1,
    );

    let messages: Vec<ListItem> = app
        .messages
        .lock().unwrap()
        .iter()
        .enumerate()
        .rev()
        .map(|(_, m)| {
            let content = vec![Spans::from(Span::raw(format!("{}", m)))];
            ListItem::new(content)
        })
        .collect();
    let messages =
        List::new(messages).block(Block::default().borders(Borders::ALL).title("Messages"));
    f.render_widget(messages, chunks[3]);
}