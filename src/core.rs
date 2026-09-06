use crate::model::{FindbarConfig, InsertPosition, SidebarItemInfo, SyncAction, SyncReport};
use crate::sfl::{self, FavoriteList};
use anyhow::{Context, Result, bail};
use std::path::{Path, PathBuf};

/// Expands `~` and environment variables in paths, converting to absolute path strings.
pub fn expand_and_normalize_path(input: &str) -> Result<String> {
    let expanded =
        shellexpand::full(input).with_context(|| format!("Failed to expand path: {}", input))?;
    let path = Path::new(expanded.as_ref());

    // If it's a file:// URI, strip the prefix and decode percent-encoded octets
    if input.starts_with("file://") {
        let stripped = input.trim_start_matches("file://");
        let decoded = urlencoding_decode(stripped);
        return Ok(decoded);
    }

    // Try to get canonical path if it exists, otherwise get normalized path
    if let Ok(canon) = path.canonicalize() {
        return Ok(canon.to_string_lossy().to_string());
    }

    if path.is_absolute() {
        Ok(path.to_string_lossy().to_string())
    } else {
        let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("/"));
        Ok(cwd.join(path).to_string_lossy().to_string())
    }
}

/// Decodes standard percent-encoded URLs (e.g. `%20` -> space, `%C3%A9` -> `é`).
pub fn urlencoding_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%'
            && i + 2 < bytes.len()
            && let Ok(byte) = u8::from_str_radix(
                std::str::from_utf8(&bytes[i + 1..i + 3]).unwrap_or_default(),
                16,
            )
        {
            decoded.push(byte);
            i += 3;
            continue;
        }
        decoded.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&decoded).into_owned()
}

/// Helper to extract default display name from path.
pub fn infer_name_from_path(path_str: &str) -> String {
    let path = Path::new(path_str);
    if let Some(file_name) = path.file_name() {
        file_name.to_string_lossy().to_string()
    } else {
        path_str.to_string()
    }
}

/// Normalizes path for comparison (stripping trailing slashes, expanding ~).
pub fn normalize_for_comparison(p: &str) -> String {
    let expanded = expand_and_normalize_path(p).unwrap_or_else(|_| p.to_string());
    let trimmed = expanded.trim_end_matches('/');
    trimmed.to_lowercase()
}

/// List all current favorites from Finder sidebar.
pub fn list_favorites() -> Result<Vec<SidebarItemInfo>> {
    let list = FavoriteList::open()?;
    let items = list.items()?;
    Ok(items.into_iter().map(|item| item.to_info()).collect())
}

/// Add an item to the Finder sidebar.
///
/// If `custom_name` is `None`, macOS LaunchServices automatically localizes the display name.
/// If `allow_duplicates` is `false`, an error is returned if the path is already in favorites.
pub fn add_favorite(
    path_input: &str,
    custom_name: Option<&str>,
    position: InsertPosition,
    allow_duplicates: bool,
) -> Result<SidebarItemInfo> {
    let normalized_path = expand_and_normalize_path(path_input)?;

    let list = FavoriteList::open()?;
    let existing_items = list.items()?;

    if !allow_duplicates {
        let norm_comp = normalize_for_comparison(&normalized_path);
        if let Some(existing) = existing_items.iter().find(|it| {
            it.path()
                .map(|p| normalize_for_comparison(&p.to_string_lossy()) == norm_comp)
                .unwrap_or(false)
        }) {
            bail!(
                "Item '{}' is already in sidebar favorites (id: {}). Use --force to add anyway.",
                existing.name(),
                existing.id()
            );
        }
    }

    let sfl_position = match position {
        InsertPosition::Beginning => sfl::InsertPosition::Beginning,
        InsertPosition::End => sfl::InsertPosition::End,
        InsertPosition::Before(target) => {
            let target_norm = normalize_for_comparison(&target);
            let idx = existing_items.iter().position(|it| {
                it.name().to_lowercase() == target_norm
                    || it
                        .path()
                        .map(|p| normalize_for_comparison(&p.to_string_lossy()))
                        == Some(target_norm.clone())
            });
            match idx {
                Some(i) => sfl::InsertPosition::Before(&existing_items[i]),
                None => bail!("Target item '{}' not found in sidebar favorites", target),
            }
        }
        InsertPosition::After(target) => {
            let target_norm = normalize_for_comparison(&target);
            let idx = existing_items.iter().position(|it| {
                it.name().to_lowercase() == target_norm
                    || it
                        .path()
                        .map(|p| normalize_for_comparison(&p.to_string_lossy()))
                        == Some(target_norm.clone())
            });
            match idx {
                Some(i) => sfl::InsertPosition::After(&existing_items[i]),
                None => bail!("Target item '{}' not found in sidebar favorites", target),
            }
        }
    };

    let item = list.insert(
        custom_name,
        Path::new(&normalized_path),
        sfl_position,
        &existing_items,
    )?;

    Ok(item.to_info())
}

