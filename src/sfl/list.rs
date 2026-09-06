use super::ffi;
use super::item::FavoriteItem;
use anyhow::{Context, Result, bail};
use core_foundation::base::TCFType;
use core_foundation::string::CFString;
use core_foundation::url::CFURL;
use core_foundation_sys::base::CFRelease;
use core_foundation_sys::base::OSStatus;
use std::path::Path;
use std::ptr;

/// Position where a new item should be inserted in the sidebar favorites list.
pub enum InsertPosition<'a> {
    Beginning,
    End,
    After(&'a FavoriteItem),
    Before(&'a FavoriteItem),
}

/// A safe, owned wrapper around macOS `LSSharedFileListRef` for Finder favorites.
pub struct FavoriteList {
    handle: ffi::LSSharedFileListRef,
}

impl FavoriteList {
    /// Opens the shared file list for Finder sidebar favorites (`kLSSharedFileListFavoriteItems`).
    pub fn open() -> Result<Self> {
        let handle = unsafe {
            ffi::LSSharedFileListCreate(
                ptr::null(),
                ffi::kLSSharedFileListFavoriteItems,
                ptr::null(),
            )
        };
        if handle.is_null() {
            bail!("Failed to open macOS FavoriteItems list");
        }
        Ok(Self { handle })
    }

    /// Fetches all current items as owned, safe [`FavoriteItem`]s.
    ///
    /// The snapshot `CFArray` is automatically released when this method returns.
    pub fn items(&self) -> Result<Vec<FavoriteItem>> {
        let mut seed: u32 = 0;
        let snapshot = unsafe { ffi::LSSharedFileListCopySnapshot(self.handle, &mut seed) };
        if snapshot.is_null() {
            return Ok(Vec::new());
        }

        // RAII guard to guarantee the snapshot CFArrayRef is released even on panic or error
        struct SnapshotGuard(core_foundation_sys::array::CFArrayRef);
        impl Drop for SnapshotGuard {
            fn drop(&mut self) {
                if !self.0.is_null() {
                    unsafe {
                        CFRelease(self.0 as *const std::ffi::c_void);
                    }
                }
            }
        }
        let _guard = SnapshotGuard(snapshot);

        let count = unsafe { core_foundation_sys::array::CFArrayGetCount(snapshot) };
        let mut items = Vec::with_capacity(count as usize);

        for i in 0..count {
            let raw = unsafe { core_foundation_sys::array::CFArrayGetValueAtIndex(snapshot, i) }
                as ffi::LSSharedFileListItemRef;

            if raw.is_null() {
                continue;
            }

            // Retain the item reference so the FavoriteItem has its own independent ownership
            unsafe {
                core_foundation_sys::base::CFRetain(raw as *const std::ffi::c_void);
                items.push(FavoriteItem::from_retained_raw(raw));
            }
        }

        Ok(items)
    }

    /// Inserts a new favorite item into the list at the given position.
    ///
    /// If `name` is `None`, LaunchServices automatically determines the localized display name.
    pub fn insert(
        &self,
        name: Option<&str>,
        path: &Path,
        position: InsertPosition<'_>,
        existing_items: &[FavoriteItem],
    ) -> Result<FavoriteItem> {
        let cf_name = name.map(CFString::new);
        let name_ref = match &cf_name {
            Some(s) => s.as_concrete_TypeRef(),
            None => ptr::null(),
        };

        let cf_url = CFURL::from_path(path, path.is_dir())
            .with_context(|| format!("Failed to create CFURL for path: {}", path.display()))?;

        let insert_after = match position {
            InsertPosition::Beginning => unsafe { ffi::kLSSharedFileListItemBeforeFirst },
            InsertPosition::End => unsafe { ffi::kLSSharedFileListItemLast },
            InsertPosition::After(target) => target.raw,
            InsertPosition::Before(target) => {
                let idx = existing_items.iter().position(|it| it.id() == target.id());
                match idx {
                    Some(0) | None => unsafe { ffi::kLSSharedFileListItemBeforeFirst },
                    Some(i) => existing_items[i - 1].raw,
                }
            }
        };

        let raw = unsafe {
            ffi::LSSharedFileListInsertItemURL(
                self.handle,
                insert_after,
                name_ref,
                ptr::null(),
                cf_url.as_concrete_TypeRef(),
                ptr::null(),
                ptr::null(),
            )
        };

        if raw.is_null() {
            let label = name.unwrap_or("<default>");
            bail!(
                "Failed to insert '{}' ({}) into sidebar",
                label,
                path.display()
            );
        }

        // LSSharedFileListInsertItemURL follows the Create Rule (returns +1 retain count)
        Ok(unsafe { FavoriteItem::from_retained_raw(raw) })
    }

    /// Appends a new favorite item to the end of the list.
    pub fn insert_at_end(&self, name: Option<&str>, path: &Path) -> Result<FavoriteItem> {
        self.insert(name, path, InsertPosition::End, &[])
    }

    /// Inserts a new favorite item at the beginning of the list.
    pub fn insert_at_beginning(&self, name: Option<&str>, path: &Path) -> Result<FavoriteItem> {
        self.insert(name, path, InsertPosition::Beginning, &[])
    }

    /// Removes an item from the sidebar.
    pub fn remove(&self, item: &FavoriteItem) -> Result<()> {
        let status = unsafe { ffi::LSSharedFileListItemRemove(self.handle, item.raw) };
        check_status(status, "LSSharedFileListItemRemove")
    }

    /// Removes all items from the sidebar.
    pub fn remove_all(&self) -> Result<()> {
        let status = unsafe { ffi::LSSharedFileListRemoveAllItems(self.handle) };
        check_status(status, "LSSharedFileListRemoveAllItems")
    }

    /// Moves an item to immediately follow `after`.
    pub fn move_item(&self, item: &FavoriteItem, after: &FavoriteItem) -> Result<()> {
        let status = unsafe { ffi::LSSharedFileListItemMove(self.handle, item.raw, after.raw) };
        check_status(status, "LSSharedFileListItemMove")
    }

    /// Moves an item to the very beginning of the list.
    pub fn move_to_beginning(&self, item: &FavoriteItem) -> Result<()> {
        let status = unsafe {
            ffi::LSSharedFileListItemMove(
                self.handle,
                item.raw,
                ffi::kLSSharedFileListItemBeforeFirst,
            )
        };
        check_status(status, "LSSharedFileListItemMove")
    }
}

impl Drop for FavoriteList {
    fn drop(&mut self) {
        if !self.handle.is_null() {
            unsafe {
                CFRelease(self.handle as *const std::ffi::c_void);
            }
        }
    }
}

fn check_status(status: OSStatus, op: &str) -> Result<()> {
    if status != 0 {
        bail!("{} failed with OSStatus {}", op, status);
    }
    Ok(())
}
