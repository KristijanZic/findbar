use serde::{Deserialize, Serialize};

/// Represents a desired sidebar item entry (e.g. from Nix or JSON config).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SidebarEntry {
    /// Optional custom display name in the Finder sidebar.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Path (e.g., `~/Downloads`, `/Applications`, `file:///...`).
    pub path: String,
}

/// Represents an existing item currently in the Finder sidebar.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SidebarItemInfo {
    pub id: u32,
    pub name: String,
    pub path: Option<String>,
    pub url: Option<String>,
}

/// The top-level declarative configuration format.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FindbarConfig {
    #[serde(default)]
    pub items: Vec<SidebarEntry>,
    #[serde(default)]
    pub keep_unmanaged: bool,
}

/// Position for inserting an item.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InsertPosition {
    Beginning,
    End,
    Before(String),
    After(String),
}

/// A recorded action during synchronization.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncAction {
    pub action: String, // "added", "removed", "reordered", "unchanged"
    pub name: String,
    pub path: Option<String>,
    pub reason: Option<String>,
}

/// Summary report returned after a sync operation.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SyncReport {
    pub added: usize,
    pub removed: usize,
    pub reordered: usize,
    pub unchanged: usize,
    pub actions: Vec<SyncAction>,
}
