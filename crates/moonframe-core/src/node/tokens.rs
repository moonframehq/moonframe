//! The named values a view's keys take: element types, theme roles,
//! layout and placement, and the commands controls run.

tokens! {
    /// The element types `ui` builds; a table's `type` names one.
    Element {
        Row = "row",
        Column = "column",
        Panel = "panel",
        Hsplit = "hsplit",
        Vsplit = "vsplit",
        Pane = "pane",
        Text = "text",
        Button = "button",
        Item = "item",
        Badge = "badge",
        Icon = "icon",
        Image = "image",
        Line = "line",
        Paths = "paths",
        Hatch = "hatch",
        Terminal = "terminal",
        Browser = "browser",
        View = "view",
        Input = "input",
        Markdown = "markdown",
        Divider = "divider",
        Spacer = "spacer",
        Region = "region",
        Mount = "mount",
        Instance = "instance",
        Popover = "popover",
        Overlay = "overlay",
        List = "list",
    }
}

tokens! {
    /// A theme role for text and badges.
    Tone {
        Normal = "normal",
        Muted = "muted",
        Accent = "accent",
        Success = "success",
        Warning = "warning",
        Danger = "danger",
    }
}

tokens! {
    TextSize {
        Small = "small",
        Normal = "normal",
        Large = "large",
        Title = "title",
        Headline = "headline",
        Display = "display",
        Hero = "hero",
    }
}

tokens! {
    Weight {
        Light = "light",
        Normal = "normal",
        Medium = "medium",
        Semibold = "semibold",
        Bold = "bold",
    }
}

tokens! {
    /// A step on the app's spacing scale, for gaps and padding.
    Space {
        None = "none",
        Small = "small",
        Normal = "normal",
        Large = "large",
    }
}

impl Space {
    pub fn pixels(self) -> f32 {
        match self {
            Self::None => 0.,
            Self::Small => 4.,
            Self::Normal => 8.,
            Self::Large => 16.,
        }
    }
}

tokens! {
    /// Where children sit across a container's axis.
    Align {
        Stretch = "stretch",
        Start = "start",
        Center = "center",
        End = "end",
    }
}

tokens! {
    /// How children share a container's axis.
    Justify {
        Start = "start",
        Center = "center",
        End = "end",
        Between = "between",
    }
}

tokens! {
    /// A theme background.
    Surface {
        Background = "background",
        Sidebar = "sidebar",
        Muted = "muted",
        Secondary = "secondary",
    }
}

tokens! {
    /// The edges that get a border; `border = true` is all of them.
    Border {
        None = "none",
        All = "all",
        Top = "top",
        Bottom = "bottom",
        Left = "left",
        Right = "right",
    }
}

tokens! {
    Direction {
        Row = "row",
        Column = "column",
    }
}

tokens! {
    Variant {
        Normal = "normal",
        Primary = "primary",
        Ghost = "ghost",
        Outline = "outline",
        Danger = "danger",
    }
}

tokens! {
    /// Which side of its anchor a popover opens on.
    Side {
        Top = "top",
        Bottom = "bottom",
        Left = "left",
        Right = "right",
    }
}

tokens! {
    /// Which edge of its anchor a popover lines up with: the left or right
    /// one above or below it, the top or bottom one beside it.
    Anchor {
        Start = "start",
        End = "end",
    }
}

tokens! {
    /// Where an overlay sits in the window.
    Placement {
        /// In the middle: a dialog or a command palette.
        Center = "center",
        /// Centered across, hanging from near the top: a palette.
        Top = "top",
        /// Sheets along an edge, as wide or as tall as the window.
        Bottom = "bottom",
        Left = "left",
        Right = "right",
        /// In a corner: toasts and notices.
        TopStart = "top-start",
        TopEnd = "top-end",
        BottomStart = "bottom-start",
        BottomEnd = "bottom-end",
    }
}

tokens! {
    /// What the app does itself, rather than a module's `update`.
    Command {
        Quit = "quit",
        Reload = "reload",
        ToggleTheme = "toggle-theme",
        /// Minimizes the window, for a title bar the app draws itself.
        Minimize = "minimize",
        /// Zooms (macOS) or maximizes the window, or restores it.
        Maximize = "maximize",
        CloseWindow = "close-window",
        ShowThemes = "show-themes",
        /// Starts Inspect Element for the mod that asked: the pick, or
        /// Escape, comes back to it on the topic `app/inspect`.
        Inspect = "inspect",
        /// Asks the app's GitHub repository for a newer release, as the
        /// app menu's Check for Updates does.
        CheckForUpdates = "check-for-updates",
    }
}

tokens! {
    /// What a control can ask of a `ui.browser`, named by its `browser` key.
    BrowserCommand {
        Back = "browser-back",
        Forward = "browser-forward",
        Reload = "browser-reload",
        Stop = "browser-stop",
        /// Opens the page's address in the system browser.
        Open = "browser-open",
    }
}
