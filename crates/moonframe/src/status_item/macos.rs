//! Menu bar items with AppKit: an `NSStatusItem` per mod, its menu's lines
//! calling one shared target. Each line carries what it sends (its
//! represented object), so a menu still open while the mod replaces it
//! sends what it showed.

use {
    super::{Icon, Line, Look},
    moonframe_core::{Chord, Data},
    objc2::{
        AnyThread as _, DefinedClass as _, MainThreadMarker, MainThreadOnly as _, define_class,
        msg_send,
        rc::Retained,
        runtime::{NSObject, NSObjectProtocol},
        sel,
    },
    objc2_app_kit::{
        NSCellImagePosition, NSControlStateValueOff, NSControlStateValueOn, NSEventModifierFlags,
        NSImage, NSMenu, NSMenuItem, NSStatusBar, NSStatusItem, NSVariableStatusItemLength,
    },
    objc2_foundation::{NSData, NSPoint, NSSize, NSString},
    std::{cell::RefCell, collections::BTreeMap},
};

/// How tall an icon stands in the menu bar, in points.
const ICON_HEIGHT: f64 = 18.0;

thread_local! {
    /// Each mod's item.
    static ITEMS: RefCell<BTreeMap<String, Retained<NSStatusItem>>> =
        const { RefCell::new(BTreeMap::new()) };
    static TARGET: RefCell<Option<Retained<Target>>> = const { RefCell::new(None) };
}

fn main_thread() -> Result<MainThreadMarker, String> {
    MainThreadMarker::new()
        .ok_or_else(|| "the menu bar is changed from the main thread only".to_owned())
}

pub(super) fn show(owner: &str, look: Look) -> Result<(), String> {
    let main = main_thread()?;
    let image = look
        .icon
        .as_ref()
        .map(|icon| image(icon, look.template))
        .transpose()?;
    let item = ITEMS.with_borrow(|items| items.get(owner).cloned());
    let item = item.unwrap_or_else(|| {
        NSStatusBar::systemStatusBar().statusItemWithLength(NSVariableStatusItemLength)
    });
    if let Some(button) = item.button(main) {
        button.setImage(image.as_deref());
        button.setTitle(&NSString::from_str(look.title.as_deref().unwrap_or("")));
        button.setImagePosition(if look.title.is_some() {
            NSCellImagePosition::ImageLeft
        } else {
            NSCellImagePosition::ImageOnly
        });
        button.setToolTip(look.tooltip.as_deref().map(NSString::from_str).as_deref());
    }
    let menu = (!look.menu.is_empty()).then(|| menu(&look.menu, main));
    item.setMenu(menu.as_deref());
    ITEMS.with_borrow_mut(|items| items.insert(owner.to_owned(), item));
    Ok(())
}

pub(super) fn popup(owner: &str, lines: Vec<Line>) -> Result<(), String> {
    let main = main_thread()?;
    let item = ITEMS
        .with_borrow(|items| items.get(owner).cloned())
        .ok_or("`popup` needs the mod's item shown first")?;
    let button = item
        .button(main)
        .ok_or("the item has no button to drop a menu from")?;
    let below = NSPoint::new(0.0, button.bounds().size.height + 4.0);
    // Tracks the menu until the user picks a line or closes it.
    menu(&lines, main).popUpMenuPositioningItem_atLocation_inView(None, below, Some(&button));
    Ok(())
}

pub(super) fn remove(owner: &str) {
    let removed = ITEMS.with_borrow_mut(|items| items.remove(owner));
    if let Some(removed) = removed {
        NSStatusBar::systemStatusBar().removeStatusItem(&removed);
    }
}

pub(super) fn owners() -> Vec<String> {
    ITEMS.with_borrow(|items| items.keys().cloned().collect())
}

/// The icon as AppKit draws it, standing [`ICON_HEIGHT`] tall.
fn image(icon: &Icon, template: bool) -> Result<Retained<NSImage>, String> {
    let image = match icon {
        Icon::Symbol(name) => NSImage::imageWithSystemSymbolName_accessibilityDescription(
            &NSString::from_str(name),
            None,
        )
        .ok_or_else(|| format!("no system symbol `{name}`"))?,
        Icon::Image(bytes) => {
            let image = NSImage::initWithData(NSImage::alloc(), &NSData::with_bytes(bytes))
                .ok_or("not an image AppKit reads")?;
            let size = image.size();
            if size.height > 0.0 {
                image.setSize(NSSize::new(
                    size.width * ICON_HEIGHT / size.height,
                    ICON_HEIGHT,
                ));
            }
            image
        },
    };
    image.setTemplate(template);
    Ok(image)
}

/// The menu for `lines`.
#[allow(
    unsafe_code,
    reason = "AppKit's menu item initializer, target and object; see SAFETY"
)]
fn menu(lines: &[Line], main: MainThreadMarker) -> Retained<NSMenu> {
    let menu = NSMenu::new(main);
    menu.setAutoenablesItems(false);
    let target = target(main);
    for line in lines {
        let Line::Item {
            label,
            action,
            enabled,
            checked,
            shortcut,
        } = line
        else {
            menu.addItem(&NSMenuItem::separatorItem(main));
            continue;
        };
        let (key, modifiers) = shortcut.as_ref().map_or_else(
            || (String::new(), NSEventModifierFlags::empty()),
            key_equivalent,
        );
        // SAFETY: a fresh item, on the main thread; the action names the
        // target's own method, which takes the item as its sender.
        let item = unsafe {
            NSMenuItem::initWithTitle_action_keyEquivalent(
                NSMenuItem::alloc(main),
                &NSString::from_str(label),
                action.as_ref().map(|_| sel!(clicked:)),
                &NSString::from_str(&key),
            )
        };
        item.setKeyEquivalentModifierMask(modifiers);
        item.setEnabled(*enabled && action.is_some());
        item.setState(if *checked {
            NSControlStateValueOn
        } else {
            NSControlStateValueOff
        });
        if let Some((to, msg)) = action {
            // SAFETY: the target lives as long as the app (`TARGET`), and
            // a menu item holds its target weakly. The object is only ever
            // read back as an `Action`, by the target.
            unsafe {
                item.setTarget(Some(&target));
                item.setRepresentedObject(Some(&Action::new(to.clone(), msg.clone())));
            }
        }
        menu.addItem(&item);
    }
    menu
}

