//! `ui.icon`'s SVGs as Rune icons. The app's own come from `assets/icons`;
//! the Lucide ones the GPUI build takes from gpui-kit are vendored in
//! `icons/`, as that crate links GPUI.

use {
    moonframe_core::IconName,
    rune::text::Icon,
    std::{borrow::Cow, cell::RefCell, collections::HashMap},
};

#[derive(rust_embed::RustEmbed)]
#[folder = "../../assets/icons"]
#[include = "*.svg"]
struct Ours;

#[derive(rust_embed::RustEmbed)]
#[folder = "icons"]
#[include = "*.svg"]
struct Lucide;

/// What Lucide sets once on `<svg>`; Rune reads these on each shape.
const INHERITED: &[&str] = &[
    "fill",
    "stroke",
    "stroke-width",
    "stroke-linecap",
    "stroke-linejoin",
];
const SHAPES: &[&str] = &["path", "rect", "circle", "ellipse", "line"];

thread_local! {
    /// Rune keeps `&'static` icons, so each name is parsed and leaked once;
    /// there are as many as `moonframe_core::ICONS`.
    static PARSED: RefCell<HashMap<&'static str, Option<&'static Icon>>> = RefCell::default();
}

fn source(file: &str) -> Option<Cow<'static, [u8]>> {
    Ours::get(file)
        .or_else(|| Lucide::get(file))
        .map(|found| found.data)
}

/// The value of `name` in the tag `tag`.
fn attribute<'a>(tag: &'a str, name: &str) -> Option<&'a str> {
    let at = tag.find(&format!("{name}=\""))? + name.len() + 2;
    let value = &tag[at..];
    Some(&value[..value.find('"')?])
}

/// The shapes inside `svg`, each carrying what the root set for them.
fn body(svg: &str) -> Option<(f32, String)> {
    let start = svg.find("<svg")?;
    let end = start + svg[start..].find('>')?;
    let root = &svg[start..end];
    let close = svg.rfind("</svg")?;
    let size = attribute(root, "viewBox")
        .and_then(|view| view.split_whitespace().nth(2)?.parse().ok())
        .unwrap_or(24.);
    let carried: String = INHERITED
        .iter()
        .filter_map(|name| Some(format!(" {name}=\"{}\"", attribute(root, name)?)))
        .collect();
    let mut shapes = svg[end + 1..close].to_owned();
    for shape in SHAPES {
        shapes = shapes.replace(&format!("<{shape} "), &format!("<{shape}{carried} "));
    }
    Some((size, shapes))
}

/// The icon `name`, parsed on first use; `None` when no SVG has it.
pub(crate) fn icon(name: &IconName) -> Option<&'static Icon> {
    let key = name.name();
    PARSED.with_borrow_mut(|parsed| {
        *parsed.entry(key).or_insert_with(|| {
            let data = source(&format!("{key}.svg"))?;
            let (view, shapes) = body(std::str::from_utf8(&data).ok()?)?;
            Some(&*Box::leak(Box::new(Icon {
                name: key,
                view,
                body: Box::leak(shapes.into_boxed_str()),
            })))
        })
    })
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn every_icon_name_has_an_svg() {
        let missing: Vec<&str> = moonframe_core::ICONS
            .iter()
            .copied()
            .filter(|name| source(&format!("{name}.svg")).is_none())
            .collect();
        assert!(missing.is_empty(), "no SVG for {missing:?}");
    }

    #[test]
    fn shapes_carry_what_the_root_set() {
        let svg = r#"<svg viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="2"><path d="M1 1"/></svg>"#;
        let (view, shapes) = body(svg).unwrap();
        assert!((view - 16.).abs() < f32::EPSILON);
        assert!(shapes.contains(r#"<path fill="none" stroke="currentColor" stroke-width="2" d="#));
    }
}