/// Remove a favorite by display name, path, or URI.
pub fn remove_favorite(target: &str) -> Result<bool> {
    let list = FavoriteList::open()?;
    let existing_items = list.items()?;
    let target_norm = normalize_for_comparison(target);

    for it in &existing_items {
        let name_match = it.name().to_lowercase() == target.to_lowercase();
        let path_match = it
            .path()
            .map(|p| normalize_for_comparison(&p.to_string_lossy()) == target_norm)
            .unwrap_or(false);

        if name_match || path_match {
            list.remove(it)?;
            return Ok(true);
        }
    }

    Ok(false)
}

/// Remove all favorites from Finder sidebar.
pub fn remove_all_favorites() -> Result<()> {
    let list = FavoriteList::open()?;
    list.remove_all()
}

/// Synchronize the Finder sidebar favorites with a declarative target configuration.
///
/// Uses non-destructive reconciliation:
/// 1. Unmanaged items not declared in config are removed (if `keep_unmanaged` is false).
/// 2. Missing items are appended.
/// 3. Existing managed items are preserved in place without destructive recreation.
/// 4. Desired ordering is aligned smoothly.
pub fn sync_favorites(config: &FindbarConfig, dry_run: bool) -> Result<SyncReport> {
    let mut report = SyncReport::default();
    let list = FavoriteList::open()?;

    // 1. Prepare desired entries with normalized paths and precomputed comparisons
    let desired_entries: Vec<(Option<String>, String, String)> = config
        .items
        .iter()
        .map(|entry| {
            let norm_path =
                expand_and_normalize_path(&entry.path).unwrap_or_else(|_| entry.path.clone());
            let comp_path = normalize_for_comparison(&norm_path);
            (entry.name.clone(), norm_path, comp_path)
        })
        .collect();

    // 2. Fetch current items
    let existing_items = list.items()?;

    // Check if the current list already exactly matches the desired entries in content and order
    let is_exact_match = existing_items.len() == desired_entries.len()
        && existing_items.iter().zip(desired_entries.iter()).all(
            |(cur, (des_name, _, des_comp))| {
                let cur_comp = cur
                    .path()
                    .map(|p| normalize_for_comparison(&p.to_string_lossy()));
                let name_matches = match des_name {
                    Some(expected) => cur.name() == expected,
                    None => true, // System default localized name matches
                };
                name_matches && cur_comp.as_deref() == Some(des_comp.as_str())
            },
        );

    if is_exact_match {
        for (name, path, _) in desired_entries {
            report.unchanged += 1;
            report.actions.push(SyncAction {
                action: "unchanged".to_string(),
                name: name.unwrap_or_else(|| infer_name_from_path(&path)),
                path: Some(path),
                reason: Some("Already matches target state".to_string()),
            });
        }
        return Ok(report);
    }

    // 3. Remove unmanaged items (if !config.keep_unmanaged)
    if !config.keep_unmanaged {
        for cur in &existing_items {
            let cur_comp = cur
                .path()
                .map(|p| normalize_for_comparison(&p.to_string_lossy()));
            let is_managed = desired_entries
                .iter()
                .any(|(_, _, des_comp)| cur_comp.as_deref() == Some(des_comp.as_str()));

            if !is_managed {
                if !dry_run {
                    list.remove(cur)?;
                }
                report.removed += 1;
                report.actions.push(SyncAction {
                    action: if dry_run {
                        "remove".to_string()
                    } else {
                        "removed".to_string()
                    },
                    name: cur.name().to_string(),
                    path: cur.path().map(|p| p.to_string_lossy().to_string()),
                    reason: Some("Unmanaged item".to_string()),
                });
            }
        }
    }

    // 4. Add missing items or record preserved
    for (des_name, des_path, des_comp) in &desired_entries {
        let already_exists = existing_items.iter().any(|cur| {
            cur.path()
                .map(|p| normalize_for_comparison(&p.to_string_lossy()))
                .as_deref()
                == Some(des_comp.as_str())
        });

        if !already_exists {
            if !dry_run {
                list.insert_at_end(des_name.as_deref(), Path::new(des_path))?;
            }
            report.added += 1;
            report.actions.push(SyncAction {
                action: if dry_run {
                    "add".to_string()
                } else {
                    "added".to_string()
                },
                name: des_name
                    .clone()
                    .unwrap_or_else(|| infer_name_from_path(des_path)),
                path: Some(des_path.clone()),
                reason: Some("Missing from sidebar".to_string()),
            });
        } else {
            report.unchanged += 1;
            report.actions.push(SyncAction {
                action: if dry_run {
                    "preserve".to_string()
                } else {
                    "unchanged".to_string()
                },
                name: des_name
                    .clone()
                    .unwrap_or_else(|| infer_name_from_path(des_path)),
                path: Some(des_path.clone()),
                reason: Some("Item already exists".to_string()),
            });
        }
    }

    // 5. Ensure desired ordering in live execution
    if !dry_run && !desired_entries.is_empty() {
        let current_items = list.items()?;
        let mut ordered_items = Vec::new();
        for (_, _, des_comp) in &desired_entries {
            if let Some(item) = current_items.iter().find(|it| {
                it.path()
                    .map(|p| normalize_for_comparison(&p.to_string_lossy()))
                    .as_deref()
                    == Some(des_comp.as_str())
            }) {
                ordered_items.push(item.clone());
            }
        }

        if let Some(first) = ordered_items.first()
            && current_items.first().map(|it| it.id()) != Some(first.id())
        {
            let _ = list.move_to_beginning(first);
        }
        for i in 1..ordered_items.len() {
            let prev = &ordered_items[i - 1];
            let cur = &ordered_items[i];
            let _ = list.move_item(cur, prev);
        }
    }

    Ok(report)
}

