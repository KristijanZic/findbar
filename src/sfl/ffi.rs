#![allow(non_camel_case_types)]

use core_foundation_sys::array::CFArrayRef;
use core_foundation_sys::base::{CFAllocatorRef, OSStatus};
use core_foundation_sys::string::CFStringRef;
use core_foundation_sys::url::CFURLRef;
use std::ffi::c_void;

#[repr(C)]
pub struct OpaqueLSSharedFileList(c_void);
pub type LSSharedFileListRef = *mut OpaqueLSSharedFileList;

#[repr(C)]
pub struct OpaqueLSSharedFileListItem(c_void);
pub type LSSharedFileListItemRef = *mut OpaqueLSSharedFileListItem;

#[link(name = "CoreServices", kind = "framework")]
unsafe extern "C" {
    pub static kLSSharedFileListFavoriteItems: CFStringRef;
    pub static kLSSharedFileListItemLast: LSSharedFileListItemRef;
    pub static kLSSharedFileListItemBeforeFirst: LSSharedFileListItemRef;

    pub fn LSSharedFileListCreate(
        inAllocator: CFAllocatorRef,
        inListType: CFStringRef,
        listOptions: *const c_void,
    ) -> LSSharedFileListRef;

    pub fn LSSharedFileListCopySnapshot(
        inList: LSSharedFileListRef,
        outSeed: *mut u32,
    ) -> CFArrayRef;

    pub fn LSSharedFileListInsertItemURL(
        inList: LSSharedFileListRef,
        insertAfterThisItem: LSSharedFileListItemRef,
        inDisplayName: CFStringRef,
        inIconRef: *const c_void,
        inURL: CFURLRef,
        inPropertiesToSet: *const c_void,
        inPropertiesToClear: *const c_void,
    ) -> LSSharedFileListItemRef;

    pub fn LSSharedFileListItemRemove(
        inList: LSSharedFileListRef,
        inItem: LSSharedFileListItemRef,
    ) -> OSStatus;

    pub fn LSSharedFileListRemoveAllItems(inList: LSSharedFileListRef) -> OSStatus;

    pub fn LSSharedFileListItemCopyDisplayName(inItem: LSSharedFileListItemRef) -> CFStringRef;

    pub fn LSSharedFileListItemCopyResolvedURL(
        inItem: LSSharedFileListItemRef,
        inFlags: u32,
        outError: *mut *const c_void,
    ) -> CFURLRef;

    pub fn LSSharedFileListItemMove(
        inList: LSSharedFileListRef,
        inItem: LSSharedFileListItemRef,
        inMoveAfterItem: LSSharedFileListItemRef,
    ) -> OSStatus;

    pub fn LSSharedFileListItemGetID(inItem: LSSharedFileListItemRef) -> u32;
}
