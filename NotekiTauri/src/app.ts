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

interface SearchResult {
  path: string;
  name: string;
  nameMatched: boolean;
  lines: { line: number; text: string }[];
  totalLineMatches: number;
}

interface TagNode {
  fullLower: string;
  label: string;
  count: number;
  notePaths: string[];
  children: TagNode[];
}

interface BacklinkMatch {
  sourcePath: string;
  sourceName: string;
  line: number;
  snippet: string;
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
const searchPaneEl = document.getElementById("search-pane")!;
const searchInputEl = document.getElementById("search-input") as HTMLInputElement;
const searchResultsEl = document.getElementById("search-results")!;
const tagsPaneEl = document.getElementById("tags-pane")!;
const backlinksEl = document.getElementById("backlinks")!;
const backlinksToggleEl = document.getElementById("backlinks-toggle") as HTMLButtonElement;
const backlinksListEl = document.getElementById("backlinks-list")!;
const paneTabs = Array.from(document.querySelectorAll<HTMLButtonElement>(".pane-tab"));

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
let searchTimer: ReturnType<typeof setTimeout> | undefined;
let activePane: "files" | "search" | "tags" = "files";
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
      const resolved = await invoke<string>("resolve_conflict", { path: notePath, choice: "keep_mine" });
      conflictNotes.delete(notePath);
      activeText = resolved;
      saveStatusEl.textContent = "Saved";
      updateBanner();
    });

    const takeTheirs = document.createElement("button");
    takeTheirs.className = "icon-btn";
    takeTheirs.textContent = "Take theirs";
    takeTheirs.addEventListener("click", async () => {
      await flushEditor();
      const resolved = await invoke<string>("resolve_conflict", { path: notePath, choice: "take_theirs" });
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
      deletedNotes.delete(notePath);
      await flushEditor();
      await invoke("save_again", { path: notePath });
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
    await saveNow(msg.noteID);
  } else if (activePath && activeKind === "note") {
    await saveNow(activePath);
  }
}

/** Saves the text last reported to the backend (it refuses during a conflict or a deletion). */
async function saveNow(path: string) {
  if (conflictNotes.has(path) || deletedNotes.has(path)) return;
  try {
    const saved = await invoke<boolean>("save_note", { path });
    if (path === activePath && saved) {
      saveStatusEl.textContent = "Saved";
      void refreshBacklinks();
      if (activePane === "tags") void renderTags();
      if (activePane === "search") void runSearch();
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
  backlinksEl.classList.add("hidden");
  if (activePane === "tags") void renderTags();

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
  void refreshBacklinks();
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
  backlinksEl.classList.add("hidden");
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
        void saveNow(msg.noteID);
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
  } else if (mod && event.shiftKey && event.key.toLowerCase() === "f") {
    event.preventDefault();
    showSidebarPane("search");
  } else if (mod && event.shiftKey && event.key.toLowerCase() === "s") {
    event.preventDefault();
    sidebarEl.classList.toggle("collapsed");
  }
});

// ---- Search, Tags, Backlinks ----

function showSidebarPane(pane: "files" | "search" | "tags") {
  activePane = pane;
  for (const tab of paneTabs) tab.classList.toggle("active", tab.dataset.pane === pane);
  fileTreeEl.classList.toggle("hidden", pane !== "files");
  searchPaneEl.classList.toggle("hidden", pane !== "search");
  tagsPaneEl.classList.toggle("hidden", pane !== "tags");
  sidebarEl.classList.remove("collapsed");
  if (pane === "search") {
    searchInputEl.focus();
    searchInputEl.select();
  }
  if (pane === "tags") void renderTags();
}

function row(className: string, text: string, onClick: () => void, extra?: string): HTMLElement {
  const el = document.createElement("div");
  el.className = `list-row ${className}`;
  el.textContent = text;
  if (extra) {
    const span = document.createElement("span");
    span.className = className === "sub" ? "line-no" : "count";
    span.textContent = extra;
    className === "sub" ? el.prepend(span) : el.append(span);
  }
  el.addEventListener("click", onClick);
  return el;
}

function empty(text: string): HTMLElement {
  const el = document.createElement("div");
  el.className = "list-empty";
  el.textContent = text;
  return el;
}

async function runSearch() {
  const query = searchInputEl.value;
  if (!vaultRoot || !query.trim()) {
    searchResultsEl.replaceChildren(empty("Words, \"a phrase\", tag:work, path:Daily"));
    return;
  }
  const results = await invoke<SearchResult[]>("search", { query });
  if (query !== searchInputEl.value) return; // typed on meanwhile
  const items: HTMLElement[] = [];
  for (const r of results) {
    items.push(row("title", r.name, () => void openNote(r.path), String(r.totalLineMatches || "")));
    for (const l of r.lines) items.push(row("sub", l.text, () => void openNote(r.path, l.line), String(l.line)));
  }
  searchResultsEl.replaceChildren(...(items.length ? items : [empty("No results")]));
}