/// Generates Nix configuration format from existing items.
pub fn export_nix(items: &[SidebarItemInfo], keep_unmanaged: bool) -> String {
    let mut out = String::new();
    out.push_str("{\n");
    out.push_str("  programs.findbar = {\n");
    out.push_str("    enable = true;\n");
    if keep_unmanaged {
        out.push_str("    keepUnmanaged = true;\n");
    }
    out.push_str("    items = [\n");
    for it in items {
        let path = it.path.as_deref().unwrap_or("~");
        // Convert home path to ~ if applicable
        let home = std::env::var("HOME").unwrap_or_default();
        let display_path = if !home.is_empty() && path.starts_with(&home) {
            format!("~{}", &path[home.len()..])
        } else {
            path.to_string()
        };

        out.push_str("      {\n");
        out.push_str(&format!("        name = \"{}\";\n", it.name));
        out.push_str(&format!("        path = \"{}\";\n", display_path));
        out.push_str("      }\n");
    }
    out.push_str("    ];\n");
    out.push_str("  };\n");
    out.push_str("}\n");
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_urlencoding_decode() {
        assert_eq!(
            urlencoding_decode("file:///Users/test/My%20Folder"),
            "file:///Users/test/My Folder"
        );
        assert_eq!(
            urlencoding_decode("file:///Users/test/Caf%C3%A9%20%28Work%29"),
            "file:///Users/test/Café (Work)"
        );
        assert_eq!(urlencoding_decode("plain_text"), "plain_text");
    }

    #[test]
    fn test_infer_name_from_path() {
        assert_eq!(
            infer_name_from_path("/Users/aresminos/Downloads"),
            "Downloads"
        );
        assert_eq!(infer_name_from_path("/Applications"), "Applications");
        assert_eq!(infer_name_from_path("~/Projects/findbar"), "findbar");
    }

    #[test]
    fn test_normalize_for_comparison() {
        let n1 = normalize_for_comparison("/Applications/");
        let n2 = normalize_for_comparison("/Applications");
        assert_eq!(n1, n2);
    }

    #[test]
    fn test_export_nix() {
        let items = vec![
            SidebarItemInfo {
                id: 1,
                name: "Applications".to_string(),
                path: Some("/Applications".to_string()),
                url: Some("file:///Applications/".to_string()),
            },
            SidebarItemInfo {
                id: 2,
                name: "Downloads".to_string(),
                path: Some("/Users/test/Downloads".to_string()),
                url: Some("file:///Users/test/Downloads/".to_string()),
            },
        ];

        let nix_code = export_nix(&items, false);
        assert!(nix_code.contains("programs.findbar = {"));
        assert!(nix_code.contains("name = \"Applications\";"));
        assert!(nix_code.contains("path = \"/Applications\";"));
        assert!(nix_code.contains("name = \"Downloads\";"));
    }

    #[test]
    fn test_findbar_config_deserialization() {
        let json_input = r#"{
            "items": [
                { "name": "Apps", "path": "/Applications" },
                { "path": "~/Downloads" }
            ],
            "keep_unmanaged": true
        }"#;

        let config: FindbarConfig =
            serde_json::from_str(json_input).expect("Failed to deserialize JSON");
        assert_eq!(config.items.len(), 2);
        assert_eq!(config.items[0].name.as_deref(), Some("Apps"));
        assert_eq!(config.items[0].path, "/Applications");
        assert_eq!(config.items[1].name, None);
        assert!(config.keep_unmanaged);
    }
}
