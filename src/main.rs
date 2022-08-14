use std::{net::SocketAddr, io::Stdout, sync::{Arc, Mutex}};

use crossbeam_channel::{Receiver, Sender};
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
use peer::Peer;

mod transport;
mod message;
mod peer;

/// The application that holds the current input and messages
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

type AppTerminal = Terminal<CrosstermBackend<Stdout>>;

fn setup_app() -> Result<(AppTerminal, App), Box<dyn Error>> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let terminal = Terminal::new(backend)?;
    let app = App::default();

    Ok((terminal, app))
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

    let messages = app.messages.lock().unwrap();

    let messages: Vec<ListItem> = messages
        .iter()
        .enumerate()
        .rev()
        .map(|(_, m)| {
            let content = vec![Spans::from(Span::raw(m))];
            ListItem::new(content)
        })
        .collect();
    let messages =
        List::new(messages).block(Block::default().borders(Borders::ALL).title("Messages"));
    f.render_widget(messages, chunks[3]);
}

fn run_chat(peer_name: &str, msg_sender: Sender<String>, msg_receiver: Receiver<(String, String)>) -> Result<(), Box<dyn Error>> {
    let (mut terminal, mut app) = setup_app()?;

    let thread_messages = app.messages.clone();
    // Thread which receives the messages from the peer instance and prints them
    std::thread::spawn(move || {
        loop {
            if let Ok((id, msg)) = msg_receiver.recv() {
                thread_messages.lock().unwrap().push(format!("{}: {}", id, msg));
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
                        msg_sender.send(line.clone()).unwrap();
                        app.messages.lock().unwrap().push(format!("{}: {}", peer_name, line));
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
    Ok(())
}

fn main() -> Result<(), Box<dyn Error>> {
    let args = CliArgs::parse();
    
    // Run peer app
    let mut peer = Peer::new(args.name.clone(), args.group, args.port, args.bootstrap).unwrap();

    // Get the chat sender and receiver
    let msg_sender = peer.msg_sender();
    let msg_receiver = peer.msg_receiver();

    // Run the peer in a separate thread
    let peer_thread = std::thread::spawn(move||{
        peer.run();
    });

    let server_mode = args.server_mode.unwrap_or(false);

    if !server_mode {
        run_chat(&args.name, msg_sender, msg_receiver).unwrap();
    } else {
        peer_thread.join().unwrap();
    }
    Ok(())
}
