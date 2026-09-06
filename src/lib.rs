#[cfg(not(target_os = "macos"))]
compile_error!("findbar is designed exclusively for macOS.");

pub mod cli;
pub mod core;
pub mod model;
pub mod sfl;

pub use core::{
    add_favorite, export_nix, list_favorites, remove_all_favorites, remove_favorite, sync_favorites,
};
pub use model::{FindbarConfig, InsertPosition, SidebarEntry, SidebarItemInfo, SyncReport};
