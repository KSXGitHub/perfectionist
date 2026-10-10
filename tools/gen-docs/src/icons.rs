//! The page's icons: one `<symbol>` per file under `assets/`, gathered
//! into the sprite [`crate::render`] emits.
//!
//! Those files are left as they are, licence notice and all. What they
//! carry besides the drawing — a fixed `width` and `height`, a `fill` —
//! would override the stylesheet, so only the `viewBox` is kept.
//!
//! [`quick_xml`] reads them, being already here by way of `syntect`'s
//! theme loading. A hand-rolled scanner would have spent most of its
//! length on attribute values that may contain `>`.

use pipe_trait::Pipe;
use quick_xml::Reader;
use quick_xml::events::Event;

/// One icon: the `id` a `<use>` points at, and the file behind it.
struct Icon {
    id: &'static str,
    source: &'static str,
}

/// Every icon the page draws.
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

/// Rewrite one `<svg>` document as a `<symbol>` under `id`.
///
/// The drawing is sliced out of `source` rather than rebuilt from the
/// parsed events, so the bytes on disk are the bytes on the page.
/// Advance `reader` to the root `<svg>` and take its `viewBox`,
/// leaving the reader positioned just after that start tag.
///
/// Anything before it — a prolog, a comment, whitespace — is passed
/// over by recursing on the next event.
fn view_box(id: &str, reader: &mut Reader<&[u8]>) -> Result<String, String> {
    match reader.read_event() {
        Ok(Event::Start(root)) if root.local_name().as_ref() == b"svg" => root
            .try_get_attribute("viewBox")
            .map_err(|error| format!("{id}: reading the root <svg>: {error}"))?
            .ok_or_else(|| format!("{id}: the root <svg> has no viewBox"))?
            // The raw value: it goes straight back into an attribute.
            .value
            .into_owned()
            .pipe(String::from_utf8)
            .map_err(|error| format!("{id}: the viewBox is not UTF-8: {error}")),
        Ok(Event::Eof) => Err(format!("{id}: no root <svg>")),
        Ok(_) => view_box(id, reader),
        Err(error) => Err(format!("{id}: {error}")),
    }
}

/// Advance `reader` to the `</svg>` closing the root element and give
/// the position where that tag begins, `depth` counting the elements
/// opened since the root and still unclosed.
///
/// A `</g>` therefore does not end the search, which is what the depth
/// is for.
fn drawing_end(id: &str, reader: &mut Reader<&[u8]>, depth: usize) -> Result<usize, String> {
    // Read before the event, so the closing tag reports where it begins
    // rather than where it ends.
    let position = reader.buffer_position() as usize;
    match reader.read_event() {
        Ok(Event::Start(_)) => drawing_end(id, reader, depth + 1),
        Ok(Event::End(_)) => match depth.checked_sub(1) {
            Some(remaining) => drawing_end(id, reader, remaining),
            None => Ok(position),
        },
        Ok(Event::Eof) => Err(format!("{id}: the root <svg> is never closed")),
        Ok(_) => drawing_end(id, reader, depth),
        Err(error) => Err(format!("{id}: {error}")),
    }
}

fn symbol(id: &str, source: &str) -> Result<String, String> {
    let mut reader = Reader::from_str(source);
    let view_box = view_box(id, &mut reader)?;
    let start = reader.buffer_position() as usize;
    let end = drawing_end(id, &mut reader, 0)?;

    let drawing = source
        .get(start..end)
        .ok_or_else(|| format!("{id}: the root <svg>'s content is not where the parser said"))?
        .trim();
    Ok(format!(
        r#"<symbol id="{id}" viewBox="{view_box}">{drawing}</symbol>"#,
    ))
}

/// The sprite, for [`crate::render`] to write into the page.
///
/// Panics on an icon it cannot rewrite: a missing icon renders as a gap
/// and is invisible in a diff, so it should stop the build.
pub(crate) fn sprite() -> String {
    let symbols = ICONS
        .iter()
        .map(|icon| {
            symbol(icon.id, icon.source).unwrap_or_else(|error| panic!("bad icon -- {error}"))
        })
        .collect::<String>();
    format!(r#"<svg xmlns="http://www.w3.org/2000/svg" hidden>{symbols}</svg>"#)
}

#[cfg(test)]
mod tests;
