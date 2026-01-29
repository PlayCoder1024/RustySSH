//! Host list widget

use crate::config::HostConfig;
use ratatui::prelude::*;
use ratatui::widgets::{Block, List, ListItem, StatefulWidget};

/// State for the host list widget
#[derive(Debug, Default)]
pub struct HostListState {
    /// Currently selected index
    pub selected: Option<usize>,
    /// Scroll offset
    pub offset: usize,
}

impl HostListState {
    /// Select the next item
    pub fn next(&mut self, len: usize) {
        if len == 0 {
            self.selected = None;
            return;
        }

        self.selected = Some(match self.selected {
            Some(i) => (i + 1).min(len - 1),
            None => 0,
        });
    }

    /// Select the previous item
    pub fn previous(&mut self, len: usize) {
        if len == 0 {
            self.selected = None;
            return;
        }

        self.selected = Some(match self.selected {
            Some(i) => i.saturating_sub(1),
            None => 0,
        });
    }

    /// Get selected index
    pub fn selected(&self) -> Option<usize> {
        self.selected
    }
}

/// Host list widget
pub struct HostList<'a> {
    hosts: &'a [HostConfig],
    block: Option<Block<'a>>,
    highlight_style: Style,
    highlight_symbol: Option<&'a str>,
}

impl<'a> HostList<'a> {
    /// Create a new host list
    pub fn new(hosts: &'a [HostConfig]) -> Self {
        Self {
            hosts,
            block: None,
            highlight_style: Style::default(),
            highlight_symbol: None,
        }
    }

    /// Set the block
    pub fn block(mut self, block: Block<'a>) -> Self {
        self.block = Some(block);
        self
    }

    /// Set highlight style
    pub fn highlight_style(mut self, style: Style) -> Self {
        self.highlight_style = style;
        self
    }

    /// Set highlight symbol
    pub fn highlight_symbol(mut self, symbol: &'a str) -> Self {
        self.highlight_symbol = Some(symbol);
        self
    }
}

impl<'a> StatefulWidget for HostList<'a> {
    type State = HostListState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        // Create list items
        let items: Vec<ListItem> = self
            .hosts
            .iter()
            .map(|host| {
                let line = Line::from(vec![
                    Span::raw("󰌘 "),
                    Span::styled(&host.name, Style::default().add_modifier(Modifier::BOLD)),
                    Span::raw(" - "),
                    Span::styled(
                        host.connection_string(),
                        Style::default().fg(Color::DarkGray),
                    ),
                ]);
                ListItem::new(line)
            })
            .collect();

        let mut list = List::new(items).highlight_style(self.highlight_style);

        if let Some(symbol) = self.highlight_symbol {
            list = list.highlight_symbol(symbol);
        }

        if let Some(block) = self.block {
            list = list.block(block);
        }

        // Convert our state to ratatui's ListState
        let mut list_state = ratatui::widgets::ListState::default().with_selected(state.selected);

        StatefulWidget::render(list, area, buf, &mut list_state);

        // Update our state from ratatui's
        state.selected = list_state.selected();
    }
}
