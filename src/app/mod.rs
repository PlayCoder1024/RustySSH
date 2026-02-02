//! Application state and main event loop

mod events;
mod state;

pub use events::{AppEvent, ConnectionResultData, EventHandler};
pub use state::{ActiveChannel, App, AppState, RenderState, SessionInfo, View};
pub use state::{
    FileBrowserSnapshot, FileEntrySnapshot, FilePaneSnapshot, KeyInfoSnapshot,
    TransferItemSnapshot, TransferQueueSnapshot,
};
