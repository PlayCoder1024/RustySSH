//! Application event system

use crossterm::event::{Event as CrosstermEvent, KeyEvent, MouseEvent};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
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
    /// Connection attempt completed (success or failure)
    ConnectionResult {
        host_id: uuid::Uuid,
        host_name: String,
        result: Result<ConnectionResultData, String>,
    },
}

/// Data for a successful connection
#[derive(Debug, Clone)]
pub struct ConnectionResultData {
    pub connection_id: uuid::Uuid,
    pub passwords_used: std::collections::HashMap<uuid::Uuid, String>,
}

/// Event handler for terminal and application events
pub struct EventHandler {
    /// Event sender
    sender: mpsc::UnboundedSender<AppEvent>,
    /// Event receiver
    receiver: mpsc::UnboundedReceiver<AppEvent>,
    /// Tick rate for periodic updates
    tick_rate: Duration,
    /// Pause flag for suspending event polling
    paused: Arc<AtomicBool>,
}

impl EventHandler {
    /// Create a new event handler
    pub fn new(tick_rate: Duration) -> Self {
        let (sender, receiver) = mpsc::unbounded_channel();
        Self {
            sender,
            receiver,
            tick_rate,
            paused: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Get a clone of the sender for external events
    pub fn sender(&self) -> mpsc::UnboundedSender<AppEvent> {
        self.sender.clone()
    }

    /// Pause event polling (for when launching external programs)
    pub fn pause(&self) {
        self.paused.store(true, Ordering::SeqCst);
    }

    /// Resume event polling
    pub fn resume(&self) {
        self.paused.store(false, Ordering::SeqCst);
    }

    /// Start the event loop (spawns background task)
    pub fn start(&self) {
        let sender = self.sender.clone();
        let tick_rate = self.tick_rate;
        let paused = self.paused.clone();

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(tick_rate);
            loop {
                // Check if paused
                if paused.load(Ordering::SeqCst) {
                    tokio::time::sleep(Duration::from_millis(50)).await;
                    continue;
                }

                let event = tokio::select! {
                    _ = interval.tick() => AppEvent::Tick,
                    event = Self::read_crossterm_event(&paused) => event,
                };

                // Don't send events while paused
                if paused.load(Ordering::SeqCst) {
                    continue;
                }

                if sender.send(event).is_err() {
                    break;
                }
            }
        });
    }

    /// Read next crossterm event
    async fn read_crossterm_event(paused: &Arc<AtomicBool>) -> AppEvent {
        loop {
            // Check if paused
            if paused.load(Ordering::SeqCst) {
                tokio::time::sleep(Duration::from_millis(50)).await;
                continue;
            }

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
