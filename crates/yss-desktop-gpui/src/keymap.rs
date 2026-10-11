//! Application shortcuts replace a command's complete default set in its original context.
//! The immutable baseline includes component bindings; rebuilding never accumulates overrides.
use anyhow::{Context as _, Result, bail, ensure};
use gpui_kit::{
    Action, App, DummyKeyboardMapper, Global, KeyBinding, KeyBindingContextPredicate, Menu,
    MenuItem, NoAction, OsMenu, OwnedMenu, OwnedMenuItem,
};
use std::collections::HashSet;
use yss_settings::KeyBindingOverride;

pub(crate) struct ShortcutCommand {
    id: String,
    label: String,
    context: String,
    action: Box<dyn Action>,
    defaults: Vec<String>,
}

impl ShortcutCommand {
    pub(crate) fn new(id: String, label: &str, context: &str, action: Box<dyn Action>) -> Self {
        Self {
            id,
            label: label.into(),
            context: context.into(),
            action,
            defaults: vec![],
        }
    }

    pub(crate) fn id(&self) -> &str {
        &self.id
    }
    pub(crate) fn label(&self) -> String {
        crate::text::translate(&self.label)
    }
    pub(crate) fn context(&self) -> &str {
        &self.context
    }
    pub(crate) fn defaults(&self) -> &[String] {
        &self.defaults
    }

    fn matches(&self, binding: &KeyBinding) -> bool {
        binding.action().partial_eq(self.action.as_ref())
            && binding
                .predicate()
                .is_some_and(|predicate| predicate.to_string() == self.context)
    }
}

struct KeymapState {
    baseline: Vec<KeyBinding>,
    commands: Vec<ShortcutCommand>,
    applied: Vec<KeyBindingOverride>,
}
impl Global for KeymapState {}

/// Called once after component initialization, before any window or user preferences are loaded.
pub(crate) fn init(cx: &mut App, bindings: impl IntoIterator<Item = KeyBinding>) {
    let first_application_binding = cx.key_bindings().borrow().bindings().len();
    cx.bind_keys(bindings);
    crate::workbench::bind_menu_keys(cx);
    let baseline: Vec<_> = cx.key_bindings().borrow().bindings().cloned().collect();
    let mut commands = crate::workbench::shortcut_commands();
    for binding in &baseline[first_application_binding..] {
        if !commands.iter().any(|command| command.matches(binding)) {
            let id = binding.action().name().to_string();
            let label = format!(
                "preferences.shortcuts.actions.{}",
                id.rsplit("::").next().unwrap_or(&id)
            );
            commands.push(ShortcutCommand::new(
                id,
                &label,
                &binding
                    .predicate()
                    .expect("application shortcuts have a context")
                    .to_string(),
                binding.action().boxed_clone(),
            ));
        }
    }
    for command in &mut commands {
        command.defaults = baseline
            .iter()
            .filter(|binding| command.matches(binding))
            .map(shortcut_text)
            .collect();
    }
    cx.set_global(KeymapState {
        baseline,
        commands,
        applied: vec![],
    });
}

pub(crate) fn commands(cx: &App) -> &[ShortcutCommand] {
    &cx.global::<KeymapState>().commands
}

pub(crate) fn shortcut_text(binding: &KeyBinding) -> String {
    binding
        .keystrokes()
        .iter()
        .map(|stroke| stroke.unparse())
        .collect::<Vec<_>>()
        .join(" ")
}

fn override_for<'a>(
    command: &ShortcutCommand,
    bindings: &'a [KeyBindingOverride],
) -> Option<&'a KeyBindingOverride> {
    bindings.iter().find(|binding| {
        binding.action == command.id && binding.context.as_deref() == Some(&command.context)
    })
}

fn load(command: &ShortcutCommand, text: &str) -> Result<KeyBinding> {
    let binding = KeyBinding::load(
        text,
        command.action.boxed_clone(),
        Some(KeyBindingContextPredicate::parse(&command.context)?.into()),
        false,
        None,
        &DummyKeyboardMapper,
    )
    .with_context(|| {
        crate::text::format(
            "preferences.shortcuts.invalid",
            &[
                ("shortcut", text.into()),
                ("action", command.id.clone()),
                ("context", command.context.clone()),
            ],
        )
    })?;
    ensure!(
        !binding.keystrokes().is_empty(),
        crate::text::translate("preferences.shortcuts.empty")
    );
    for stroke in binding.keystrokes() {
        let key = stroke.key();
        let function_key = key
            .strip_prefix('f')
            .and_then(|number| number.parse::<u8>().ok())
            .is_some_and(|number| (1..=35).contains(&number));
        ensure!(
            key.chars().count() == 1
                || function_key
                || matches!(
                    key,
                    "space"
                        | "tab"
                        | "enter"
                        | "escape"
                        | "backspace"
                        | "delete"
                        | "insert"
                        | "home"
                        | "end"
                        | "pageup"
                        | "pagedown"
                        | "up"
                        | "down"
                        | "left"
                        | "right"
                ),
            crate::text::format("preferences.shortcuts.unknownKey", &[("key", key.into())])
        );
    }
    Ok(binding)
}

