//! SFTP file transfer operations

use chrono::{DateTime, Utc};
use std::collections::VecDeque;
use std::path::PathBuf;
use tokio::sync::mpsc;
use uuid::Uuid;

/// Transfer direction
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransferDirection {
    Upload,
    Download,
}

/// Transfer status
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransferStatus {
    Pending,
    InProgress,
    Completed,
    Failed(String),
    Cancelled,
}

/// Single transfer item
#[derive(Clone)]
pub struct TransferItem {
    /// Transfer ID
    pub id: Uuid,
    /// Source path
    pub source: PathBuf,
    /// Destination path
    pub destination: PathBuf,
    /// Transfer direction
    pub direction: TransferDirection,
    /// Current status
    pub status: TransferStatus,
    /// Total bytes to transfer
    pub total_bytes: u64,
    /// Bytes transferred so far
    pub transferred_bytes: u64,
    /// Start time
    pub started_at: Option<DateTime<Utc>>,
    /// Completion time
    pub completed_at: Option<DateTime<Utc>>,
    /// Current transfer speed (bytes/sec)
    pub speed: f64,
}

impl TransferItem {
    /// Create a new transfer item
    pub fn new(
        source: PathBuf,
        destination: PathBuf,
        direction: TransferDirection,
        total_bytes: u64,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            source,
            destination,
            direction,
            status: TransferStatus::Pending,
            total_bytes,
            transferred_bytes: 0,
            started_at: None,
            completed_at: None,
            speed: 0.0,
        }
    }

    /// Get progress percentage (0.0 - 100.0)
    pub fn progress(&self) -> f64 {
        if self.total_bytes == 0 {
            return 100.0;
        }
        (self.transferred_bytes as f64 / self.total_bytes as f64) * 100.0
    }

    /// Get estimated time remaining in seconds
    pub fn eta(&self) -> Option<f64> {
        if self.speed <= 0.0 {
            return None;
        }
        let remaining = self.total_bytes.saturating_sub(self.transferred_bytes);
        Some(remaining as f64 / self.speed)
    }

    /// Get formatted speed
    pub fn speed_display(&self) -> String {
        format_speed(self.speed)
    }

    /// Get formatted ETA
    pub fn eta_display(&self) -> String {
        if let Some(eta) = self.eta() {
            format_duration(eta)
        } else {
            "--:--".to_string()
        }
    }
}

/// Format transfer speed
fn format_speed(bytes_per_sec: f64) -> String {
    const KB: f64 = 1024.0;
    const MB: f64 = KB * 1024.0;
    const GB: f64 = MB * 1024.0;

    if bytes_per_sec >= GB {
        format!("{:.1} GB/s", bytes_per_sec / GB)
    } else if bytes_per_sec >= MB {
        format!("{:.1} MB/s", bytes_per_sec / MB)
    } else if bytes_per_sec >= KB {
        format!("{:.1} KB/s", bytes_per_sec / KB)
    } else {
        format!("{:.0} B/s", bytes_per_sec)
    }
}

/// Format duration in seconds to MM:SS or HH:MM:SS
fn format_duration(seconds: f64) -> String {
    let secs = seconds as u64;
    let hours = secs / 3600;
    let mins = (secs % 3600) / 60;
    let secs = secs % 60;

    if hours > 0 {
        format!("{:02}:{:02}:{:02}", hours, mins, secs)
    } else {
        format!("{:02}:{:02}", mins, secs)
    }
}

/// Progress update from transfer
#[derive(Debug, Clone)]
pub struct TransferProgress {
    pub id: Uuid,
    pub transferred_bytes: u64,
    pub speed: f64,
}

