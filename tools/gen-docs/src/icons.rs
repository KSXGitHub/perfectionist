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
use quick_xml::escape::escape;
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

/// Take the root `<svg>`'s `viewBox`, leaving `reader` just past that
/// start tag.
///
/// The value is escaped for the `"`-delimited attribute it lands in.
/// Its own spelling will not do: XML also lets `'` delimit an
/// attribute, and such a value may hold a `"`.
fn view_box(reader: &mut Reader<&[u8]>, id: &str) -> Result<String, String> {
    match reader.read_event() {
        Ok(Event::Start(root)) if root.local_name().as_ref() == b"svg" => root
            .try_get_attribute("viewBox")
            .map_err(|error| format!("{id}: reading the root <svg>: {error}"))?
            .ok_or_else(|| format!("{id}: the root <svg> has no viewBox"))?
            .unescape_value()
            .map_err(|error| format!("{id}: reading the viewBox: {error}"))?
            .pipe_deref(escape)
            .into_owned()
            .pipe(Ok),
        Ok(Event::Eof) => Err(format!("{id}: no root <svg>")),
        Ok(_) => view_box(reader, id),
        Err(error) => Err(format!("{id}: {error}")),
    }
}

/// Give the position where the `</svg>` closing the root begins.
///
/// `depth` counts the elements opened since the root and still
/// unclosed, so a `</g>` does not end the search.
fn drawing_end(reader: &mut Reader<&[u8]>, id: &str, depth: usize) -> Result<usize, String> {
    // Read before the event, so the closing tag reports where it begins
    // rather than where it ends.
    let position = reader.buffer_position() as usize;
    match reader.read_event() {
        Ok(Event::Start(_)) => drawing_end(reader, id, depth + 1),
        Ok(Event::End(_)) => match depth.checked_sub(1) {
            Some(remaining) => drawing_end(reader, id, remaining),
            None => Ok(position),
        },
        Ok(Event::Eof) => Err(format!("{id}: the root <svg> is never closed")),
        Ok(_) => drawing_end(reader, id, depth),
        Err(error) => Err(format!("{id}: {error}")),
    }
}

/// Rewrite one `<svg>` document as a `<symbol>` under `id`.
///
/// The drawing is sliced out of `source` rather than rebuilt from the
/// parsed events, so the bytes on disk are the bytes on the page.
fn symbol(id: &str, source: &str) -> Result<String, String> {
    let mut reader = Reader::from_str(source);
    let view_box = view_box(&mut reader, id)?;
    let start = reader.buffer_position() as usize;
    let end = drawing_end(&mut reader, id, 0)?;

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
