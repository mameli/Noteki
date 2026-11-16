import "./bootstrap";
import "../../Editor/src/editor.css";
import "./shell.css";
import "../../Editor/src/main";

import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import { openUrl } from "@tauri-apps/plugin-opener";
import type { EditorCommand, EditorMessage, LinkCompletions } from "../../Editor/src/bridge";
import type { EmbeddedFile } from "../../Editor/src/embeds";
import type { FrontmatterReading } from "../../Editor/src/frontmatter";
import { setEditorMessageHandler } from "./bootstrap";

type VaultItem =
  | { kind: "folder"; name: string; path: string; children: VaultItem[] }
  | { kind: "note"; name: string; path: string }
  | { kind: "attachment"; name: string; path: string; attachmentKind: "image" | "pdf" };

interface VaultSnapshot {
  root: string;
  tree: VaultItem[];
  noteCount: number;
}

interface AppSettings {
  vaultPath: string | null;
  theme: string;
  appearance: string;
  sourceMode: boolean;
  recentNotes: string[];
}

interface OpenNoteResponse {
  path: string;
  text: string;
  conflict: boolean;
  deleted: boolean;
}

interface CheckLinksResult {
  unresolved: string[];
  embeds: Record<string, EmbeddedFile>;
}

type FollowLinkResult =
  | { kind: "note"; path: string; created: boolean }
  | { kind: "attachment"; path: string; attachmentKind: "image" | "pdf" };

const sidebarEl = document.getElementById("sidebar")!;
const vaultNameEl = document.getElementById("vault-name")!;
const fileTreeEl = document.getElementById("file-tree")!;
const footerEl = document.getElementById("sidebar-footer")!;
const activeNotePathEl = document.getElementById("active-note-path")!;
const saveStatusEl = document.getElementById("save-status")!;
const bannerEl = document.getElementById("banner")!;
const bannerTextEl = document.getElementById("banner-text")!;
const bannerActionsEl = document.getElementById("banner-actions")!;
const emptyStateEl = document.getElementById("empty-state")!;
const attachmentViewEl = document.getElementById("attachment-view")!;
const editorEl = document.getElementById("editor")!;

const btnOpenVault = document.getElementById("btn-open-vault") as HTMLButtonElement;
const btnEmptyOpen = document.getElementById("btn-empty-open") as HTMLButtonElement;
const btnNewNote = document.getElementById("btn-new-note") as HTMLButtonElement;
const btnToggleSidebar = document.getElementById("btn-toggle-sidebar") as HTMLButtonElement;
const btnSourceMode = document.getElementById("btn-source-mode") as HTMLButtonElement;
const selectTheme = document.getElementById("select-theme") as HTMLSelectElement;
const selectAppearance = document.getElementById("select-appearance") as HTMLSelectElement;

let settings: AppSettings = {
  vaultPath: null,
  theme: "ayu",
  appearance: "system",
  sourceMode: false,
  recentNotes: [],
};

let vaultRoot: string | null = null;
let tree: VaultItem[] = [];
const expandedFolders = new Set<string>();

let activePath: string | null = null;
let activeKind: "note" | "attachment" | null = null;
let activeText = "";
let conflictNotes = new Set<string>();
let deletedNotes = new Set<string>();
let saveTimer: ReturnType<typeof setTimeout> | undefined;
let editorReady = false;

function sendToEditor(cmd: EditorCommand) {
  return window.noteki.receive(cmd);
}

async function applyAppearanceAndTheme() {
  const win = getCurrentWindow();
  const themeChoice = settings.appearance === "light" ? "light" : settings.appearance === "dark" ? "dark" : null;
  try {
    await win.setTheme(themeChoice);
  } catch {
    // Ignore on platforms/environments where setTheme is unsupported
  }

  sendToEditor({
    type: "settings",
    settings: {
      theme: settings.theme,
      size: 15,
      lineLength: 100,
      limitsLineLength: true,
      focusMode: false,
      typewriterMode: false,
    },
  });
  sendToEditor({ type: "sourceMode", on: settings.sourceMode });
  btnSourceMode.textContent = settings.sourceMode ? "Source" : "Live";
  btnSourceMode.classList.toggle("active", settings.sourceMode);
  selectTheme.value = settings.theme;
  selectAppearance.value = settings.appearance;
}

function vaultDisplayName(root: string): string {
  const parts = root.replace(/[/\\]+$/, "").split(/[/\\]/);
  return parts[parts.length - 1] || root;
}