fn overlapping_context(a: &KeyBinding, b: &KeyBinding) -> bool {
    match (a.predicate(), b.predicate()) {
        (Some(a), Some(b)) => a.is_superset(&b) || b.is_superset(&a),
        (None, None) => true,
        _ => false,
    }
}

fn conflicts(a: &KeyBinding, b: &KeyBinding) -> bool {
    let a_keys = a.keystrokes();
    let b_keys = b.keystrokes();
    // Reject ambiguous chord prefixes as well as exact matches in the same scope.
    (a_keys.starts_with(b_keys) || b_keys.starts_with(a_keys)) && overlapping_context(a, b)
}

fn prepare(bindings: &[KeyBindingOverride], state: &KeymapState) -> Result<Vec<KeyBinding>> {
    let mut seen = HashSet::new();
    let mut replacements = vec![];
    for binding in bindings {
        ensure!(
            seen.insert((&binding.action, &binding.context)),
            crate::text::format(
                "preferences.shortcuts.duplicate",
                &[
                    ("action", binding.action.clone()),
                    ("context", binding.context.clone().unwrap_or_default()),
                ]
            )
        );
        let command = state
            .commands
            .iter()
            .find(|command| {
                command.id == binding.action && binding.context.as_deref() == Some(&command.context)
            })
            .with_context(|| {
                crate::text::format(
                    "preferences.shortcuts.unknownAction",
                    &[
                        ("action", binding.action.clone()),
                        ("context", binding.context.clone().unwrap_or_default()),
                    ],
                )
            })?;
        if !binding.keystroke.is_empty() {
            replacements.push(load(command, &binding.keystroke)?);
        }
    }
    let mut effective: Vec<_> = state
        .baseline
        .iter()
        .filter(|binding| {
            !state.commands.iter().any(|command| {
                command.matches(binding) && override_for(command, bindings).is_some()
            })
        })
        .cloned()
        .collect();
    for (ix, replacement) in replacements.iter().enumerate() {
        if let Some(other) = effective
            .iter()
            .chain(replacements[..ix].iter())
            .find(|other| conflicts(replacement, other))
        {
            let action = state
                .commands
                .iter()
                .find(|command| command.matches(other))
                .map(|command| command.id.clone())
                .unwrap_or_else(|| other.action().name().into());
            bail!(crate::text::format(
                "preferences.shortcuts.conflict",
                &[
                    ("shortcut", shortcut_text(replacement)),
                    ("action", action),
                    (
                        "context",
                        other
                            .predicate()
                            .map(|predicate| predicate.to_string())
                            .unwrap_or_default()
                    ),
                    ("existing", shortcut_text(other)),
                ]
            ));
        }
    }
    // Block an overridden command's former keys at its original depth. Without this,
    // a SaveGraph binding on Workbench could still handle a removed GraphCanvas key.
    for command in &state.commands {
        if override_for(command, bindings).is_some() {
            for original in state
                .baseline
                .iter()
                .filter(|binding| command.matches(binding))
            {
                effective.push(KeyBinding::load(
                    &shortcut_text(original),
                    Box::new(NoAction),
                    original.predicate(),
                    false,
                    None,
                    &DummyKeyboardMapper,
                )?);
            }
        }
    }
    effective.extend(replacements);
    Ok(effective)
}

pub(crate) fn validate_bindings(bindings: &[KeyBindingOverride], cx: &App) -> Result<()> {
    prepare(bindings, cx.global::<KeymapState>()).map(|_| ())
}

pub(crate) fn apply_bindings(bindings: &[KeyBindingOverride], cx: &mut App) -> Result<()> {
    if cx.global::<KeymapState>().applied == bindings {
        return Ok(());
    }
    let effective = prepare(bindings, cx.global::<KeymapState>())?;
    cx.clear_key_bindings();
    cx.bind_keys(effective);
    cx.global_mut::<KeymapState>().applied = bindings.to_vec();
    // Native menus cache accelerators when set, independently of window refreshes.
    if let Some(menus) = cx.get_menus() {
        cx.set_menus(menus.into_iter().map(restore_menu));
    }
    Ok(())
}

