//! [`Rule`]: what a chord becomes, and where.

use crate::Chord;

/// The app a rule is limited to, named the way its system names apps. A rule written on one
/// system never matches on another: a Windows program file is no macOS bundle.
///
/// In settings files: `app = { exe = "chrome.exe" }`, `{ bundle_id = "com.google.Chrome" }` or
/// `{ window_class = "firefox" }`.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize), serde(rename_all = "snake_case"))]
pub enum AppMatcher {
    /// Windows: the program's file name, `chrome.exe`.
    Exe(String),
    /// macOS: the app's bundle identifier, `com.google.Chrome`.
    BundleId(String),
    /// Linux: the window class (X11 `WM_CLASS`, Wayland `app_id`), `firefox`.
    WindowClass(String),
}

impl AppMatcher {
    /// The name it matches, as the user picked it.
    pub fn name(&self) -> &str {
        match self {
            AppMatcher::Exe(name) | AppMatcher::BundleId(name) | AppMatcher::WindowClass(name) => name,
        }
    }

    /// Whether `app` is the one, ignoring case. Doesn't allocate: it runs for every key.
    pub fn matches(&self, app: App<'_>) -> bool {
        match (self, app) {
            (AppMatcher::Exe(want), App::Exe(name))
            | (AppMatcher::BundleId(want), App::BundleId(name))
            | (AppMatcher::WindowClass(want), App::WindowClass(name)) => want.eq_ignore_ascii_case(name),
            _ => false,
        }
    }
}

/// The app in front, as the system's backend knows it. `Unknown` matches no app's rules.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum App<'a> {
    Exe(&'a str),
    BundleId(&'a str),
    WindowClass(&'a str),
    Unknown,
}

/// Pressing `from` sends `to` instead, everywhere or only while `app` is in front.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Rule {
    pub from: Chord,
    pub to: Chord,
    /// `None` for every app.
    pub app: Option<AppMatcher>,
}

impl Rule {
    /// Key to key, with no modifiers on either side: it applies whatever modifiers are held, which
    /// pass through (Shift + Caps Lock with Caps Lock to Esc is Shift + Esc).
    pub fn is_plain(&self) -> bool {
        self.from.mods.is_empty() && self.to.mods.is_empty()
    }

    /// Whether `other` fires on the same keys in the same place, so only one of them can stay.
    pub fn clashes_with(&self, other: &Self) -> bool {
        self.from == other.from && self.app == other.app
    }
}

/// Puts `rules` in the order [`decide`](crate::decide) looks them up: rules for one app before
/// global ones, and shortcuts before plain remaps.
pub fn sort_rules(rules: &mut [Rule]) {
    rules.sort_by_key(|r| (r.app.is_none(), r.is_plain()));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn apps_match_their_own_system_ignoring_case() {
        let chrome = AppMatcher::Exe("chrome.exe".into());
        assert!(chrome.matches(App::Exe("Chrome.EXE")));
        assert!(!chrome.matches(App::Exe("code.exe")));
        assert!(!chrome.matches(App::BundleId("chrome.exe")));
        assert!(!chrome.matches(App::Unknown));
        assert!(AppMatcher::BundleId("com.google.Chrome".into()).matches(App::BundleId("com.google.chrome")));
    }
}
