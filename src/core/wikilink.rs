use crate::core::vault::{AttachmentKind, VaultItem};
use serde::Serialize;
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Wikilink {
    pub target: String,
    pub heading: Option<String>,
    pub alias: Option<String>,
}

impl Wikilink {
    pub fn parse(inner: &str) -> Self {
        let unescaped = inner.replace("\\|", "|");
        let (dest_and_heading, alias) = match unescaped.split_once('|') {
            Some((left, right)) => (left, Some(right.trim().to_string())),
            None => (unescaped.as_str(), None),
        };
        let (dest, heading) = match dest_and_heading.split_once('#') {
            Some((left, right)) => (left, Some(right.trim().to_string())),
            None => (dest_and_heading, None),
        };
        Self {
            target: dest.trim().to_string(),
            heading: heading.filter(|h| !h.is_empty()),
            alias: alias.filter(|a| !a.is_empty()),
        }
    }

    pub fn display_text(&self) -> String {
        if let Some(alias) = &self.alias {
            return alias.clone();
        }
        if self.target.is_empty() {
            if let Some(h) = &self.heading {
                return format!("#{h}");
            }
        }
        let name = self.target.rsplit('/').next().unwrap_or(&self.target);
        name.strip_suffix(".md")
            .or_else(|| name.strip_suffix(".MD"))
            .unwrap_or(name)
            .to_string()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum ResolvedItem {
    Note {
        name: String,
        path: String,
    },
    Attachment {
        name: String,
        path: String,
        #[serde(rename = "attachmentKind")]
        attachment_kind: AttachmentKind,
    },
}

#[derive(Debug, Clone)]
struct Candidate {
    item: ResolvedItem,
    lower_path: String,
}

#[derive(Debug, Clone, Default)]
pub struct VaultIndex {
    by_name: HashMap<String, Vec<Candidate>>,
    notes: Vec<(String, String)>, // (name, path)
}

#[derive(Debug, Clone, Serialize)]
pub struct LinkCompletion {
    pub name: String,
    pub folder: String,
    pub link: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct LinkCompletions {
    pub recent: Vec<LinkCompletion>,
    pub others: Vec<LinkCompletion>,
}

impl VaultIndex {
    pub fn from_tree(tree: &[VaultItem]) -> Self {
        let mut by_name: HashMap<String, Vec<Candidate>> = HashMap::new();
        let mut notes = Vec::new();
        collect(tree, &mut by_name, &mut notes);
        Self { by_name, notes }
    }

    pub fn all_notes(&self) -> &[(String, String)] {
        &self.notes
    }

    pub fn resolve(&self, target: &str, source_path: Option<&str>) -> Option<ResolvedItem> {
        if target.is_empty() {
            return source_path.map(|p| ResolvedItem::Note {
                name: note_name(p),
                path: p.to_string(),
            });
        }

        let mut link = target.to_lowercase();
        let mut name = after_last_slash(&link).to_string();
        let mut candidates = if name.contains('.') {
            self.by_name.get(&name)
        } else {
            None
        };
        if candidates.is_none() {
            link = format!("{}.md", target).to_lowercase();
            name = after_last_slash(&link).to_string();
            candidates = self.by_name.get(&name);
        }
        let candidates = candidates?;
        if candidates.is_empty() {
            return None;
        }
        if name == link && candidates.len() == 1 {
            return Some(candidates[0].item.clone());
        }

        let mut folder = folder_of(source_path.unwrap_or("")).to_lowercase();
        if link.starts_with("./") || link.starts_with("../") {
            if let Some(rest) = link.strip_prefix("./../") {
                link = format!("../{rest}");
            }
            if let Some(rest) = link.strip_prefix("./") {
                link = if folder.is_empty() {
                    rest.to_string()
                } else {
                    format!("{folder}/{rest}")
                };
            } else {
                while let Some(rest) = link.strip_prefix("../") {
                    link = rest.to_string();
                    folder = folder_of(&folder).to_string();
                }
                if !folder.is_empty() {
                    link = format!("{folder}/{link}");
                }
            }
            if let Some(exact) = candidates.iter().find(|c| c.lower_path == link) {
                return Some(exact.item.clone());
            }
        }

        if let Some(stripped) = link.strip_prefix('/') {
            link = stripped.to_string();
        }
        if let Some(exact) = candidates.iter().find(|c| c.lower_path == link) {
            return Some(exact.item.clone());
        }
        if target.starts_with('/') {
            return None;
        }

        let matching: Vec<&Candidate> = candidates
            .iter()
            .filter(|c| c.lower_path.ends_with(&link))
            .collect();
        let mut near: Vec<&Candidate> = matching
            .iter()
            .copied()
            .filter(|c| c.lower_path.starts_with(&folder))
            .collect();
        let mut far: Vec<&Candidate> = matching
            .iter()
            .copied()
            .filter(|c| !c.lower_path.starts_with(&folder))
            .collect();

        near.sort_by_key(|c| c.lower_path.len());
        far.sort_by_key(|c| c.lower_path.len());
        near.into_iter().chain(far).next().map(|c| c.item.clone())
    }

    pub fn link_completions(&self, recent_paths: &[String]) -> LinkCompletions {
        let listed: HashSet<&str> = self.notes.iter().map(|(_, p)| p.as_str()).collect();
        let mut offered: HashSet<String> = HashSet::new();
        let mut recent = Vec::new();

        for path in recent_paths {
            if listed.contains(path.as_str()) && offered.insert(path.clone()) {
                let name = note_name(path);
                recent.push(self.completion(&name, path));
            }
        }
        let others = self
            .notes
            .iter()
            .filter(|(_, p)| !offered.contains(p))
            .map(|(n, p)| self.completion(n, p))
            .collect();

        LinkCompletions { recent, others }
    }

    fn completion(&self, name: &str, path: &str) -> LinkCompletion {
        let parts: Vec<&str> = path.split('/').collect();
        let file_lower = parts.last().copied().unwrap_or("").to_lowercase();
        let candidates = self
            .by_name
            .get(&file_lower)
            .map(|v| v.as_slice())
            .unwrap_or(&[]);

        let mut chosen = path.to_string();
        for count in 1..=parts.len() {
            let suffix = parts[parts.len() - count..].join("/");
            let lowered = suffix.to_lowercase();
            let link = suffix
                .strip_suffix(".md")
                .or_else(|| suffix.strip_suffix(".MD"))
                .unwrap_or(&suffix)
                .to_string();
            let unique = count == parts.len()
                || candidates
                    .iter()
                    .filter(|c| c.lower_path.ends_with(&lowered))
                    .count()
                    == 1;
            if unique {
                if let Some(ResolvedItem::Note {
                    path: resolved_path,
                    ..
                }) = self.resolve(&link, None)
                {
                    if resolved_path == path {
                        chosen = link;
                        break;
                    }
                }
            }
        }

        LinkCompletion {
            name: name.to_string(),
            folder: folder_of(path).to_string(),
            link: chosen,
        }
    }
}

fn collect(
    items: &[VaultItem],
    by_name: &mut HashMap<String, Vec<Candidate>>,
    notes: &mut Vec<(String, String)>,
) {
    for item in items {
        match item {
            VaultItem::Folder { children, .. } => collect(children, by_name, notes),
            VaultItem::Note { name, path } => {
                notes.push((name.clone(), path.clone()));
                let lower = path.to_lowercase();
                by_name
                    .entry(after_last_slash(&lower).to_string())
                    .or_default()
                    .push(Candidate {
                        item: ResolvedItem::Note {
                            name: name.clone(),
                            path: path.clone(),
                        },
                        lower_path: lower,
                    });
            }
            VaultItem::Attachment {
                name,
                path,
                attachment_kind,
            } => {
                let lower = path.to_lowercase();
                by_name
                    .entry(after_last_slash(&lower).to_string())
                    .or_default()
                    .push(Candidate {
                        item: ResolvedItem::Attachment {
                            name: name.clone(),
                            path: path.clone(),
                            attachment_kind: *attachment_kind,
                        },
                        lower_path: lower,
                    });
            }
        }
    }
}

fn after_last_slash(path: &str) -> &str {
    path.rsplit_once('/').map(|(_, tail)| tail).unwrap_or(path)
}

pub fn folder_of(path: &str) -> &str {
    path.rsplit_once('/').map(|(dir, _)| dir).unwrap_or("")
}

pub fn note_name(path: &str) -> String {
    let last = after_last_slash(path);
    last.strip_suffix(".md").unwrap_or(last).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_by_folder_proximity_and_path_suffix() {
        let tree = vec![
            VaultItem::Folder {
                name: "Clients".into(),
                path: "Clients".into(),
                children: vec![VaultItem::Folder {
                    name: "Axo".into(),
                    path: "Clients/Axo".into(),
                    children: vec![
                        VaultItem::Note {
                            name: "Retro".into(),
                            path: "Clients/Axo/Retro.md".into(),
                        },
                        VaultItem::Note {
                            name: "Overview".into(),
                            path: "Clients/Axo/Overview.md".into(),
                        },
                    ],
                }],
            },
            VaultItem::Folder {
                name: "Archive".into(),
                path: "Archive".into(),
                children: vec![VaultItem::Note {
                    name: "Retro".into(),
                    path: "Archive/Retro.md".into(),
                }],
            },
        ];
        let index = VaultIndex::from_tree(&tree);
        assert_eq!(
            index.resolve("Retro", Some("Clients/Axo/Overview.md")),
            Some(ResolvedItem::Note {
                name: "Retro".into(),
                path: "Clients/Axo/Retro.md".into(),
            })
        );
        assert_eq!(
            index.resolve("Axo/Retro", Some("Other.md")),
            Some(ResolvedItem::Note {
                name: "Retro".into(),
                path: "Clients/Axo/Retro.md".into(),
            })
        );
        assert_eq!(
            index.resolve("Retro", Some("Other.md")),
            Some(ResolvedItem::Note {
                name: "Retro".into(),
                path: "Archive/Retro.md".into(),
            })
        );
    }
}
