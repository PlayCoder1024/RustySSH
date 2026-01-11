//! Application event system

use crossterm::event::{Event as CrosstermEvent, KeyEvent, MouseEvent};
use std::time::Duration;
use tokio::sync::mpsc;

/// Application events
#[derive(Debug, Clone)]
pub enum AppEvent {
    /// Terminal key press
    Key(KeyEvent),
    /// Mouse event
    Mouse(MouseEvent),
    /// Terminal resize
    Resize(u16, u16),
    /// Tick event for periodic updates
    Tick,
    /// SSH session data received
    SshData { session_id: uuid::Uuid, data: Vec<u8> },
    /// SSH session disconnected
    SshDisconnected { session_id: uuid::Uuid, reason: String },
    /// SFTP transfer progress
    SftpProgress { transfer_id: uuid::Uuid, bytes: u64, total: u64 },
    /// Error notification
    Error(String),
}

/// Event handler for terminal and application events
pub struct EventHandler {
    /// Event sender
    sender: mpsc::UnboundedSender<AppEvent>,
    /// Event receiver
    receiver: mpsc::UnboundedReceiver<AppEvent>,
    /// Tick rate for periodic updates
    tick_rate: Duration,
}

impl EventHandler {
    /// Create a new event handler
    pub fn new(tick_rate: Duration) -> Self {
        let (sender, receiver) = mpsc::unbounded_channel();
        Self {
            sender,
            receiver,
            tick_rate,
        }
    }

    /// Get a clone of the sender for external events
    pub fn sender(&self) -> mpsc::UnboundedSender<AppEvent> {
        self.sender.clone()
    }

    /// Start the event loop (spawns background task)
    pub fn start(&self) {
        let sender = self.sender.clone();
        let tick_rate = self.tick_rate;

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(tick_rate);
            loop {
                let event = tokio::select! {
                    _ = interval.tick() => AppEvent::Tick,
                    event = Self::read_crossterm_event() => event,
                };

                if sender.send(event).is_err() {
                    break;
                }
            }
        });
    }

    /// Read next crossterm event
    async fn read_crossterm_event() -> AppEvent {
        loop {
            if crossterm::event::poll(Duration::from_millis(10)).unwrap_or(false) {
                match crossterm::event::read() {
                    Ok(CrosstermEvent::Key(key)) => return AppEvent::Key(key),
                    Ok(CrosstermEvent::Mouse(mouse)) => return AppEvent::Mouse(mouse),
                    Ok(CrosstermEvent::Resize(w, h)) => return AppEvent::Resize(w, h),
                    _ => {}
                }
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }

    /// Get next event
    pub async fn next(&mut self) -> Option<AppEvent> {
        self.receiver.recv().await
    }
}