function renderTree() {
  fileTreeEl.innerHTML = "";
  const fragment = document.createDocumentFragment();

  function addItems(items: VaultItem[], depth: number) {
    for (const item of items) {
      const row = document.createElement("div");
      row.className = "tree-row";
      row.style.paddingLeft = `${8 + depth * 14}px`;
      if (item.path === activePath) row.classList.add("active");

      if (item.kind === "folder") {
        const open = expandedFolders.has(item.path);
        const caret = document.createElement("span");
        caret.className = "tree-caret";
        caret.textContent = open ? "▾" : "▸";
        const label = document.createElement("span");
        label.textContent = item.name;
        row.append(caret, label);
        row.addEventListener("click", () => {
          if (expandedFolders.has(item.path)) expandedFolders.delete(item.path);
          else expandedFolders.add(item.path);
          renderTree();
        });
        fragment.append(row);
        if (open) addItems(item.children, depth + 1);
      } else if (item.kind === "note") {
        const caret = document.createElement("span");
        caret.className = "tree-caret";
        caret.textContent = "";
        const label = document.createElement("span");
        label.textContent = item.name;
        row.append(caret, label);
        row.addEventListener("click", () => void openNote(item.path));
        fragment.append(row);
      } else {
        const caret = document.createElement("span");
        caret.className = "tree-caret";
        caret.textContent = "";
        const label = document.createElement("span");
        label.textContent = item.name;
        const badge = document.createElement("span");
        badge.className = "tree-badge";
        badge.textContent = item.attachmentKind.toUpperCase();
        row.append(caret, label, badge);
        row.addEventListener("click", () => void openAttachment(item.path, item.attachmentKind));
        fragment.append(row);
      }
    }
  }

  addItems(tree, 0);
  fileTreeEl.append(fragment);
}

function expandParentsOf(relPath: string) {
  const parts = relPath.split("/");
  let acc = "";
  for (let i = 0; i < parts.length - 1; i++) {
    acc = acc ? `${acc}/${parts[i]}` : parts[i];
    expandedFolders.add(acc);
  }
}

function updateBanner() {
  if (!activePath || activeKind !== "note") {
    bannerEl.classList.add("hidden");
    return;
  }

  if (conflictNotes.has(activePath)) {
    const notePath = activePath;
    bannerEl.className = "banner conflict";
    bannerTextEl.textContent = "External change conflicts with your unsaved edits.";
    bannerActionsEl.innerHTML = "";

    const keepMine = document.createElement("button");
    keepMine.className = "icon-btn";
    keepMine.textContent = "Keep mine";
    keepMine.addEventListener("click", async () => {
      await flushEditor();
      const resolved = await invoke<string>("resolve_conflict", {
        path: notePath,
        choice: "keep_mine",
        currentText: activeText,
      });
      conflictNotes.delete(notePath);
      activeText = resolved;
      saveStatusEl.textContent = "Saved";
      updateBanner();
    });

    const takeTheirs = document.createElement("button");
    takeTheirs.className = "icon-btn";
    takeTheirs.textContent = "Take theirs";
    takeTheirs.addEventListener("click", async () => {
      const resolved = await invoke<string>("resolve_conflict", {
        path: notePath,
        choice: "take_theirs",
        currentText: activeText,
      });
      conflictNotes.delete(notePath);
      activeText = resolved;
      sendToEditor({ type: "load", noteID: notePath, text: resolved });
      saveStatusEl.textContent = "Saved";
      updateBanner();
    });

    bannerActionsEl.append(keepMine, takeTheirs);
    return;
  }

  if (deletedNotes.has(activePath)) {
    const notePath = activePath;
    bannerEl.className = "banner";
    bannerTextEl.textContent = "This Note was deleted on disk by another app.";
    bannerActionsEl.innerHTML = "";
    const saveAgain = document.createElement("button");
    saveAgain.className = "icon-btn";
    saveAgain.textContent = "Save again";
    saveAgain.addEventListener("click", async () => {
      await flushEditor();
      await invoke("save_note", { path: notePath, text: activeText });
      deletedNotes.delete(notePath);
      saveStatusEl.textContent = "Saved";
      updateBanner();
    });
    bannerActionsEl.append(saveAgain);
    return;
  }

  bannerEl.classList.add("hidden");
}

function showPane(pane: "empty" | "editor" | "attachment") {
  emptyStateEl.classList.toggle("hidden", pane !== "empty");
  editorEl.classList.toggle("hidden", pane !== "editor");
  attachmentViewEl.classList.toggle("hidden", pane !== "attachment");
}

