//! The page's icons, and the sprite they are drawn from.
//!
//! Each icon is vendored under `assets/` exactly as it is published, so
//! the file on disk can be diffed against the upstream download with
//! nothing to discount. That leaves two things about those files which
//! do not suit the page: each is a whole `<svg>` document rather than
//! something `<use>` can draw, and each fixes `width` and `height` at
//! 16, which would pin every icon to that size whatever the stylesheet
//! asks for. [`symbol`] rewrites one into a `<symbol>`, keeping only
//! the `viewBox` and the drawing itself, and [`sprite`] collects them
//! into the one hidden `<svg>` that [`crate::render`] emits.
//!
//! The rewrite reads the file with [`quick_xml`], already in this
//! binary by way of `syntect`'s theme loading. The alternative was a
//! hand-rolled scanner, which for the XML these files use would have
//! spent most of its length on attribute values that may contain `>`.

use quick_xml::Reader;
use quick_xml::events::Event;

/// One icon: the `id` its `<symbol>` is given, which [`crate::render`]
/// points a `<use>` at, and the vendored document it is built from.
struct Icon {
    id: &'static str,
    /// The upstream file, verbatim. Its name under `assets/` says what
    /// the icon is for; [`UPSTREAM_NOTICE`] says where it came from.
    source: &'static str,
}

/// Every icon the page draws. Adding one here is what puts its
/// `<symbol>` in the sprite; nothing else enumerates them.
const ICONS: &[Icon] = &[
    Icon {
        id: "icon-search",
        source: include_str!("assets/search.svg"),
    },
    Icon {
        id: "icon-filter",
        source: include_str!("assets/filter.svg"),
    },
    Icon {
        id: "icon-theme-light",
        source: include_str!("assets/theme-light.svg"),
    },
    Icon {
        id: "icon-theme-dark",
        source: include_str!("assets/theme-dark.svg"),
    },
    Icon {
        id: "icon-theme-system",
        source: include_str!("assets/theme-system.svg"),
    },
    Icon {
        id: "icon-rule-anchor",
        source: include_str!("assets/rule-anchor.svg"),
    },
];

/// Shipped inside the sprite, because the MIT licence the icons carry
/// asks for its notice to travel with every copy of them and the
/// rendered page is one such copy. The files under `assets/` hold no
/// notice of their own, being verbatim copies of files that hold none
/// either; this is the one place it is stated.
const UPSTREAM_NOTICE: &str = "<!-- GitHub Octicons, MIT License, Copyright (c) GitHub Inc. \
     https://github.com/primer/octicons/blob/main/LICENSE -->";

/// Rewrite one standalone `<svg>` document as a `<symbol>` under `id`.
///
/// Everything the root element carries is dropped but its `viewBox`,
/// which is what lets a `<use>` scale the drawing to whatever size the
/// stylesheet gives it. Dropping the rest is the point: `width` and
/// `height` would override that size, and a `fill` would override the
/// `currentColor` the icons are themed through.
///
/// The drawing is taken as the source text between the root element's
/// tags rather than rebuilt from parsed events, so what reaches the
/// page is the upstream bytes.
///
/// Returns why it could not, rather than a broken symbol: an icon that
/// silently draws nothing is the failure this is most likely to cause,
/// and it is invisible in a diff.
fn symbol(id: &str, source: &str) -> Result<String, String> {
    let mut reader = Reader::from_str(source);
    let view_box = loop {
        match reader.read_event() {
            Ok(Event::Start(root)) if root.local_name().as_ref() == b"svg" => {
                let attribute = root
                    .try_get_attribute("viewBox")
                    .map_err(|error| format!("{id}: reading the root <svg>: {error}"))?
                    .ok_or_else(|| format!("{id}: the root <svg> has no viewBox"))?;
                // The raw value, not the unescaped one: it goes straight
                // back into an attribute, where it is already spelled
                // the way it needs to be.
                break String::from_utf8(attribute.value.into_owned())
                    .map_err(|error| format!("{id}: the viewBox is not UTF-8: {error}"))?;
            }
            Ok(Event::Eof) => return Err(format!("{id}: no root <svg>")),
            Ok(_) => {}
            Err(error) => return Err(format!("{id}: {error}")),
        }
    };

    let start = reader.buffer_position() as usize;
    let mut depth = 0usize;
    let end = loop {
        // Before the event, so that the `</svg>` that ends the loop
        // reports where it begins rather than where it ends.
        let position = reader.buffer_position() as usize;
        match reader.read_event() {
            Ok(Event::Start(_)) => depth += 1,
            Ok(Event::End(_)) => match depth.checked_sub(1) {
                Some(remaining) => depth = remaining,
                None => break position,
            },
            Ok(Event::Eof) => return Err(format!("{id}: the root <svg> is never closed")),
            Ok(_) => {}
            Err(error) => return Err(format!("{id}: {error}")),
        }
    };

    let drawing = source
        .get(start..end)
        .ok_or_else(|| format!("{id}: the root <svg>'s content is not where the parser said"))?
        .trim();
    Ok(format!(
        r#"<symbol id="{id}" viewBox="{view_box}">{drawing}</symbol>"#,
    ))
}

/// The hidden `<svg>` holding every icon's `<symbol>`, ready to be
/// written into the page.
///
/// Panics if an icon cannot be rewritten, which fails the build: the
/// alternative is a page that renders with an icon missing, and
/// `gen-docs` treats every other unusable asset the same way.
pub(crate) fn sprite() -> String {
    let symbols = ICONS
        .iter()
        .map(|icon| {
            symbol(icon.id, icon.source).unwrap_or_else(|error| panic!("bad icon -- {error}"))
        })
        .collect::<String>();
    format!(r#"<svg xmlns="http://www.w3.org/2000/svg" hidden>{UPSTREAM_NOTICE}{symbols}</svg>"#)
}

#[cfg(test)]
mod tests;