/// What AppKit shows for `chord`: the key's character, and its modifiers.
fn key_equivalent(chord: &Chord) -> (String, NSEventModifierFlags) {
    let function = |code: u32| char::from_u32(code).map(String::from).unwrap_or_default();
    let key = match chord.key.as_str() {
        "space" => " ".to_owned(),
        "enter" => "\r".to_owned(),
        "tab" => "\t".to_owned(),
        "escape" => "\u{1b}".to_owned(),
        "backspace" => "\u{8}".to_owned(),
        "delete" => function(0xF728),
        "insert" => function(0xF727),
        "up" => function(0xF700),
        "down" => function(0xF701),
        "left" => function(0xF702),
        "right" => function(0xF703),
        "home" => function(0xF729),
        "end" => function(0xF72B),
        "pageup" => function(0xF72C),
        "pagedown" => function(0xF72D),
        key => match key.strip_prefix('f').and_then(|n| n.parse::<u32>().ok()) {
            Some(number) => function(0xF704 + number - 1),
            None => key.to_owned(),
        },
    };
    let mut modifiers = NSEventModifierFlags::empty();
    for (on, flag) in [
        (chord.ctrl, NSEventModifierFlags::Control),
        (chord.alt, NSEventModifierFlags::Option),
        (chord.shift, NSEventModifierFlags::Shift),
        (chord.cmd || chord.secondary, NSEventModifierFlags::Command),
    ] {
        if on {
            modifiers |= flag;
        }
    }
    (key, modifiers)
}

fn target(main: MainThreadMarker) -> Retained<Target> {
    TARGET.with_borrow_mut(|target| target.get_or_insert_with(|| Target::new(main)).clone())
}

define_class!(
    // SAFETY: NSObject has no subclassing requirements, and the target
    // implements no `Drop`.
    #[unsafe(super(NSObject))]
    #[thread_kind = objc2::MainThreadOnly]
    #[name = "MoonframeStatusItemTarget"]
    struct Target;

    unsafe impl NSObjectProtocol for Target {}

    impl Target {
        /// A line was picked: sends what it carries.
        #[unsafe(method(clicked:))]
        fn clicked(&self, sender: &NSMenuItem) {
            let object = sender.representedObject();
            let action = object
                .as_deref()
                .and_then(|object| object.downcast_ref::<Action>());
            if let Some(action) = action {
                let (to, msg) = action.ivars();
                super::clicked(to.clone(), msg.clone());
            }
        }
    }
);

impl Target {
    #[allow(unsafe_code, reason = "NSObject's init; see SAFETY")]
    fn new(main: MainThreadMarker) -> Retained<Self> {
        let this = Self::alloc(main).set_ivars(());
        // SAFETY: NSObject's `init`, on a freshly allocated object.
        unsafe { msg_send![super(this), init] }
    }
}

define_class!(
    // SAFETY: NSObject has no subclassing requirements, and the action
    // implements no `Drop`.
    #[unsafe(super(NSObject))]
    #[name = "MoonframeStatusItemAction"]
    #[ivars = (String, Data)]
    struct Action;

    unsafe impl NSObjectProtocol for Action {}
);

impl Action {
    /// What a line sends: `msg` for the mod `to`.
    #[allow(unsafe_code, reason = "NSObject's init; see SAFETY")]
    fn new(to: String, msg: Data) -> Retained<Self> {
        let this = Self::alloc().set_ivars((to, msg));
        // SAFETY: NSObject's `init`, on a freshly allocated object.
        unsafe { msg_send![super(this), init] }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn a_shortcut_shows_as_appkit_writes_it() {
        let chord = |name: &str| Chord::parse(name).unwrap_or_else(|problem| panic!("{problem}"));
        let (key, modifiers) = key_equivalent(&chord("ctrl-shift-space"));
        assert_eq!(key, " ");
        assert_eq!(
            modifiers,
            NSEventModifierFlags::Control | NSEventModifierFlags::Shift
        );
        let (key, modifiers) = key_equivalent(&chord("secondary-k"));
        assert_eq!(
            (key.as_str(), modifiers),
            ("k", NSEventModifierFlags::Command)
        );
        assert_eq!(key_equivalent(&chord("f5")).0, "\u{F708}");
        // A named key is the one character AppKit has for it.
        assert_eq!(key_equivalent(&chord("ctrl-insert")).0, "\u{F727}");
        assert_eq!(key_equivalent(&chord("ctrl-delete")).0, "\u{F728}");
        assert_eq!(key_equivalent(&chord("ctrl-backspace")).0, "\u{8}");
    }

    #[test]
    fn a_line_carries_what_it_sends() {
        let action = Action::new("capture".to_owned(), Data::from("go"));
        let object: &objc2::runtime::AnyObject = &action;
        let (to, msg) = object
            .downcast_ref::<Action>()
            .map(|action| action.ivars().clone())
            .unwrap_or_default();
        assert_eq!((to.as_str(), msg), ("capture", Data::from("go")));
    }
}
