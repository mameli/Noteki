//! The icons the window draws. GPUI Kit's `Assets` holds only its components'
//! icons; an icon missing from it is drawn as nothing (the Tab's ×, the
//! toolbar's buttons), so the others Noteki uses are added here.

use std::borrow::Cow;

use gpui_kit::assets::{icon_assets, Assets};
use gpui_kit::{AssetSource, Result, SharedString};

icon_assets!(
    ExtraIcons,
    [
        CalendarDays,
        Command,
        Download,
        FilePlus,
        FolderPlus,
        Image,
        Link,
        List,
        Pencil,
        SquarePlus,
        Tag,
        Trash,
        X,
    ]
);

pub struct NotekiAssets;

impl AssetSource for NotekiAssets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        if let Some(bytes) = ExtraIcons.load(path)? {
            return Ok(Some(bytes));
        }
        Assets.load(path)
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        let mut paths = Assets.list(path)?;
        paths.extend(ExtraIcons.list(path)?);
        paths.sort();
        paths.dedup();
        Ok(paths)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui_kit::assets::IconName;

    /// Every icon named in `src` (as `AssetIcon::…`) can be drawn.
    #[test]
    fn every_icon_used_is_bundled() {
        fn sources(dir: &std::path::Path, out: &mut String) {
            for entry in std::fs::read_dir(dir).unwrap().flatten() {
                let path = entry.path();
                if path.is_dir() {
                    sources(&path, out);
                } else if path.extension().is_some_and(|e| e == "rs") {
                    out.push_str(&std::fs::read_to_string(path).unwrap());
                }
            }
        }
        let mut code = String::new();
        sources(&std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src"), &mut code);
        let mut used: Vec<&str> = code
            .split("AssetIcon::")
            .skip(1)
            .map(|rest| rest.split(|c: char| !c.is_alphanumeric()).next().unwrap())
            .filter(|name| !name.is_empty())
            .collect();
        used.sort();
        used.dedup();
        assert!(used.len() > 10, "found only {used:?}");
        let missing: Vec<&str> = used
            .into_iter()
            .filter(|name| {
                let icon = IconName::ALL
                    .iter()
                    .find(|i| format!("{i:?}") == *name)
                    .unwrap_or_else(|| panic!("no icon {name}"));
                NotekiAssets.load(icon.path().as_ref()).unwrap().is_none()
            })
            .collect();
        assert!(missing.is_empty(), "not bundled: {missing:?}");
    }
}
