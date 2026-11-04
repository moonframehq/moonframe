//! The native view a `ui.video` shows in: a `CAMetalLayer` in a view above
//! the window's drawing, as `ui.browser`'s page is. It takes no events,
//! so the pointer and the wheel reach the window's own elements under it.

#![allow(
    unsafe_code,
    reason = "AppKit and Core Graphics calls with no safe binding; see each SAFETY"
)]

use {
    super::render::LAYER_FORMAT,
    core_graphics_types::geometry::CGSize,
    gpui_kit::{Bounds, Pixels, Window},
    metal::{MetalLayer, foreign_types::ForeignType as _},
    moonframe_core::VideoColor,
    objc2::{
        MainThreadMarker, MainThreadOnly, define_class, msg_send, rc::Retained, runtime::AnyObject,
    },
    objc2_app_kit::{NSAutoresizingMaskOptions, NSView},
    objc2_foundation::{NSPoint, NSRect, NSSize},
    raw_window_handle::{HasWindowHandle, RawWindowHandle},
    std::{cell::Cell, ffi::c_void, ptr},
};

define_class!(
    /// A view events pass through.
    #[unsafe(super(NSView))]
    #[thread_kind = MainThreadOnly]
    #[name = "MoonframeVideoView"]
    struct PassThrough;

    impl PassThrough {
        #[unsafe(method(hitTest:))]
        fn hit_test(&self, _point: NSPoint) -> *mut NSView {
            ptr::null_mut()
        }
    }
);

impl PassThrough {
    fn make(mtm: MainThreadMarker) -> Retained<Self> {
        let this = Self::alloc(mtm).set_ivars(());
        // SAFETY: `initWithFrame:` is NSView's designated initialiser.
        unsafe { msg_send![super(this), initWithFrame: NSRect::ZERO] }
    }
}

#[link(name = "CoreGraphics", kind = "framework")]
unsafe extern "C" {
    static kCGColorSpaceSRGB: *const c_void;
    static kCGColorSpaceDisplayP3: *const c_void;
    static kCGColorSpaceITUR_709: *const c_void;
    static kCGColorSpaceITUR_2020: *const c_void;
    static kCGColorSpaceLinearSRGB: *const c_void;
    static kCGColorSpaceLinearDisplayP3: *const c_void;
    static kCGColorSpaceExtendedSRGB: *const c_void;
    static kCGColorSpaceExtendedDisplayP3: *const c_void;
    static kCGColorSpaceExtendedLinearSRGB: *const c_void;
    static kCGColorSpaceExtendedLinearDisplayP3: *const c_void;
    fn CGColorSpaceCreateWithName(name: *const c_void) -> *mut c_void;
    fn CGColorSpaceRelease(space: *mut c_void);
}

pub(super) struct Surface {
    clip: Retained<PassThrough>,
    view: Retained<PassThrough>,
    layer: MetalLayer,
    shown: Cell<bool>,
    color: Cell<Option<(VideoColor, bool)>>,
    placed: Cell<Option<(Bounds<Pixels>, Bounds<Pixels>, f32)>>,
}

impl Surface {
    /// Adds a hidden surface to `window`.
    pub(super) fn new(window: &Window, device: &metal::DeviceRef) -> Result<Self, String> {
        let mtm = MainThreadMarker::new().ok_or("not on the main thread")?;
        let handle = HasWindowHandle::window_handle(window).map_err(|error| error.to_string())?;
        let RawWindowHandle::AppKit(handle) = handle.as_raw() else {
            return Err("not an AppKit window".to_owned());
        };
        // SAFETY: the handle's view is the window's, alive as `window` is.
        let parent: &NSView = unsafe { handle.ns_view.cast::<NSView>().as_ref() };
        let layer = MetalLayer::new();
        layer.set_device(device);
        layer.set_pixel_format(LAYER_FORMAT);
        layer.set_opaque(true);
        layer.set_framebuffer_only(true);
        layer.set_maximum_drawable_count(3);
        layer.set_presents_with_transaction(false);
        let clip = PassThrough::make(mtm);
        clip.setClipsToBounds(true);
        clip.setHidden(true);
        // Pinned to the window's top as it resizes, as a page is.
        clip.setAutoresizingMask(NSAutoresizingMaskOptions::ViewMinYMargin);
        let view = PassThrough::make(mtm);
        // SAFETY: a layer-hosting view takes its layer before wanting one;
        // the view keeps the layer it is given.
        unsafe {
            let raw = layer.as_ptr().cast::<AnyObject>();
            let _: () = msg_send![&*view, setLayer: raw];
            let _: () = msg_send![&*view, setWantsLayer: true];
        }
        parent.addSubview(&clip);
        clip.addSubview(&view);
        Ok(Self {
            clip,
            view,
            layer,
            shown: Cell::new(false),
            color: Cell::new(None),
            placed: Cell::new(None),
        })
    }

