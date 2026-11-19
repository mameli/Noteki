//! The dialogs: Quick Open, the command palette, the Wikilink picker and
//! the name prompts.

use super::*;

impl NotekiApp {
    pub(crate) fn open_name_prompt(
        &mut self,
        kind: PromptKind,
        value: String,
        target: Option<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.vault.is_none() {
            return;
        }
        let input = cx.new(|cx| {
            InputState::new(window, cx)
                .default_value(value)
                .placeholder(match kind {
                    PromptKind::NewNote => "Note name",
                    PromptKind::NewFolder => "Folder name",
                    PromptKind::Rename => "New name",
                })
        });
        let (title, ok) = match kind {
            PromptKind::NewNote => ("New Note", "Create"),
            PromptKind::NewFolder => ("New Folder", "Create"),
            PromptKind::Rename => ("Rename", "Rename"),
        };
        let entity = cx.entity();
        let field = input.clone();
        window.open_dialog(cx, move |dialog, _, _| {
            let (input_ok, entity, target) = (input.clone(), entity.clone(), target.clone());
            dialog
                .title(title)
                .width(px(380.))
                .button_props(DialogButtonProps::default().ok_text(ok).show_cancel(true))
                .child(Input::new(&input).id("prompt-input"))
                .on_ok(move |_, window, cx| {
                    let value = input_ok.read(cx).value().trim().to_string();
                    if !value.is_empty() {
                        entity.update(cx, |app, cx| {
                            app.run_prompt(kind, &value, target.as_deref(), window, cx)
                        });
                    }
                    true
                })
        });
        // The dialog takes focus as it opens; typing goes to the name at once.
        field.update(cx, |s, cx| s.focus(window, cx));
    }

    pub(in crate::app) fn open_command_dialog(
        &mut self,
        placeholder: &'static str,
        items: Vec<CommandItem>,
        on_pick: impl Fn(&mut NotekiApp, usize, &mut Window, &mut Context<NotekiApp>) + 'static,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let state = cx.new(|cx| CommandState::new(window, cx));
        let entity = cx.entity();
        let on_pick = std::rc::Rc::new(on_pick);
        let query = state.clone();
        window.open_dialog(cx, move |dialog, _, _| {
            let (entity, on_pick) = (entity.clone(), on_pick.clone());
            dialog.width(px(540.)).close_button(false).p_0().child(
                Command::new(&state)
                    .bordered(false)
                    .placeholder(placeholder)
                    .items(items.clone())
                    .on_confirm(move |index, window, cx| {
                        window.close_dialog(cx);
                        let on_pick = on_pick.clone();
                        entity.update(cx, |app, cx| on_pick(app, index.row, window, cx));
                    })
                    .on_cancel(|window, cx| window.close_dialog(cx)),
            )
        });
        // The dialog takes focus as it opens; typing goes to the query at once.
        query.update(cx, |s, cx| s.focus(window, cx));
    }

    pub(crate) fn open_quick_switcher(
        &mut self,
        mode: OpenIn,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(vault) = &self.vault else { return };
        let ranked = crate::core::index::quick_switcher_rank(&vault.notes, &self.recent_notes, "");
        let items = ranked
            .iter()
            .map(|e| {
                let label = if e.folder.is_empty() {
                    e.name.clone()
                } else {
                    format!("{}  ·  {}", e.name, e.folder)
                };
                CommandItem::new()
                    .label(label)
                    .icon(AssetIcon::FileText)
                    .keywords([e.path.clone()])
            })
            .collect();
        let paths: Vec<String> = ranked.into_iter().map(|e| e.path).collect();
        self.open_command_dialog(
            "Open a note…",
            items,
            move |app, row, window, cx| {
                if let Some(p) = paths.get(row) {
                    app.open_path(p, mode, window, cx);
                }
            },
            window,
            cx,
        );
    }

    pub(crate) fn open_wikilink_picker(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.editor_path.is_none() {
            return;
        }
        let all = self.link_completions();
        let items = all
            .iter()
            .map(|(name, folder, _)| {
                let label = if folder.is_empty() {
                    name.clone()
                } else {
                    format!("{name}  ·  {folder}")
                };
                CommandItem::new().label(label).icon(AssetIcon::Link)
            })
            .collect();
        let links: Vec<String> = all.into_iter().map(|(_, _, l)| l).collect();
        self.open_command_dialog(
            "Insert a link to…",
            items,
            move |app, row, window, cx| {
                if let Some(l) = links.get(row) {
                    app.insert_wikilink(l, window, cx);
                }
            },
            window,
            cx,
        );
    }

