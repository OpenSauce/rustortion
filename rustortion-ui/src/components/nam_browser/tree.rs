//! Pure list logic for the NAM browser: the folder tree, the filter, and visible rows.

use std::collections::HashSet;
use std::hash::BuildHasher;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModelNode {
    /// `path` is the folder's full path from the root, e.g. "Pack/Deep".
    Folder {
        label: String,
        path: String,
        children: Vec<Self>,
    },
    /// `key` is the full registry key; `label` its last segment.
    Model {
        label: String,
        key: String,
        rate_hz: u32,
    },
}

impl ModelNode {
    fn label(&self) -> &str {
        match self {
            Self::Folder { label, .. } | Self::Model { label, .. } => label,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ListRow {
    Folder {
        depth: usize,
        label: String,
        path: String,
        open: bool,
    },
    /// `folder` is set only while filtering, so the flat list can show where a match lives.
    Model {
        depth: usize,
        label: String,
        key: String,
        folder: Option<String>,
        rate_hz: u32,
    },
}

/// The last path segment of a registry key.
pub fn leaf_label(key: &str) -> &str {
    key.rsplit('/').next().unwrap_or(key)
}

/// Everything before the last '/', or `None` for a loose file.
pub fn folder_of(key: &str) -> Option<&str> {
    key.rsplit_once('/').map(|(folder, _)| folder)
}

/// Group registry keys into a folder tree. Folders and loose files sort together
/// by name, ignoring case, at every level.
pub fn model_tree(entries: &[(String, u32)]) -> Vec<ModelNode> {
    let mut root = Vec::new();
    for (key, rate_hz) in entries {
        let mut segments = key.split('/').peekable();
        let mut level = &mut root;
        let mut path = String::new();
        let mut depth = 0;
        while let Some(segment) = segments.next() {
            if segments.peek().is_none() {
                level.push(ModelNode::Model {
                    label: segment.to_owned(),
                    key: key.clone(),
                    rate_hz: *rate_hz,
                });
                break;
            }
            if depth > 0 {
                path.push('/');
            }
            path.push_str(segment);
            depth += 1;
            let position = level
                .iter()
                .position(|node| matches!(node, ModelNode::Folder { path: p, .. } if *p == path))
                .unwrap_or_else(|| {
                    level.push(ModelNode::Folder {
                        label: segment.to_owned(),
                        path: path.clone(),
                        children: Vec::new(),
                    });
                    level.len() - 1
                });
            let ModelNode::Folder { children, .. } = &mut level[position] else {
                unreachable!("position was found by matching a Folder");
            };
            level = children;
        }
    }
    sort_tree(&mut root);
    root
}

fn sort_tree(nodes: &mut [ModelNode]) {
    nodes.sort_by_cached_key(|node| node.label().to_lowercase());
    for node in nodes {
        if let ModelNode::Folder { children, .. } = node {
            sort_tree(children);
        }
    }
}

/// Case-insensitive substring match on the full key. `filter` is matched literally.
pub fn matches_filter(key: &str, filter: &str) -> bool {
    key.to_lowercase().contains(&filter.to_lowercase())
}

/// The rows to draw: the tree honouring open folders, or a flat list of matches
/// while a filter is set.
pub fn visible_rows<S: BuildHasher>(
    tree: &[ModelNode],
    filter: &str,
    open_folders: &HashSet<String, S>,
    loaded: Option<&str>,
) -> Vec<ListRow> {
    let filter = filter.trim();
    let mut rows = Vec::new();
    if filter.is_empty() {
        push_tree(tree, 0, open_folders, loaded, &mut rows);
    } else {
        push_matches(tree, filter, &mut rows);
    }
    rows
}

fn push_tree<S: BuildHasher>(
    nodes: &[ModelNode],
    depth: usize,
    open_folders: &HashSet<String, S>,
    loaded: Option<&str>,
    rows: &mut Vec<ListRow>,
) {
    for node in nodes {
        match node {
            ModelNode::Folder {
                label,
                path,
                children,
            } => {
                let open = open_folders.contains(path)
                    || loaded.is_some_and(|key| {
                        key.strip_prefix(path.as_str())
                            .is_some_and(|rest| rest.starts_with('/'))
                    });
                rows.push(ListRow::Folder {
                    depth,
                    label: label.clone(),
                    path: path.clone(),
                    open,
                });
                if open {
                    push_tree(children, depth + 1, open_folders, loaded, rows);
                }
            }
            ModelNode::Model {
                label,
                key,
                rate_hz,
            } => rows.push(ListRow::Model {
                depth,
                label: label.clone(),
                key: key.clone(),
                folder: None,
                rate_hz: *rate_hz,
            }),
        }
    }
}

fn push_matches(nodes: &[ModelNode], filter: &str, rows: &mut Vec<ListRow>) {
    for node in nodes {
        match node {
            ModelNode::Folder { children, .. } => push_matches(children, filter, rows),
            ModelNode::Model {
                label,
                key,
                rate_hz,
            } => {
                if matches_filter(key, filter) {
                    rows.push(ListRow::Model {
                        depth: 0,
                        label: label.clone(),
                        key: key.clone(),
                        folder: folder_of(key).map(str::to_owned),
                        rate_hz: *rate_hz,
                    });
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entries(keys: &[&str]) -> Vec<(String, u32)> {
        keys.iter().map(|k| ((*k).to_owned(), 48_000)).collect()
    }
    fn labels(rows: &[ListRow]) -> Vec<&str> {
        rows.iter()
            .map(|r| match r {
                ListRow::Folder { label, .. } | ListRow::Model { label, .. } => label.as_str(),
            })
            .collect()
    }

    #[test]
    fn model_tree_groups_keys_by_folder_and_sorts_ignoring_case() {
        let tree = model_tree(&entries(&[
            "Pack/Deep/leaf",
            "Pack/inner",
            "alpha",
            "Zeta/one",
            "beta",
            "Pack",
        ]));
        let top: Vec<&str> = tree.iter().map(ModelNode::label).collect();
        assert_eq!(top, ["alpha", "beta", "Pack", "Pack", "Zeta"]);
        let ModelNode::Folder { path, children, .. } = &tree[2] else {
            panic!("expected the Pack folder")
        };
        assert_eq!(path, "Pack");
        let ModelNode::Folder {
            path: deep,
            children: deep_children,
            ..
        } = &children[0]
        else {
            panic!("expected Deep")
        };
        assert_eq!(deep, "Pack/Deep");
        assert_eq!(
            deep_children[0],
            ModelNode::Model {
                label: "leaf".into(),
                key: "Pack/Deep/leaf".into(),
                rate_hz: 48_000
            }
        );
    }

    #[test]
    fn leaf_label_and_folder_of_split_a_key() {
        assert_eq!(leaf_label("Pack/Deep/leaf"), "leaf");
        assert_eq!(leaf_label("alpha"), "alpha");
        assert_eq!(folder_of("Pack/Deep/leaf"), Some("Pack/Deep"));
        assert_eq!(folder_of("alpha"), None);
    }

    #[test]
    fn filter_is_a_literal_case_insensitive_substring() {
        assert!(matches_filter(
            "S-[AMP] 020W BOLD CLEAN Clean-Blues #01 (Factory)",
            "[amp]"
        ));
        assert!(matches_filter(
            "S-[AMP] 020W BOLD CLEAN Clean-Blues #01 (Factory)",
            "#01 ("
        ));
        assert!(matches_filter("Boosted-6505+-A2-Chugs-FullRig", "6505+"));
        assert!(matches_filter("Pack/Deluxe Reverb", "pack/del"));
        assert!(matches_filter("合集/失真 高增益", "高增益"));
        assert!(!matches_filter("Deluxe Reverb", ".*"));
        assert!(!matches_filter("Deluxe Reverb", "bassman"));
    }

    #[test]
    fn collapsed_folders_hide_their_models() {
        let tree = model_tree(&entries(&["Pack/a", "Pack/b", "loose"]));
        let rows = visible_rows(&tree, "", &HashSet::new(), None);
        assert_eq!(labels(&rows), ["loose", "Pack"]);
        assert!(matches!(
            &rows[1],
            ListRow::Folder {
                open: false,
                depth: 0,
                ..
            }
        ));
    }

    #[test]
    fn opened_folders_show_their_models_one_level_in() {
        let tree = model_tree(&entries(&["Pack/a", "Pack/b", "loose"]));
        let open = HashSet::from(["Pack".to_owned()]);
        let rows = visible_rows(&tree, "", &open, None);
        assert_eq!(labels(&rows), ["loose", "Pack", "a", "b"]);
        assert!(matches!(
            &rows[2],
            ListRow::Model {
                depth: 1,
                folder: None,
                ..
            }
        ));
    }

    #[test]
    fn the_folders_holding_the_loaded_model_are_open_without_being_asked() {
        let tree = model_tree(&entries(&["Pack/Deep/leaf", "Pack/other", "Zeta/one"]));
        let rows = visible_rows(&tree, "", &HashSet::new(), Some("Pack/Deep/leaf"));
        assert_eq!(labels(&rows), ["Pack", "Deep", "leaf", "other", "Zeta"]);
    }

    #[test]
    fn filtering_flattens_the_tree_and_says_where_each_match_lives() {
        let tree = model_tree(&entries(&["Pack/Deluxe", "Deluxe Reverb", "Pack/Bassman"]));
        let rows = visible_rows(&tree, "  DELUXE ", &HashSet::new(), None);
        assert_eq!(
            rows,
            [
                ListRow::Model {
                    depth: 0,
                    label: "Deluxe Reverb".into(),
                    key: "Deluxe Reverb".into(),
                    folder: None,
                    rate_hz: 48_000
                },
                ListRow::Model {
                    depth: 0,
                    label: "Deluxe".into(),
                    key: "Pack/Deluxe".into(),
                    folder: Some("Pack".into()),
                    rate_hz: 48_000
                },
            ]
        );
    }

    #[test]
    fn malformed_keys_do_not_panic_and_keep_their_key() {
        let tree = model_tree(&entries(&["a//b", "c/", "/d"]));
        let rows = visible_rows(&tree, "b", &HashSet::new(), None);
        assert!(matches!(&rows[0], ListRow::Model { key, .. } if key == "a//b"));
    }
}
