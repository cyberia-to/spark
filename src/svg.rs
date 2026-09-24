//! SVG dimensions from the root `<svg>` element's attributes.
//!
//! SVG carries its size as attribute text, not a fixed-offset binary
//! field, so this scans for the tag and reads `width`/`height` directly,
//! falling back to the third and fourth numbers of `viewBox` when either
//! is absent — the same two ways a browser lays out an `<svg>` with no
//! CSS size. No XML parser: file's `Kind::ImageSvg` sniff already proved
//! the tag is there; this re-finds it independently on raw bytes, the
//! same shape as every other format spark parses in this crate.

/// Width and height, or `None` if the bytes have no locatable `<svg>` tag,
/// or neither `width`/`height` nor a four-number `viewBox` is present.
/// A unit other than `px` (`%`, `cm`, `em`, ...) is not a pixel count, so
/// that case returns `None` too rather than a silently wrong number.
pub fn dimensions(bytes: &[u8]) -> Option<(f64, f64)> {
    let text = core::str::from_utf8(bytes).ok()?;
    let tag_start = find_svg_tag(text)?;
    let tag_end = tag_start + text[tag_start..].find('>')?;
    let attrs = &text[tag_start..tag_end];

    if let (Some(width), Some(height)) = (attr_number(attrs, "width"), attr_number(attrs, "height")) {
        return Some((width, height));
    }
    let view_box = attr_value(attrs, "viewBox")?;
    let mut parts = view_box.split_whitespace().filter_map(|p| p.parse::<f64>().ok());
    let _min_x = parts.next()?;
    let _min_y = parts.next()?;
    let width = parts.next()?;
    let height = parts.next()?;
    Some((width, height))
}

/// Byte offset of `<svg` (case-insensitive), only where the next byte is
/// whitespace, `>` or `/` — so `<svgfoo>` is not mistaken for the tag.
fn find_svg_tag(text: &str) -> Option<usize> {
    let lower = text.to_ascii_lowercase();
    let mut from = 0;
    while let Some(rel) = lower[from..].find("<svg") {
        let at = from + rel;
        let after = at + 4;
        match text.as_bytes().get(after) {
            Some(b) if b.is_ascii_whitespace() || *b == b'>' || *b == b'/' => return Some(at),
            Some(_) => from = after,
            None => return None,
        }
    }
    None
}

fn attr_value<'a>(attrs: &'a str, name: &str) -> Option<&'a str> {
    let mut from = 0;
    loop {
        let rel = attrs[from..].find(name)?;
        let at = from + rel;
        let after_name = at + name.len();
        from = after_name;
        let before_ok = at == 0 || !attrs.as_bytes()[at - 1].is_ascii_alphanumeric();
        let mut rest = attrs[after_name..].trim_start();
        if !before_ok || !rest.starts_with('=') {
            continue;
        }
        rest = rest[1..].trim_start();
        let quote = rest.chars().next()?;
        if quote != '"' && quote != '\'' {
            continue;
        }
        let body = &rest[1..];
        let end = body.find(quote)?;
        return Some(&body[..end]);
    }
}

fn attr_number(attrs: &str, name: &str) -> Option<f64> {
    let raw = attr_value(attrs, name)?.trim();
    let numeric = raw.strip_suffix("px").unwrap_or(raw);
    numeric.parse::<f64>().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_width_and_height_attributes() {
        let svg = br#"<svg xmlns="http://www.w3.org/2000/svg" width="120" height="80"></svg>"#;
        assert_eq!(dimensions(svg), Some((120.0, 80.0)));
    }

    #[test]
    fn reads_px_suffixed_attributes() {
        let svg = br#"<svg width="120px" height="80px"></svg>"#;
        assert_eq!(dimensions(svg), Some((120.0, 80.0)));
    }

    #[test]
    fn falls_back_to_view_box_when_width_height_absent() {
        let svg = br#"<svg viewBox="0 0 300 150"></svg>"#;
        assert_eq!(dimensions(svg), Some((300.0, 150.0)));
    }

    #[test]
    fn width_height_win_over_view_box_when_both_present() {
        let svg = br#"<svg width="50" height="25" viewBox="0 0 300 150"></svg>"#;
        assert_eq!(dimensions(svg), Some((50.0, 25.0)));
    }

    #[test]
    fn percentage_width_is_not_a_pixel_count() {
        let svg = br#"<svg width="100%" height="100%"></svg>"#;
        assert_eq!(dimensions(svg), None);
    }

    #[test]
    fn finds_the_tag_behind_an_xml_declaration_and_doctype() {
        let svg = b"<?xml version=\"1.0\"?>\n<!DOCTYPE svg PUBLIC \"-//W3C//DTD SVG 1.1//EN\" \"x\">\n<svg width=\"10\" height=\"20\"></svg>";
        assert_eq!(dimensions(svg), Some((10.0, 20.0)));
    }

    #[test]
    fn tag_name_prefix_collision_is_not_matched() {
        let svg = br#"<svgicon width="10" height="20"></svgicon>"#;
        assert_eq!(dimensions(svg), None);
    }

    #[test]
    fn non_svg_bytes_have_no_dimensions() {
        assert_eq!(dimensions(b"just some text, not an image"), None);
    }

    #[test]
    fn neither_size_nor_view_box_is_none() {
        let svg = br#"<svg xmlns="http://www.w3.org/2000/svg"></svg>"#;
        assert_eq!(dimensions(svg), None);
    }
}