/// Transfer queue managing multiple transfers
pub struct TransferQueue {
    /// Pending transfers
    pending: VecDeque<TransferItem>,
    /// Currently active transfers
    active: Vec<TransferItem>,
    /// Completed transfers (history)
    completed: VecDeque<TransferItem>,
    /// Maximum concurrent transfers
    max_concurrent: usize,
    /// Maximum history size
    max_history: usize,
    /// Progress sender
    progress_tx: mpsc::UnboundedSender<TransferProgress>,
    /// Progress receiver
    progress_rx: mpsc::UnboundedReceiver<TransferProgress>,
}

impl TransferQueue {
    /// Create a new transfer queue
    pub fn new(max_concurrent: usize) -> Self {
        let (progress_tx, progress_rx) = mpsc::unbounded_channel();

        Self {
            pending: VecDeque::new(),
            active: Vec::new(),
            completed: VecDeque::new(),
            max_concurrent,
            max_history: 100,
            progress_tx,
            progress_rx,
        }
    }

    /// Add a transfer to the queue
    pub fn add(&mut self, item: TransferItem) -> Uuid {
        let id = item.id;
        self.pending.push_back(item);
        id
    }

    /// Get all pending transfers
    pub fn pending(&self) -> &VecDeque<TransferItem> {
        &self.pending
    }

    /// Get all active transfers
    pub fn active(&self) -> &[TransferItem] {
        &self.active
    }

    /// Get completed transfers
    pub fn completed(&self) -> &VecDeque<TransferItem> {
        &self.completed
    }

    /// Get total pending + active count
    pub fn total_pending(&self) -> usize {
        self.pending.len() + self.active.len()
    }

    /// Cancel a transfer
    pub fn cancel(&mut self, id: Uuid) {
        // Check pending
        if let Some(pos) = self.pending.iter().position(|t| t.id == id) {
            if let Some(mut item) = self.pending.remove(pos) {
                item.status = TransferStatus::Cancelled;
                self.add_to_history(item);
            }
        }

        // Check active (mark as cancelled, actual cancellation happens in transfer task)
        if let Some(item) = self.active.iter_mut().find(|t| t.id == id) {
            item.status = TransferStatus::Cancelled;
        }
    }

    /// Update progress for a transfer
    pub fn update_progress(&mut self, progress: TransferProgress) {
        if let Some(item) = self.active.iter_mut().find(|t| t.id == progress.id) {
            item.transferred_bytes = progress.transferred_bytes;
            item.speed = progress.speed;
        }
    }

    /// Mark a transfer as completed
    pub fn complete(&mut self, id: Uuid, error: Option<String>) {
        if let Some(pos) = self.active.iter().position(|t| t.id == id) {
            let mut item = self.active.remove(pos);
            item.completed_at = Some(Utc::now());

            if let Some(err) = error {
                item.status = TransferStatus::Failed(err);
            } else {
                item.status = TransferStatus::Completed;
            }

            self.add_to_history(item);
        }
    }

    /// Add to history with size limit
    fn add_to_history(&mut self, item: TransferItem) {
        self.completed.push_front(item);
        while self.completed.len() > self.max_history {
            self.completed.pop_back();
        }
    }

    /// Start next pending transfer if capacity available
    pub fn process_pending(&mut self) -> Vec<TransferItem> {
        let mut started = Vec::new();

        while self.active.len() < self.max_concurrent {
            if let Some(mut item) = self.pending.pop_front() {
                item.status = TransferStatus::InProgress;
                item.started_at = Some(Utc::now());
                started.push(item.clone());
                self.active.push(item);
            } else {
                break;
            }
        }

        started
    }

    /// Get progress sender for transfer tasks
    pub fn progress_sender(&self) -> mpsc::UnboundedSender<TransferProgress> {
        self.progress_tx.clone()
    }

    /// Process any pending progress updates
    pub fn process_progress(&mut self) {
        while let Ok(progress) = self.progress_rx.try_recv() {
            self.update_progress(progress);
        }
    }

    /// Clear completed history
    pub fn clear_history(&mut self) {
        self.completed.clear();
    }
}

impl Default for TransferQueue {
    fn default() -> Self {
        Self::new(3) // Default to 3 concurrent transfers
    }
}