async function flushEditor() {
  clearTimeout(saveTimer);
  saveTimer = undefined;
  const raw = sendToEditor({ type: "flush" });
  if (typeof raw === "string") {
    const msg = JSON.parse(raw) as Extract<EditorMessage, { type: "changed" }>;
    if (msg.noteID === activePath) activeText = msg.text;
    await invoke("update_note_buffer", { path: msg.noteID, revision: msg.revision, text: msg.text });
    await saveNow(msg.noteID, msg.text);
  } else if (activePath && activeKind === "note" && !conflictNotes.has(activePath) && !deletedNotes.has(activePath)) {
    await saveNow(activePath, activeText);
  }
}

async function saveNow(path: string, text: string) {
  if (conflictNotes.has(path) || deletedNotes.has(path)) return;
  try {
    const saved = await invoke<boolean>("save_note", { path, text });
    if (path === activePath && saved) {
      saveStatusEl.textContent = "Saved";
    }
  } catch (err) {
    if (path === activePath) saveStatusEl.textContent = "Save failed";
    console.error(err);
  }
}

async function openVault(path: string) {
  await flushEditor();
  const snap = await invoke<VaultSnapshot>("open_vault", { path });
  vaultRoot = snap.root;
  tree = snap.tree;
  activePath = null;
  activeKind = null;
  activeText = "";
  conflictNotes.clear();
  deletedNotes.clear();

  vaultNameEl.textContent = vaultDisplayName(snap.root);
  footerEl.textContent = `${snap.noteCount} notes`;
  btnNewNote.disabled = false;
  activeNotePathEl.textContent = "Select a Note";
  saveStatusEl.textContent = "";
  showPane("empty");
  sendToEditor({ type: "clear" });
  renderTree();
  updateBanner();

  const first = firstNote(tree);
  if (first) await openNote(first);
}

function firstNote(items: VaultItem[]): string | null {
  for (const item of items) {
    if (item.kind === "note") return item.path;
  }
  for (const item of items) {
    if (item.kind === "folder") {
      const found = firstNote(item.children);
      if (found) return found;
    }
  }
  return null;
}

async function openNote(path: string, line?: number) {
  if (activeKind === "note" && activePath && activePath !== path) {
    await flushEditor();
  }
  const resp = await invoke<OpenNoteResponse>("open_note", { path });
  activePath = resp.path;
  activeKind = "note";
  activeText = resp.text;
  if (resp.conflict) conflictNotes.add(resp.path);
  if (resp.deleted) deletedNotes.add(resp.path);

  expandParentsOf(resp.path);
  activeNotePathEl.textContent = resp.path;
  saveStatusEl.textContent = resp.conflict ? "Conflict" : "Saved";
  showPane("editor");
  renderTree();
  updateBanner();

  if (editorReady) {
    sendToEditor({ type: "load", noteID: resp.path, text: resp.text, line });
  }
}

async function openAttachment(path: string, kind: "image" | "pdf") {
  if (activeKind === "note" && activePath) {
    await flushEditor();
  }
  activePath = path;
  activeKind = "attachment";
  expandParentsOf(path);
  activeNotePathEl.textContent = path;
  saveStatusEl.textContent = "";
  showPane("attachment");
  renderTree();
  updateBanner();

  const src = "noteki-file://vault/" + path.split("/").map(encodeURIComponent).join("/");
  attachmentViewEl.innerHTML = "";
  if (kind === "image") {
    const img = document.createElement("img");
    img.src = src;
    img.alt = path;
    attachmentViewEl.append(img);
  } else {
    const frame = document.createElement("iframe");
    frame.src = src;
    attachmentViewEl.append(frame);
  }
}

async function pickVaultFolder() {
  const selected = await openDialog({ directory: true, multiple: false, title: "Open Vault" });
  if (typeof selected === "string") {
    await openVault(selected);
  }
}

async function createNote() {
  if (!vaultRoot) return;
  const folderRel = activePath && activePath.includes("/") ? activePath.slice(0, activePath.lastIndexOf("/")) : "";
  const [newPath, snap] = await invoke<[string, VaultSnapshot]>("create_new_note", { folderRel });
  tree = snap.tree;
  footerEl.textContent = `${snap.noteCount} notes`;
  await openNote(newPath);
}