    /// The Command palette (⇧⌘P): every command of the menus, with its
    /// shortcut and its tick, and those only here (the theme, the interface's
    /// size). A command runs as its menu item does, on what had the keyboard.
    pub(crate) fn open_command_palette(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        type Do = fn(&mut NotekiApp, &mut Window, &mut Context<NotekiApp>);
        enum Run {
            Action(Box<dyn gpui_kit::Action>),
            Do(Do),
        }
        let editor_focus = self.editor.focus_handle(cx);
        let mut items = Vec::new();
        let mut runs = Vec::new();
        let mut add = |label: String, checked: bool, run: Run, window: &Window| {
            let stroke = match &run {
                Run::Action(a) => window
                    .highest_precedence_binding_for_action_in(a.as_ref(), &editor_focus)
                    .or_else(|| window.highest_precedence_binding_for_action(a.as_ref()))
                    .and_then(|b| {
                        b.keystrokes()
                            .first()
                            .map(|k| gpui_kit::AsKeystroke::as_keystroke(k).clone())
                    }),
                Run::Do(_) => None,
            };
            let text = label.clone();
            items.push(CommandItem::new().label(label).child(move |_, cx| {
                h_flex()
                    .flex_1()
                    .gap_2()
                    .items_center()
                    .child(text.clone())
                    .child(div().flex_1())
                    .when_some(stroke.clone(), |row, k| {
                        row.child(gpui_kit::component::kbd::Kbd::new(k))
                    })
                    .when(checked, |row| {
                        row.child(
                            Icon::new(AssetIcon::Check)
                                .xsmall()
                                .text_color(cx.theme().muted_foreground),
                        )
                    })
            }));
            runs.push(run);
        };
        pub(in crate::app) fn walk(
            menu: &gpui_kit::Menu,
            path: &str,
            out: &mut Vec<(String, bool, Box<dyn gpui_kit::Action>)>,
        ) {
            for item in &menu.items {
                match item {
                    gpui_kit::MenuItem::Action {
                        name,
                        action,
                        checked,
                        ..
                    } => out.push((format!("{path}: {name}"), *checked, action.boxed_clone())),
                    gpui_kit::MenuItem::Submenu(sub) => {
                        walk(sub, &format!("{path} › {}", sub.name), out)
                    }
                    _ => {}
                }
            }
        }
        let mut from_menus = Vec::new();
        // The app's own menu (Settings, Hide, Quit) last: the Note's first.
        let mut menus = super::menus::app_menus(&self.settings);
        menus.rotate_left(1);
        for menu in menus {
            walk(&menu, &menu.name, &mut from_menus);
        }
        for (label, checked, action) in from_menus {
            // Itself, and the list of shortcuts that opens it.
            if action.partial_eq(&OpenCommandPalette) || action.partial_eq(&KeyboardShortcutsAction)
            {
                continue;
            }
            add(label, checked, Run::Action(action), window);
        }
        let own: [(&str, Do); 2] = [
            ("View: Next Theme", |a, w, cx| a.cycle_theme(w, cx)),
            ("View: Light / Dark / System", |a, w, cx| {
                a.cycle_appearance(w, cx)
            }),
        ];
        for (label, run) in own {
            add(label.to_string(), false, Run::Do(run), window);
        }
        let sizes: [(InterfaceSize, Do); 3] = [
            (InterfaceSize::Small, |a, w, cx| {
                a.update_settings(|s| s.interface_size = InterfaceSize::Small, w, cx)
            }),
            (InterfaceSize::Medium, |a, w, cx| {
                a.update_settings(|s| s.interface_size = InterfaceSize::Medium, w, cx)
            }),
            (InterfaceSize::Large, |a, w, cx| {
                a.update_settings(|s| s.interface_size = InterfaceSize::Large, w, cx)
            }),
        ];
        for (size, run) in sizes {
            let label = format!("View: Interface Size › {}", size.label());
            add(
                label,
                self.settings.interface_size == size,
                Run::Do(run),
                window,
            );
        }
        self.open_command_dialog(
            "Run a command…",
            items,
            move |app, row, window, cx| match runs.get(row) {
                Some(Run::Action(action)) => window.dispatch_action(action.boxed_clone(), cx),
                Some(Run::Do(run)) => run(app, window, cx),
                None => {}
            },
            window,
            cx,
        );
    }
}
