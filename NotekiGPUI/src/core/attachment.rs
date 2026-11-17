//! Where a pasted or dropped image is saved: next to its Note, under a name
//! that takes nothing already there (SPEC › Embeds and Attachments).

use std::path::{Path, PathBuf};

/// The name Obsidian gives a pasted image: `Pasted image 20261117103045.png`.
pub fn pasted_image_name(now: chrono::NaiveDateTime, extension: &str) -> String {
    format!("Pasted image {}.{extension}", now.format("%Y%m%d%H%M%S"))
}

/// `folder/name`, or `folder/name 1.ext`, `folder/name 2.ext`... the first
/// that `exists` does not hold.
pub fn free_path(folder: &Path, name: &str, exists: impl Fn(&Path) -> bool) -> PathBuf {
    let first = folder.join(name);
    if !exists(&first) {
        return first;
    }
    let (stem, ext) = match name.rsplit_once('.') {
        Some((s, e)) if !s.is_empty() => (s, format!(".{e}")),
        _ => (name, String::new()),
    };
    (1..)
        .map(|n| folder.join(format!("{stem} {n}{ext}")))
        .find(|p| !exists(p))
        .expect("an unused name")
}

/// Whether a dropped file is an image Noteki shows.
pub fn is_image(path: &Path) -> bool {
    path.extension().and_then(|e| e.to_str()).is_some_and(|e| {
        matches!(
            e.to_lowercase().as_str(),
            "png" | "jpg" | "jpeg" | "gif" | "webp" | "svg" | "heic"
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_never_take_a_file_already_there() {
        let t = chrono::NaiveDate::from_ymd_opt(2026, 11, 17)
            .unwrap()
            .and_hms_opt(10, 30, 45)
            .unwrap();
        assert_eq!(
            pasted_image_name(t, "png"),
            "Pasted image 20261117103045.png"
        );
        let taken = |p: &Path| p.ends_with("a.png") || p.ends_with("a 1.png");
        assert_eq!(
            free_path(Path::new("/v"), "a.png", taken),
            Path::new("/v/a 2.png")
        );
        assert_eq!(
            free_path(Path::new("/v"), "b.png", taken),
            Path::new("/v/b.png")
        );
        assert!(is_image(Path::new("x/Schermata.PNG")));
        assert!(!is_image(Path::new("x/doc.pdf")));
    }
}
