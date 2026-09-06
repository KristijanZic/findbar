use super::ffi::{self, LSSharedFileListItemRef};
use crate::model::SidebarItemInfo;
use core_foundation::base::TCFType;
use core_foundation::string::CFString;
use core_foundation::url::CFURL;
use core_foundation_sys::base::{CFRelease, CFRetain};
use std::path::{Path, PathBuf};
use std::ptr;

/// A safe, owned wrapper around a macOS Finder sidebar favorite item (`LSSharedFileListItemRef`).
///
/// Automatically balances reference counts via `CFRelease` on [`Drop`], and increments reference
/// count via `CFRetain` on [`Clone`]. Callers never need to touch raw Core Foundation pointers.
pub struct FavoriteItem {
    pub(crate) raw: LSSharedFileListItemRef,
    id: u32,
    name: String,
    path: Option<PathBuf>,
    url: Option<String>,
}

impl FavoriteItem {
    /// Constructs a `FavoriteItem` taking ownership of a retained raw reference.
    ///
    /// # Safety
    /// `raw` must be a valid, non-null `LSSharedFileListItemRef` with a retain count
    /// that this struct will assume ownership of and release on Drop.
    pub(crate) unsafe fn from_retained_raw(raw: LSSharedFileListItemRef) -> Self {
        let id = unsafe { ffi::LSSharedFileListItemGetID(raw) };

        // LSSharedFileListItemCopyDisplayName follows the Core Foundation Create/Copy Rule (+1 retain count)
        let name_cf = unsafe { ffi::LSSharedFileListItemCopyDisplayName(raw) };
        let name = if !name_cf.is_null() {
            let cf_str = unsafe { CFString::wrap_under_create_rule(name_cf) };
            cf_str.to_string()
        } else {
            String::new()
        };

        // LSSharedFileListItemCopyResolvedURL follows the Create/Copy Rule (+1 retain count)
        let url_cf = unsafe { ffi::LSSharedFileListItemCopyResolvedURL(raw, 0, ptr::null_mut()) };
        let (path, url) = if !url_cf.is_null() {
            let cf_url = unsafe { CFURL::wrap_under_create_rule(url_cf) };
            let path = cf_url.to_path();
            let url_str = cf_url.get_string().to_string();
            (path, Some(url_str))
        } else {
            (None, None)
        };

        Self {
            raw,
            id,
            name,
            path,
            url,
        }
    }

    /// Unique item identifier assigned by LaunchServices.
    pub fn id(&self) -> u32 {
        self.id
    }

    /// Display name of the favorite item in the Finder sidebar.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Resolved filesystem path if the item points to a local file/folder.
    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    /// URL string representation (e.g., `file:///...`).
    pub fn url(&self) -> Option<&str> {
        self.url.as_deref()
    }

    /// Converts this item into the public data model [`SidebarItemInfo`].
    pub fn to_info(&self) -> SidebarItemInfo {
        SidebarItemInfo {
            id: self.id,
            name: self.name.clone(),
            path: self.path.as_ref().map(|p| p.to_string_lossy().into_owned()),
            url: self.url.clone(),
        }
    }
}

impl Drop for FavoriteItem {
    fn drop(&mut self) {
        if !self.raw.is_null() {
            unsafe {
                CFRelease(self.raw as *const std::ffi::c_void);
            }
        }
    }
}

impl Clone for FavoriteItem {
    fn clone(&self) -> Self {
        if !self.raw.is_null() {
            unsafe {
                CFRetain(self.raw as *const std::ffi::c_void);
            }
        }
        Self {
            raw: self.raw,
            id: self.id,
            name: self.name.clone(),
            path: self.path.clone(),
            url: self.url.clone(),
        }
    }
}
