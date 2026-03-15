use std::num::NonZeroU32;

use serde::{Deserialize, Serialize};

use crate::cell::CellRegion;
use crate::event::{KeyEvent, MouseEvent};
use crate::polarity::Value;
use crate::tag::TagLine;
use crate::widget::{WidgetEvent, WidgetNode};

/// Opaque, compositor-assigned pane identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PaneId(NonZeroU32);

impl PaneId {
    /// Create a PaneId. Only the compositor should call this.
    pub fn new(id: NonZeroU32) -> Self {
        Self(id)
    }

    pub fn get(self) -> u32 {
        self.0.get()
    }
}

/// What kind of content a pane displays.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PaneKind {
    /// Cell grid — compositor renders text from cell data.
    CellGrid,
    /// Graphical widgets — compositor renders widget tree.
    Widget,
    /// Wayland surface — client renders pixels.
    Surface,
}

/// Messages from a pane-native client to the compositor.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum PaneRequest {
    /// Create a new pane.
    Create { name: String, kind: PaneKind },
    /// Close an existing pane.
    Close { id: PaneId },
    /// Write cells to the pane body (CellGrid panes).
    WriteCells { id: PaneId, region: CellRegion },
    /// Set the widget tree (Widget panes).
    SetWidgetTree { id: PaneId, root: WidgetNode },
    /// Scroll the pane body. Positive = down (toward newer content), unit = rows.
    Scroll { id: PaneId, delta: i32 },
    /// Set the pane tag line.
    SetTag { id: PaneId, tag: TagLine },
    /// Mark the pane as dirty or clean.
    SetDirty { id: PaneId, dirty: bool },
    /// Request a specific geometry (cols, rows).
    RequestGeometry { id: PaneId, cols: u16, rows: u16 },
}

impl Value for PaneRequest {}

/// Messages from the compositor to a pane-native client.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum PaneEvent {
    /// A pane was created. Returns the assigned id and kind.
    Created { id: PaneId, kind: PaneKind },
    /// Keyboard input.
    Key { id: PaneId, event: KeyEvent },
    /// Mouse input.
    Mouse { id: PaneId, event: MouseEvent },
    /// Pane was resized.
    Resize { id: PaneId, cols: u16, rows: u16 },
    /// Pane gained or lost focus.
    Focus { id: PaneId, focused: bool },
    /// Compositor requests the pane to close.
    CloseRequested { id: PaneId },
    /// User executed a tag action (B2/left-click).
    TagExecute { id: PaneId, action: TagLine },
    /// User routed text from the tag (B3 click).
    TagRoute { id: PaneId, text: String },
    /// A routed message was delivered to this client.
    Route { message: RouteMessage },
    /// Widget interaction event (Widget panes).
    Widget { id: PaneId, event: WidgetEvent },
}

/// A message routed through pane-route.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RouteMessage {
    /// Source application identifier.
    pub src: String,
    /// Destination port (e.g., "edit", "web").
    pub dst: String,
    /// Working directory for relative paths.
    pub wdir: String,
    /// Content type (e.g., "text").
    pub content_type: String,
    /// Key-value attributes.
    pub attrs: Vec<(String, String)>,
    /// The text data being routed.
    pub data: String,
}

impl Value for RouteMessage {}
impl Value for CellRegion {}