fn restore_menu(menu: OwnedMenu) -> Menu {
    Menu::new(menu.name)
        .disabled(menu.disabled)
        .items(menu.items.into_iter().map(|item| match item {
            OwnedMenuItem::Separator => MenuItem::Separator,
            OwnedMenuItem::Submenu(menu) => MenuItem::Submenu(restore_menu(menu)),
            OwnedMenuItem::SystemMenu(menu) => MenuItem::SystemMenu(OsMenu {
                name: menu.name,
                menu_type: menu.menu_type,
            }),
            OwnedMenuItem::Action {
                name,
                action,
                os_action,
                checked,
                disabled,
            } => MenuItem::Action {
                name: name.into(),
                action,
                os_action,
                checked,
                disabled,
            },
        }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::canvas::{SaveGraph, UndoGraph};
    use gpui_kit::{KeyContext, Keymap, Keystroke};

    fn state() -> KeymapState {
        KeymapState {
            baseline: vec![
                KeyBinding::new("ctrl-s", SaveGraph, Some("Workbench")),
                KeyBinding::new("ctrl-s", SaveGraph, Some("GraphCanvas")),
                KeyBinding::new("cmd-s", SaveGraph, Some("GraphCanvas")),
                KeyBinding::new("ctrl-z", UndoGraph, Some("GraphCanvas")),
                KeyBinding::new("ctrl-z", gpui_kit::component::input::Undo, Some("Input")),
            ],
            commands: vec![
                ShortcutCommand::new("save".into(), "Save", "GraphCanvas", Box::new(SaveGraph)),
                ShortcutCommand::new("undo".into(), "Undo", "GraphCanvas", Box::new(UndoGraph)),
            ],
            applied: vec![],
        }
    }

    fn binding(action: &str, key: &str) -> KeyBindingOverride {
        KeyBindingOverride::new(action.into(), Some("GraphCanvas".into()), key.into())
    }

    #[test]
    fn rebind_removes_both_defaults_and_blocks_ancestor_fallback() {
        let state = state();
        let map = Keymap::new(prepare(&[binding("save", "ctrl-alt-s")], &state).unwrap());
        let contexts = [
            KeyContext::parse("Workbench").unwrap(),
            KeyContext::parse("GraphCanvas").unwrap(),
        ];
        for old in ["ctrl-s", "cmd-s"] {
            assert!(
                map.bindings_for_input(&[Keystroke::parse(old).unwrap()], &contexts)
                    .0
                    .is_empty()
            );
        }
        let matches = map
            .bindings_for_input(&[Keystroke::parse("ctrl-alt-s").unwrap()], &contexts)
            .0;
        assert!(matches[0].action().partial_eq(&SaveGraph));
        let reset = Keymap::new(prepare(&[], &state).unwrap());
        assert!(
            reset
                .bindings_for_input(&[Keystroke::parse("ctrl-s").unwrap()], &contexts)
                .0[0]
                .action()
                .partial_eq(&SaveGraph)
        );
        assert_eq!(reset.bindings().len(), state.baseline.len());
    }

    #[test]
    fn remove_preserves_unrelated_text_editing() {
        let state = state();
        let map = Keymap::new(prepare(&[binding("undo", "")], &state).unwrap());
        let contexts = [
            KeyContext::parse("Workbench").unwrap(),
            KeyContext::parse("GraphCanvas").unwrap(),
            KeyContext::parse("Input").unwrap(),
        ];
        let matches = map
            .bindings_for_input(&[Keystroke::parse("ctrl-z").unwrap()], &contexts)
            .0;
        assert!(
            matches[0]
                .action()
                .partial_eq(&gpui_kit::component::input::Undo)
        );
    }

    #[test]
    fn invalid_and_conflicting_overrides_leave_baseline_unchanged() {
        let state = state();
        for overrides in [
            vec![binding("save", "ctrl-z")],
            vec![binding("save", "ctrl-k"), binding("undo", "ctrl-k ctrl-z")],
            vec![binding("save", "ctrl-k"), binding("save", "alt-k")],
            vec![binding("save", "ctrl-not-a-key")],
            vec![binding("missing", "ctrl-k")],
            vec![KeyBindingOverride::new(
                "save".into(),
                Some("Input".into()),
                "ctrl-k".into(),
            )],
        ] {
            assert!(prepare(&overrides, &state).is_err());
        }
        assert_eq!(state.baseline.len(), 5);
        assert_eq!(shortcut_text(&state.baseline[1]), "ctrl-s");
    }

    #[test]
    fn replacing_one_override_never_accumulates_prior_bindings() {
        let state = state();
        let first = prepare(&[binding("save", "ctrl-alt-s")], &state).unwrap();
        let second = prepare(&[binding("save", "ctrl-alt-d")], &state).unwrap();
        assert_eq!(first.len(), second.len());
        assert!(
            !second
                .iter()
                .any(|binding| shortcut_text(binding) == "ctrl-alt-s")
        );
        assert!(
            prepare(
                &[binding("save", "super-k"), binding("undo", "cmd-k")],
                &state
            )
            .is_err()
        );
    }
}
