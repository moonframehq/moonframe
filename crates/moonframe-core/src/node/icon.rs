//! The bundled icons and the names that pick one.

use std::sync::Arc;

/// What names a host app's own icon: this, then its name in the host's
/// assets (`app-sessions` is the host's `icons/app-sessions.svg`).
pub const APP_ICON: &str = "app-";
/// What an app icon the host does not ship draws instead: a dashed outline,
/// so a missing file shows as a gap to fill rather than nothing at all.
pub const MISSING_APP_ICON: &str = "circle-dashed";
/// The longest name an app icon takes after [`APP_ICON`].
const MAX_APP_ICON: usize = 64;

/// One of [`ICONS`], or an icon the host app ships (`app-…`, see
/// [`APP_ICON`]); anything else is refused when the view is read, so a typo
/// is an error rather than a blank space. A host's icon is only checked for
/// its shape: one the host lacks draws [`MISSING_APP_ICON`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IconName(Name);

#[derive(Clone, Debug, PartialEq, Eq)]
enum Name {
    Bundled(&'static str),
    App(Arc<str>),
}

impl IconName {
    pub fn new(name: &str) -> Option<Self> {
        if let Some(rest) = name.strip_prefix(APP_ICON) {
            return app_icon(rest).then(|| Self(Name::App(name.into())));
        }
        ICONS
            .binary_search(&name)
            .ok()
            .map(|index| Self(Name::Bundled(ICONS[index])))
    }

    pub fn name(&self) -> &str {
        match &self.0 {
            Name::Bundled(name) => name,
            Name::App(name) => name,
        }
    }

    /// The name when Moonframe bundles the icon, not the host app.
    pub fn bundled(&self) -> Option<&'static str> {
        match self.0 {
            Name::Bundled(name) => Some(name),
            Name::App(_) => None,
        }
    }

    /// Where the app's assets keep it.
    pub fn path(&self) -> String {
        format!("icons/{}.svg", self.name())
    }
}

/// Whether `rest`, after [`APP_ICON`], is a name an app icon may have:
/// lowercase words of letters and digits joined by single hyphens.
fn app_icon(rest: &str) -> bool {
    !rest.is_empty()
        && rest.len() <= MAX_APP_ICON
        && rest.split('-').all(|word| {
            !word.is_empty()
                && word
                    .bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit())
        })
}

/// The icons the app bundles, sorted: gpui-kit's Lucide set, and the app's
/// own in `assets/icons` (more of Lucide, and `agent-*` and `brand-*` logos).
pub const ICONS: &[&str] = &[
    "a-large-small",
    "activity",
    "agent-amp",
    "agent-claude",
    "agent-codex",
    "agent-copilot",
    "agent-cursor",
    "agent-droid",
    "agent-gemini",
    "agent-grok",
    "agent-kiro",
    "agent-opencode",
    "agent-qwen",
    "archive",
    "arrow-down",
    "arrow-left",
    "arrow-right",
    "arrow-up",
    "arrow-up-down",
    "arrow-up-right",
    "asterisk",
    "at-sign",
    "ban",
    "battery",
    "battery-charging",
    "battery-full",
    "battery-low",
    "battery-medium",
    "battery-warning",
    "bell",
    "bell-off",
    "bold",
    "book-open",
    "bookmark",
    "bot",
    "brain",
    "brand-anthropic",
    "brand-github",
    "brand-gitlab",
    "brand-google-scholar",
    "brand-linkedin",
    "brand-mistral",
    "brand-ollama",
    "brand-openai",
    "brand-slack",
    "brand-wikipedia",
    "brand-x",
    "bug",
    "building-2",
    "calendar",
    "calendar-check",
    "calendar-days",
    "case-sensitive",
    "chart-column",
    "chart-gantt",
    "chart-line",
    "chart-pie",
    "check",
    "chevron-down",
    "chevron-left",
    "chevron-right",
    "chevron-up",
    "chevrons-left",
    "chevrons-right",
    "chevrons-up-down",
    "circle",
    "circle-alert",
    "circle-check",
    "circle-dashed",
    "circle-dot",
    "circle-help",
    "circle-pause",
    "circle-play",
    "circle-question-mark",
    "circle-stop",
    "circle-user",
    "circle-x",
    "clock",
    "close",
    "cloud",
    "cloud-off",
    "code",
    "coffee",
    "columns-2",
    "command",
    "copy",
    "cpu",
    "crop",
    "dash",
    "database",
    "delete",
    "diamond",
    "dot",
    "download",
    "ellipsis",
    "ellipsis-vertical",
    "external-link",
    "eye",
    "eye-off",
    "file",
    "file-archive",
    "file-text",
    "filter",
    "flag",
    "folder",
    "folder-closed",
    "folder-git-2",
    "folder-open",
    "frame",
    "funnel",
    "gallery-vertical-end",
    "git-branch",
    "git-commit-horizontal",
    "git-compare",
    "git-merge",
    "git-pull-request",
    "git-pull-request-closed",
    "git-pull-request-draft",
    "github",
    "gitlab",
    "globe",
    "grip-vertical",
    "hard-drive",
    "hash",
    "heading",
    "headphones",
    "heart",
    "heart-off",
    "history",
    "house",
    "image",
    "inbox",
    "info",
    "inspector",
    "italic",
    "key-round",
    "keyboard",
    "laptop",
    "layers",
    "layout-dashboard",
    "layout-grid",
    "link",
    "list",
    "list-checks",
    "list-ordered",
    "list-tree",
    "loader",
    "loader-circle",
    "lock",
    "lock-open",
    "log-out",
    "mail",
    "mail-open",
    "mails",
    "map",
    "maximize",
    "maximize-2",
    "memory-stick",
    "menu",
    "message-square",
    "messages-square",
    "mic",
    "minimize",
    "minimize-2",
    "minus",
    "monitor",
    "moon",
    "network",
    "octagon-alert",
    "palette",
    "panel-bottom",
    "panel-bottom-open",
    "panel-left",
    "panel-left-close",
    "panel-left-open",
    "panel-right",
    "panel-right-close",
    "panel-right-open",
    "paperclip",
    "pause",
    "pencil",
    "pin",
    "pin-off",
    "pipette",
    "play",
    "plug",
    "plus",
    "power",
    "redo",
    "redo-2",
    "refresh-ccw",
    "refresh-cw",
    "repeat",
    "replace",
    "reply",
    "resize-corner",
    "rocket",
    "rotate-ccw",
    "rotate-ccw-clock",
    "rotate-cw",
    "rows-2",
    "scan",
    "search",
    "send",
    "server",
    "settings",
    "settings-2",
    "share",
    "shield",
    "sliders-horizontal",
    "smile-plus",
    "sort-ascending",
    "sort-descending",
    "sparkles",
    "spline",
    "split-square-horizontal",
    "split-square-vertical",
    "square",
    "square-check",
    "square-pen",
    "square-split-horizontal",
    "square-split-vertical",
    "square-terminal",
    "star",
    "star-fill",
    "star-off",
    "strikethrough",
    "sun",
    "tag",
    "tags",
    "terminal",
    "text-quote",
    "thumbs-down",
    "thumbs-up",
    "timer",
    "trash",
    "trash-2",
    "triangle-alert",
    "undo",
    "undo-2",
    "unlock",
    "upload",
    "user",
    "user-plus",
    "users",
    "wand-sparkles",
    "wifi",
    "wifi-off",
    "window-close",
    "window-maximize",
    "window-minimize",
    "window-restore",
    "workflow",
    "zap",
    "zoom-in",
    "zoom-out",
];