    /// The layer, for the thread that draws into it.
    pub(super) fn layer(&self) -> MetalLayer {
        self.layer.clone()
    }

    /// Tells the system what the pixels are: it matches them to each
    /// screen. No colour space means no matching at all.
    pub(super) fn set_color(&self, color: VideoColor, extended: bool) {
        if self.color.get() == Some((color, extended)) {
            return;
        }
        self.color.set(Some((color, extended)));
        // SAFETY: the names are constants; a layer keeps its own reference
        // to what it is given, so the one made here is released after.
        unsafe {
            let name = match (color, extended) {
                (VideoColor::None, _) => None,
                (VideoColor::Srgb | VideoColor::Rec709, true) => Some(kCGColorSpaceExtendedSRGB),
                (VideoColor::DisplayP3, true) => Some(kCGColorSpaceExtendedDisplayP3),
                (VideoColor::LinearSrgb, true) => Some(kCGColorSpaceExtendedLinearSRGB),
                (VideoColor::LinearP3, true) => Some(kCGColorSpaceExtendedLinearDisplayP3),
                (VideoColor::Srgb, false) => Some(kCGColorSpaceSRGB),
                (VideoColor::DisplayP3, false) => Some(kCGColorSpaceDisplayP3),
                (VideoColor::Rec709, false) => Some(kCGColorSpaceITUR_709),
                (VideoColor::Rec2020, _) => Some(kCGColorSpaceITUR_2020),
                (VideoColor::LinearSrgb, false) => Some(kCGColorSpaceLinearSRGB),
                (VideoColor::LinearP3, false) => Some(kCGColorSpaceLinearDisplayP3),
            };
            let space = name.map_or(ptr::null_mut(), |name| CGColorSpaceCreateWithName(name));
            let raw = self.layer.as_ptr().cast::<AnyObject>();
            let _: () = msg_send![raw, setColorspace: space];
            if !space.is_null() {
                CGColorSpaceRelease(space);
            }
        }
        self.layer
            .set_wants_extended_dynamic_range_content(extended);
    }

    /// Puts the picture at `bounds`, showing what falls in `visible`, in
    /// the window's points from its top left; whether anything changed.
    pub(super) fn place(
        &self,
        bounds: Bounds<Pixels>,
        visible: Bounds<Pixels>,
        scale: f32,
    ) -> bool {
        if self.placed.get() == Some((bounds, visible, scale)) {
            return false;
        }
        self.placed.set(Some((bounds, visible, scale)));
        // SAFETY: a property read on this view, on the main thread.
        let Some(parent) = (unsafe { self.clip.superview() }) else {
            return false;
        };
        let (width, height) = (
            f64::from(visible.size.width),
            f64::from(visible.size.height),
        );
        let top = f64::from(visible.origin.y);
        let y = if parent.isFlipped() {
            top
        } else {
            parent.bounds().size.height - top - height
        };
        self.clip.setFrame(NSRect::new(
            NSPoint::new(f64::from(visible.origin.x), y),
            NSSize::new(width, height),
        ));
        // Within the clip, which isn't flipped: from its bottom left.
        let (full_width, full_height) =
            (f64::from(bounds.size.width), f64::from(bounds.size.height));
        let below = f64::from(bounds.origin.y) + full_height - f64::from(visible.origin.y) - height;
        self.view.setFrame(NSRect::new(
            NSPoint::new(
                f64::from(bounds.origin.x) - f64::from(visible.origin.x),
                -below,
            ),
            NSSize::new(full_width, full_height),
        ));
        self.layer.set_contents_scale(f64::from(scale));
        self.layer.set_drawable_size(core_graphics_size(
            full_width * f64::from(scale),
            full_height * f64::from(scale),
        ));
        true
    }

    pub(super) fn show(&self, shown: bool) {
        if self.shown.replace(shown) != shown {
            self.clip.setHidden(!shown);
        }
    }
}

fn core_graphics_size(width: f64, height: f64) -> CGSize {
    CGSize::new(width.max(1.), height.max(1.))
}

impl Drop for Surface {
    fn drop(&mut self) {
        self.clip.removeFromSuperview();
    }
}
