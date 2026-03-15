use serde::{Deserialize, Serialize};

use crate::cell::CellRegion;
use crate::polarity::Value;

/// A tree of composable widget elements.
/// Clients build this; the compositor renders it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum WidgetNode {
    /// Clickable button.
    Button { label: String, id: u32 },
    /// Static text.
    Label { text: String },
    /// Editable text field.
    TextInput {
        value: String,
        placeholder: String,
        id: u32,
    },
    /// Range control.
    Slider {
        min: f32,
        max: f32,
        value: f32,
        id: u32,
    },
    /// Boolean toggle.
    Checkbox {
        label: String,
        checked: bool,
        id: u32,
    },
    /// Selectable list.
    List {
        items: Vec<String>,
        selected: Option<usize>,
        id: u32,
    },
    /// Horizontal layout.
    HBox {
        children: Vec<WidgetNode>,
        spacing: u16,
    },
    /// Vertical layout.
    VBox {
        children: Vec<WidgetNode>,
        spacing: u16,
    },
    /// Scrollable container.
    Scroll { child: Box<WidgetNode> },
    /// Visual divider.
    Separator,
    /// Embedded cell grid region (hybrid panes).
    CellGrid { region: CellRegion },
}

impl Value for WidgetNode {}

/// Events from widget interactions, sent compositor → client.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum WidgetEvent {
    /// Button was clicked.
    Clicked { id: u32 },
    /// Slider or numeric value changed.
    ValueChanged { id: u32, value: f32 },
    /// Text input content changed.
    TextChanged { id: u32, value: String },
    /// List selection changed.
    Selected { id: u32, index: Option<usize> },
    /// Checkbox toggled.
    Toggled { id: u32, checked: bool },
}