setEditorMessageHandler((msg) => {
  switch (msg.type) {
    case "ready": {
      editorReady = true;
      void applyAppearanceAndTheme();
      if (activePath && activeKind === "note") {
        sendToEditor({ type: "load", noteID: activePath, text: activeText });
      }
      break;
    }
    case "changed": {
      if (msg.noteID === activePath) {
        activeText = msg.text;
        if (!conflictNotes.has(msg.noteID) && !deletedNotes.has(msg.noteID)) {
          saveStatusEl.textContent = "Unsaved…";
        }
      }
      void invoke("update_note_buffer", { path: msg.noteID, revision: msg.revision, text: msg.text });
      clearTimeout(saveTimer);
      saveTimer = setTimeout(() => {
        void saveNow(msg.noteID, msg.text);
      }, 900);
      break;
    }
    case "checkLinks": {
      void invoke<CheckLinksResult>("check_links", {
        notePath: msg.noteID,
        inners: msg.inners,
      }).then((res) => {
        sendToEditor({ type: "unresolvedLinks", noteID: msg.noteID, inners: res.unresolved });
        sendToEditor({ type: "embeds", noteID: msg.noteID, files: res.embeds });
      });
      break;
    }
    case "followLink": {
      if (!activePath) break;
      void invoke<FollowLinkResult>("follow_link", {
        sourcePath: activePath,
        innerText: msg.inner,
      }).then(async (res) => {
        if (res.kind === "note") await openNote(res.path);
        else await openAttachment(res.path, res.attachmentKind);
      });
      break;
    }
    case "openURL": {
      void openUrl(msg.url);
      break;
    }
    case "completeLink": {
      void invoke<LinkCompletions>("complete_link").then((completions) => {
        sendToEditor({ type: "linkCompletions", requestID: msg.requestID, completions });
      });
      break;
    }
    case "readFrontmatter": {
      void invoke<FrontmatterReading | null>("read_frontmatter", { text: msg.text }).then((reading) => {
        sendToEditor({
          type: "frontmatter",
          noteID: msg.noteID,
          text: msg.text,
          reading: reading ?? undefined,
        });
      });
      break;
    }
    case "format": {
      sendToEditor({ type: "format", action: msg.action });
      break;
    }
    case "addTag":
    case "removeTag":
    case "completeTag":
      // Tag chip editing can be wired next; keep prototype focused on core slice.
      break;
  }
});

btnOpenVault.addEventListener("click", () => void pickVaultFolder());
btnEmptyOpen.addEventListener("click", () => void pickVaultFolder());
btnNewNote.addEventListener("click", () => void createNote());
btnToggleSidebar.addEventListener("click", () => sidebarEl.classList.toggle("collapsed"));

btnSourceMode.addEventListener("click", () => {
  settings.sourceMode = !settings.sourceMode;
  void invoke("save_settings", { settings });
  void applyAppearanceAndTheme();
});

selectTheme.addEventListener("change", () => {
  settings.theme = selectTheme.value;
  void invoke("save_settings", { settings });
  void applyAppearanceAndTheme();
});

selectAppearance.addEventListener("change", () => {
  settings.appearance = selectAppearance.value;
  void invoke("save_settings", { settings });
  void applyAppearanceAndTheme();
});

window.addEventListener("keydown", (event) => {
  const mod = event.metaKey || event.ctrlKey;
  if (mod && event.key.toLowerCase() === "e" && !event.shiftKey) {
    event.preventDefault();
    btnSourceMode.click();
  } else if (mod && event.key.toLowerCase() === "n" && !event.shiftKey) {
    event.preventDefault();
    void createNote();
  } else if (mod && event.shiftKey && event.key.toLowerCase() === "s") {
    event.preventDefault();
    sidebarEl.classList.toggle("collapsed");
  }
});

window.addEventListener("beforeunload", () => {
  void flushEditor();
});

async function init() {
  await listen<{ tree: VaultItem[]; noteCount: number }>("vault-tree-changed", (ev) => {
    tree = ev.payload.tree;
    footerEl.textContent = `${ev.payload.noteCount} notes`;
    renderTree();
  });

  await listen<{ path: string; revision: number; text: string; merged: boolean }>("note-reloaded", (ev) => {
    if (ev.payload.path === activePath && activeKind === "note") {
      activeText = ev.payload.text;
      deletedNotes.delete(ev.payload.path);
      sendToEditor({
        type: "reload",
        noteID: ev.payload.path,
        revision: ev.payload.revision,
        text: ev.payload.text,
      });
      saveStatusEl.textContent = ev.payload.merged ? "Merged" : "Reloaded";
      updateBanner();
    }
  });

  await listen<{ path: string }>("note-conflict", (ev) => {
    conflictNotes.add(ev.payload.path);
    if (ev.payload.path === activePath) {
      saveStatusEl.textContent = "Conflict";
      updateBanner();
    }
  });

  await listen<{ path: string }>("note-deleted", (ev) => {
    deletedNotes.add(ev.payload.path);
    if (ev.payload.path === activePath) {
      saveStatusEl.textContent = "Deleted";
      updateBanner();
    }
  });

  settings = await invoke<AppSettings>("get_settings");
  await applyAppearanceAndTheme();
  if (settings.vaultPath) {
    try {
      await openVault(settings.vaultPath);
    } catch (err) {
      console.warn("Could not reopen previous vault:", err);
    }
  }
}

void init();
