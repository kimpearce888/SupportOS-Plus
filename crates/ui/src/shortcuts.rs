use wasm_bindgen::JsCast;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShortcutAction {
    None,
    OpenPalette,
    QuickSearch,
    Navigate(&'static str),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ShortcutOutcome {
    pub action: ShortcutAction,
    pub pending_g: bool,
}

#[must_use]
pub fn resolve(pending_g: bool, typing: bool, mod_k: bool, key: &str) -> ShortcutOutcome {
    if mod_k {
        return ShortcutOutcome {
            action: ShortcutAction::OpenPalette,
            pending_g: false,
        };
    }
    if typing {
        return ShortcutOutcome {
            action: ShortcutAction::None,
            pending_g: false,
        };
    }
    if key == "/" {
        return ShortcutOutcome {
            action: ShortcutAction::QuickSearch,
            pending_g: false,
        };
    }
    if key == "g" {
        return ShortcutOutcome {
            action: ShortcutAction::None,
            pending_g: true,
        };
    }
    if pending_g {
        let action = match key {
            "d" => ShortcutAction::Navigate("/"),
            "i" => ShortcutAction::Navigate("/inbox"),
            "s" => ShortcutAction::Navigate("/sync-health"),
            _ => ShortcutAction::None,
        };
        return ShortcutOutcome {
            action,
            pending_g: false,
        };
    }
    ShortcutOutcome {
        action: ShortcutAction::None,
        pending_g: false,
    }
}

#[must_use]
pub fn is_typing(target: Option<&web_sys::EventTarget>) -> bool {
    target
        .and_then(|t| t.dyn_ref::<web_sys::HtmlElement>())
        .is_some_and(|el| {
            let tag = el.tag_name();
            tag == "INPUT" || tag == "TEXTAREA" || el.is_content_editable()
        })
}
