//! The whole backend as the window drives it, on a temporary copy of
//! `docs/demo-vault`: the same calls the Tauri commands make, and the events
//! the window would receive.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use noteki_tauri_lib::session::{Event, FollowLinkResult, Session};

struct Fixture {
    dir: PathBuf,
    vault: PathBuf,
    s: Session,
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn copy_dir(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for e in std::fs::read_dir(from).unwrap().flatten() {
        let t = to.join(e.file_name());
        if e.file_type().unwrap().is_dir() {
            copy_dir(&e.path(), &t);
        } else {
            std::fs::copy(e.path(), t).unwrap();
        }
    }
}

fn fixture() -> Fixture {
    static N: AtomicUsize = AtomicUsize::new(0);
    let dir = std::env::temp_dir().join(format!(
        "noteki-tauri-{}-{}",
        std::process::id(),
        N.fetch_add(1, Ordering::SeqCst)
    ));
    let _ = std::fs::remove_dir_all(&dir);
    copy_dir(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/demo-vault"),
        &dir.join("vault"),
    );
    let dir = std::fs::canonicalize(dir).unwrap();
    let vault = dir.join("vault");
    let mut s = Session::new(Some(dir.join("settings.json")));
    s.open_vault(&vault).unwrap();
    Fixture { dir, vault, s }
}

impl Fixture {
    fn read(&self, rel: &str) -> String {
        std::fs::read_to_string(self.vault.join(rel)).unwrap()
    }
    /// Another app writes (or deletes) a file; the watcher reports it.
    fn external(&mut self, rel: &str, text: Option<&str>) -> Vec<Event> {
        let abs = self.vault.join(rel);
        match text {
            Some(t) => std::fs::write(&abs, t).unwrap(),
            None => std::fs::remove_file(&abs).unwrap(),
        }
        self.s.fs_changed(&[abs])
    }
    fn ideas(&mut self) -> String {
        self.s.open_note("Ideas.md").unwrap().text
    }
}

fn reloaded(events: &[Event]) -> Option<(u64, String, bool)> {
    events.iter().find_map(|e| match e {
        Event::NoteReloaded {
            revision,
            text,
            merged,
            ..
        } => Some((*revision, text.clone(), *merged)),
        _ => None,
    })
}

// ---- Saving ----

#[test]
fn typing_is_saved_and_a_clean_save_writes_nothing() {
    let mut f = fixture();
    let text = f.ideas();
    f.s.update_buffer("Ideas.md", 1, &format!("X{text}"));
    assert!(f.s.save_note("Ideas.md").unwrap());
    assert_eq!(f.read("Ideas.md"), format!("X{text}"));

    let mtime = std::fs::metadata(f.vault.join("Ideas.md"))
        .unwrap()
        .modified()
        .unwrap();
    std::thread::sleep(std::time::Duration::from_millis(20));
    assert!(f.s.save_note("Ideas.md").unwrap());
    assert_eq!(
        std::fs::metadata(f.vault.join("Ideas.md"))
            .unwrap()
            .modified()
            .unwrap(),
        mtime
    );
}

#[test]
fn our_own_save_coming_back_from_the_watcher_is_ignored() {
    let mut f = fixture();
    let text = f.ideas();
    f.s.update_buffer("Ideas.md", 1, &format!("X{text}"));
    f.s.save_note("Ideas.md").unwrap();
    f.s.update_buffer("Ideas.md", 2, &format!("XY{text}"));
    let events = f.s.fs_changed(&[f.vault.join("Ideas.md")]);
    assert!(reloaded(&events).is_none(), "{events:?}");
    assert_eq!(f.s.doc("Ideas.md").unwrap().text(), format!("XY{text}"));
}

#[test]
fn temporary_and_hidden_files_are_ignored() {
    let mut f = fixture();
    f.ideas();
    std::fs::write(f.vault.join(".Ideas.md.123.noteki.tmp"), "x").unwrap();
    assert!(f
        .s
        .fs_changed(&[f.vault.join(".Ideas.md.123.noteki.tmp")])
        .is_empty());
}

#[test]
fn flush_all_saves_every_open_note() {
    let mut f = fixture();
    let ideas = f.ideas();
    let reading = f.s.open_note("Reading list.md").unwrap().text;
    f.s.update_buffer("Ideas.md", 1, "a");
    f.s.update_buffer("Reading list.md", 2, "b");
    assert!(f.s.flush_all().is_empty());
    assert_eq!(
        (f.read("Ideas.md"), f.read("Reading list.md")),
        ("a".into(), "b".into())
    );
    assert_ne!(ideas, reading);
}

// ---- External changes (SPEC › External changes) ----

#[test]
fn a_clean_note_is_reloaded() {
    let mut f = fixture();
    let text = f.ideas();
    f.s.update_buffer("Ideas.md", 3, &text);
    let new = format!("{text}- more\n");
    let ev = f.external("Ideas.md", Some(&new));
    assert_eq!(reloaded(&ev), Some((3, new, false)));
}

#[test]
fn unsaved_edits_are_merged_and_saved() {
    let mut f = fixture();
    let text = f.ideas();
    f.s.update_buffer("Ideas.md", 1, &format!("Mine {text}"));
    let theirs = text.replace("Graph view?", "Graph view (theirs)?");
    let ev = f.external("Ideas.md", Some(&theirs));
    let merged = format!("Mine {theirs}");
    assert_eq!(reloaded(&ev), Some((1, merged.clone(), true)));
    assert_eq!(f.read("Ideas.md"), merged);
}

#[test]
fn a_conflict_stops_saving_until_resolved() {
    let mut f = fixture();
    let text = f.ideas();
    f.s.update_buffer("Ideas.md", 1, &format!("Mine {text}"));
    let ev = f.external("Ideas.md", Some("Theirs # Ideas\n"));
    assert!(ev.iter().any(
        |e| matches!(e, Event::NoteConflict { disk_text, .. } if disk_text == "Theirs # Ideas\n")
    ));
    assert!(!f.s.save_note("Ideas.md").unwrap());
    assert_eq!(f.read("Ideas.md"), "Theirs # Ideas\n");
    // Reopening the tab keeps the conflict.
    assert!(f.s.open_note("Ideas.md").unwrap().conflict);

    let kept = f.s.resolve_conflict("Ideas.md", "keep_mine").unwrap();
    assert!(kept.starts_with("Mine # Ideas"));
    assert_eq!(f.read("Ideas.md"), kept);
}

#[test]
fn take_theirs_shows_the_disk_text() {
    let mut f = fixture();
    let text = f.ideas();
    f.s.update_buffer("Ideas.md", 1, &format!("Mine {text}"));
    f.external("Ideas.md", Some("Theirs\n"));
    assert_eq!(
        f.s.resolve_conflict("Ideas.md", "take_theirs").unwrap(),
        "Theirs\n"
    );
    assert_eq!(f.read("Ideas.md"), "Theirs\n");
    assert!(f.s.resolve_conflict("Ideas.md", "nonsense").is_err());
}

#[test]
fn a_deleted_note_is_not_recreated_until_saved_again() {
    let mut f = fixture();
    let text = f.ideas();
    f.s.update_buffer("Ideas.md", 1, &format!("Keep {text}"));
    let ev = f.external("Ideas.md", None);
    assert!(
        ev.contains(&Event::NoteDeleted {
            path: "Ideas.md".into()
        }),
        "{ev:?}"
    );
    assert!(!f.s.save_note("Ideas.md").unwrap());
    assert!(f.s.flush_all().is_empty());
    assert!(!f.vault.join("Ideas.md").exists());
    f.s.save_again("Ideas.md").unwrap();
    assert_eq!(f.read("Ideas.md"), format!("Keep {text}"));
}

#[test]
fn a_rename_by_another_app_is_followed() {
    let mut f = fixture();
    let text = f.ideas();
    f.s.update_buffer("Ideas.md", 5, &text); // the editor reported under the old path
    let (old, new) = (
        f.vault.join("Ideas.md"),
        f.vault.join("Projects/Ideas moved.md"),
    );
    std::fs::rename(&old, &new).unwrap();
    let ev = f.s.fs_changed(&[old, new]);
    assert!(
        ev.contains(&Event::NoteRenamed {
            from: "Ideas.md".into(),
            to: "Projects/Ideas moved.md".into()
        }),
        "{ev:?}"
    );
    assert!(ev
        .iter()
        .any(|e| matches!(e, Event::VaultTreeChanged { .. })));
    assert_eq!(f.s.open_note("Projects/Ideas moved.md").unwrap().text, text);
    assert!(f.s.doc("Ideas.md").is_none());
    // The editor has reported nothing under the new path: reloads must carry revision 0,
    // or the editor would refuse every one of them.
    let ev = f.external("Projects/Ideas moved.md", Some("changed later\n"));
    assert_eq!(reloaded(&ev), Some((0, "changed later\n".into(), false)));
}

/// The race this backend had: a reload arrives while the user is typing, the
/// editor (rightly) refuses it, and its next save used to overwrite the other
/// app's change.
#[test]
fn a_reload_refused_by_the_editor_is_merged_again_not_lost() {
    let mut f = fixture();
    let text = f.ideas();
    f.s.update_buffer("Ideas.md", 1, &text);
    let theirs = text.replace("Graph view?", "Graph view (Takku)?");
    let ev = f.external("Ideas.md", Some(&theirs));
    assert_eq!(reloaded(&ev).map(|r| r.2), Some(false));

    // Meanwhile the user typed on the old text: the editor reports it and refuses the reload.
    let typed = format!("Typed {text}");
    f.s.update_buffer("Ideas.md", 2, &typed);
    let ev = f.s.reload_rejected("Ideas.md");
    let expected = format!("Typed {theirs}");
    assert_eq!(reloaded(&ev), Some((2, expected.clone(), true)));
    assert_eq!(f.read("Ideas.md"), expected, "both changes on disk");
    assert!(f.s.save_note("Ideas.md").unwrap());
    assert_eq!(f.read("Ideas.md"), expected);
}

#[test]
fn a_refused_reload_that_cannot_merge_becomes_a_conflict() {
    let mut f = fixture();
    let text = f.ideas();
    f.s.update_buffer("Ideas.md", 1, &text);
    f.external("Ideas.md", Some("Takku rewrote it\n"));
    f.s.update_buffer("Ideas.md", 2, "User rewrote it\n");
    let ev = f.s.reload_rejected("Ideas.md");
    assert!(
        ev.iter().any(|e| matches!(e, Event::NoteConflict { .. })),
        "{ev:?}"
    );
    assert!(!f.s.save_note("Ideas.md").unwrap());
    assert_eq!(f.read("Ideas.md"), "Takku rewrote it\n");
}

#[test]
fn a_refused_reload_with_nothing_new_is_not_sent_again() {
    // No loop of reloads: the user's text already holds the disk change (typed
    // on top of it), so merging again gives the user's text back.
    let mut f = fixture();
    let text = f.ideas();
    f.s.update_buffer("Ideas.md", 1, &text);
    let theirs = format!("{text}\ntheirs\n");
    f.external("Ideas.md", Some(&theirs));
    f.s.update_buffer("Ideas.md", 2, &format!("Typed {theirs}"));
    assert!(f.s.reload_rejected("Ideas.md").is_empty());
}

#[test]
fn a_reload_the_editor_applied_needs_nothing_more() {
    let mut f = fixture();
    let text = f.ideas();
    f.external("Ideas.md", Some(&format!("{text}x\n")));
    assert!(f.s.reload_rejected("Nope.md").is_empty());
    f.s.update_buffer("Ideas.md", 1, &format!("{text}x\n"));
    assert!(f.s.reload_rejected("Ideas.md").is_empty());
}

// ---- Links, search, tags ----

#[test]
fn following_links() {
    let mut f = fixture();
    let r = f.s.follow_link("Welcome.md", "Launch plan").unwrap();
    assert_eq!(
        r,
        FollowLinkResult::Note {
            path: "Projects/Launch plan.md".into(),
            created: false
        }
    );
    assert!(matches!(
        f.s.follow_link("Welcome.md", "noteki.png|120").unwrap(),
        FollowLinkResult::Attachment { .. }
    ));

    let r =
        f.s.follow_link("Physics/Euler's identity.md", "Brand new")
            .unwrap();
    assert_eq!(
        r,
        FollowLinkResult::Note {
            path: "Physics/Brand new.md".into(),
            created: true
        }
    );
    assert_eq!(f.read("Physics/Brand new.md"), "");
    assert!(f.s.follow_link("Welcome.md", "bad:name").is_err());
    assert!(f.s.search("").is_empty());
}

#[test]
fn search_backlinks_and_tags_follow_saves() {
    let mut f = fixture();
    let names = |s: &Session, q: &str| s.search(q).into_iter().map(|r| r.name).collect::<Vec<_>>();
    assert_eq!(
        names(&f.s, "sparkle"),
        vec!["2026-11-10 Product sync", "Launch plan"]
    );
    assert_eq!(f.s.backlinks("Projects/Launch plan.md").len(), 3);

    let text = f.ideas();
    f.s.update_buffer(
        "Ideas.md",
        1,
        &format!("{text}\nSee [[Launch plan]] #brand-new-tag sparkle\n"),
    );
    f.s.save_note("Ideas.md").unwrap();
    assert_eq!(
        names(&f.s, "sparkle"),
        vec!["Ideas", "2026-11-10 Product sync", "Launch plan"],
        "sorted by path"
    );
    assert_eq!(f.s.backlinks("Projects/Launch plan.md").len(), 4);
    assert!(f.s.tags().iter().any(|t| t.full_lower == "brand-new-tag"));

    // A note added by another app is searchable after the watcher reports it.
    f.external("Inbox.md", Some("quokka"));
    assert_eq!(names(&f.s, "quokka"), vec!["Inbox"]);
}

#[test]
fn new_note_is_untitled_in_the_folder() {
    let mut f = fixture();
    let (p, snap) = f.s.create_new_note("Projects").unwrap();
    assert_eq!(p, "Projects/Untitled.md");
    assert_eq!(snap.note_count, 9);
    assert_eq!(
        f.s.create_new_note("Projects").unwrap().0,
        "Projects/Untitled 1.md"
    );
}

// ---- Settings and the window contract ----

#[test]
fn settings_survive_and_a_corrupt_file_is_kept_aside() {
    let mut f = fixture();
    f.ideas();
    let path = f.dir.join("settings.json");
    let again = Session::new(Some(path.clone()));
    assert_eq!(again.settings.recent_notes, vec!["Ideas.md"]);
    assert_eq!(
        again.settings.vault_path.as_deref(),
        Some(f.vault.to_str().unwrap())
    );

    std::fs::write(&path, "{ broken").unwrap();
    assert_eq!(Session::new(Some(path.clone())).settings.theme, "ayu");
    assert!(path.with_extension("json.bak").exists());
    let mut s = f.s.settings.clone();
    s.theme = "everforest".into();
    f.s.save_settings(s).unwrap();
    assert_eq!(Session::new(Some(path)).settings.theme, "everforest");
}

#[test]
fn events_have_the_shape_app_ts_reads() {
    let json = |e: Event| serde_json::to_value(e).unwrap();
    assert_eq!(
        json(Event::NoteReloaded {
            path: "a.md".into(),
            revision: 2,
            text: "t".into(),
            merged: true
        }),
        serde_json::json!({"path": "a.md", "revision": 2, "text": "t", "merged": true})
    );
    assert_eq!(
        json(Event::NoteConflict {
            path: "a.md".into(),
            disk_text: "d".into()
        }),
        serde_json::json!({"path": "a.md", "diskText": "d"})
    );
    assert_eq!(
        json(Event::NoteRenamed {
            from: "a".into(),
            to: "b".into()
        }),
        serde_json::json!({"from": "a", "to": "b"})
    );
    assert_eq!(
        json(Event::VaultTreeChanged {
            tree: vec![],
            note_count: 0
        }),
        serde_json::json!({"tree": [], "noteCount": 0})
    );
    assert_eq!(
        Event::NoteDeleted { path: "a".into() }.name(),
        "note-deleted"
    );
}
