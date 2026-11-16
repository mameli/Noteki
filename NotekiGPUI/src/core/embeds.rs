use crate::core::vault::{resolve_vault_path, AttachmentKind};
use crate::core::wikilink::{ResolvedItem, VaultIndex, Wikilink};
use serde::Serialize;
use std::path::Path;
use std::time::UNIX_EPOCH;

#[derive(Debug, Clone, Serialize)]
pub struct EmbeddedFile {
    pub path: String,
    pub kind: AttachmentKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub width: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub height: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<u64>,
}

pub fn resolve_embed(
    vault_root: &Path,
    inner: &str,
    source_path: Option<&str>,
    index: &VaultIndex,
) -> Option<EmbeddedFile> {
    if inner.contains("://") {
        return None;
    }
    let link = Wikilink::parse(inner);
    let ResolvedItem::Attachment {
        path,
        attachment_kind,
        ..
    } = index.resolve(&link.target, source_path)?
    else {
        return None;
    };

    match attachment_kind {
        AttachmentKind::Pdf => Some(EmbeddedFile {
            path,
            kind: AttachmentKind::Pdf,
            width: None,
            height: None,
            version: None,
        }),
        AttachmentKind::Image => {
            let abs = resolve_vault_path(vault_root, &path).ok()?;
            let (width, height) = imagesize::size(&abs)
                .ok()
                .map(|dim| (Some(dim.width), Some(dim.height)))
                .unwrap_or((None, None));
            let version = abs
                .metadata()
                .ok()
                .and_then(|m| m.modified().ok())
                .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                .map(|d| d.as_secs());
            Some(EmbeddedFile {
                path,
                kind: AttachmentKind::Image,
                width,
                height,
                version,
            })
        }
    }
}