async function renderTags() {
  if (!vaultRoot) {
    tagsPaneEl.replaceChildren(empty("No vault"));
    return;
  }
  const tags = await invoke<TagNode[]>("tags");
  const items: HTMLElement[] = [];
  const add = (nodes: TagNode[], depth: number) => {
    for (const t of nodes) {
      const el = row("", `#${t.label}`, () => {
        searchInputEl.value = `tag:${t.fullLower}`;
        showSidebarPane("search");
        void runSearch();
      }, String(t.count));
      el.style.paddingLeft = `${8 + depth * 14}px`;
      items.push(el);
      add(t.children, depth + 1);
    }
  };
  add(tags, 0);
  tagsPaneEl.replaceChildren(...(items.length ? items : [empty("No tags")]));
}

async function refreshBacklinks() {
  if (!activePath || activeKind !== "note") {
    backlinksEl.classList.add("hidden");
    return;
  }
  const path = activePath;
  const links = await invoke<BacklinkMatch[]>("backlinks", { path });
  if (path !== activePath) return;
  backlinksEl.classList.remove("hidden");
  backlinksToggleEl.textContent = links.length ? `Backlinks (${links.length})` : "No backlinks";
  backlinksListEl.replaceChildren(
    ...links.map((b) => {
      const el = row("", b.sourceName, () => void openNote(b.sourcePath, b.line));
      el.title = b.snippet;
      const snippet = document.createElement("span");
      snippet.className = "count";
      snippet.textContent = b.snippet;
      el.append(snippet);
      return el;
    }),
  );
}

for (const tab of paneTabs) {
  tab.addEventListener("click", () => showSidebarPane(tab.dataset.pane as "files" | "search" | "tags"));
}
searchInputEl.addEventListener("input", () => {
  clearTimeout(searchTimer);
  searchTimer = setTimeout(() => void runSearch(), 150);
});
backlinksToggleEl.addEventListener("click", () => backlinksEl.classList.toggle("collapsed"));

async function init() {
  await listen<{ tree: VaultItem[]; noteCount: number }>("vault-tree-changed", (ev) => {
    tree = ev.payload.tree;
    footerEl.textContent = `${ev.payload.noteCount} notes`;
    renderTree();
    if (activePane === "tags") void renderTags();
    if (activePane === "search") void runSearch();
  });

  await listen<{ path: string; revision: number; text: string; merged: boolean }>("note-reloaded", (ev) => {
    if (ev.payload.path === activePath && activeKind === "note") {
      deletedNotes.delete(ev.payload.path);
      const applied = sendToEditor({
        type: "reload",
        noteID: ev.payload.path,
        revision: ev.payload.revision,
        text: ev.payload.text,
      });
      if (applied === "applied") {
        activeText = ev.payload.text;
        saveStatusEl.textContent = ev.payload.merged ? "Merged" : "Reloaded";
      } else {
        // The user typed meanwhile: the backend merges the disk change into that text.
        void invoke("reload_rejected", { path: ev.payload.path });
      }
      updateBanner();
    }
    void refreshBacklinks();
  });

  await listen<{ from: string; to: string }>("note-renamed", async (ev) => {
    const { from, to } = ev.payload;
    conflictNotes.delete(from);
    deletedNotes.delete(from);
    if (from === activePath && activeKind === "note") {
      // What the editor has not reported yet goes to the note under its new path.
      clearTimeout(saveTimer);
      const raw = sendToEditor({ type: "flush" });
      sendToEditor({ type: "drop", noteIDs: [from] });
      activePath = null;
      await openNote(to);
      if (typeof raw === "string") {
        const msg = JSON.parse(raw) as Extract<EditorMessage, { type: "changed" }>;
        sendToEditor({ type: "reload", noteID: to, revision: 0, text: msg.text });
        await invoke("update_note_buffer", { path: to, revision: 0, text: msg.text });
        await saveNow(to);
      }
      saveStatusEl.textContent = "Moved by another app";
    }
  });

  await listen<{ path: string; error: string }>("save-failed", (ev) => {
    if (ev.payload.path === activePath) saveStatusEl.textContent = "Save failed";
    console.error("Save failed", ev.payload);
  });

  await listen("app-will-quit", async () => {
    try {
      await flushEditor();
    } finally {
      await invoke("quit_app");
    }
  });

  await getCurrentWindow().onCloseRequested(async () => {
    // Up to 250 ms of typing may be unreported: save it before the window goes.
    await flushEditor();
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
