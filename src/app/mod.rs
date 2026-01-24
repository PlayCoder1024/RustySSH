//! Application state and main event loop

mod events;
mod state;

pub use events::{AppEvent, EventHandler, ConnectionResultData};
pub use state::{App, AppState, View, RenderState, SessionInfo, ActiveChannel};
pub use state::{FileBrowserSnapshot, FilePaneSnapshot, FileEntrySnapshot, TransferQueueSnapshot, TransferItemSnapshot};
