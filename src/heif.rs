//! Image dimensions for HEIC/HEIF/AVIF — walk ISOBMFF boxes to the first
//! `ispe` (Image Spatial Extents) property.
//!
//! Unlike BMP/TIFF/PNG, HEIF containers do not put width/height at a fixed
//! offset: they live in an `ispe` box nested inside `meta > iprp > ipco`.
//! This walks only the box types on that path (plus `meta`'s FullBox
//! version/flags header) and returns the first `ispe` found. Real files
//! carry one image per `ispe`; a container with a thumbnail or an
//! auxiliary (alpha/depth) image ahead of the primary item in box order
//! is a documented gap, not a silent wrong answer — `ipma`'s item-to-
//! property association, which would disambiguate them, is not parsed.

const BOX_HEADER_LEN: usize = 8;
const FULLBOX_HEADER_LEN: usize = 4;

/// `(width, height)` from the first `ispe` box reachable through
/// `meta`/`iprp`/`ipco`, or `None` if the bytes are not a recognized
/// ISOBMFF stream or no `ispe` box is found.
pub fn dimensions(bytes: &[u8]) -> Option<(u32, u32)> {
    scan_boxes(bytes, 0, bytes.len())
}

fn scan_boxes(bytes: &[u8], start: usize, end: usize) -> Option<(u32, u32)> {
    let mut pos = start;
    while pos + BOX_HEADER_LEN <= end {
        let size = u32::from_be_bytes(bytes[pos..pos + 4].try_into().ok()?) as usize;
        let kind = &bytes[pos + 4..pos + 8];
        // size == 0 ("extends to EOF") and size == 1 (64-bit largesize
        // follows) are both real ISOBMFF shapes; neither is expected this
        // deep in a still image's box tree, so they are a documented gap.
        if size < BOX_HEADER_LEN || pos + size > end {
            break;
        }
        let body_start = pos + BOX_HEADER_LEN;
        let body_end = pos + size;

        match kind {
            b"ispe" => {
                let fields_start = body_start + FULLBOX_HEADER_LEN;
                if fields_start + 8 <= body_end {
                    let w = u32::from_be_bytes(bytes[fields_start..fields_start + 4].try_into().ok()?);
                    let h = u32::from_be_bytes(bytes[fields_start + 4..fields_start + 8].try_into().ok()?);
                    return Some((w, h));
                }
            }
            b"meta" => {
                // meta is a FullBox: 4 bytes of version+flags before its children.
                if let Some(found) = scan_boxes(bytes, body_start + FULLBOX_HEADER_LEN, body_end) {
                    return Some(found);
                }
            }
            b"iprp" | b"ipco" => {
                if let Some(found) = scan_boxes(bytes, body_start, body_end) {
                    return Some(found);
                }
            }
            _ => {}
        }
        pos += size;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bx(kind: &[u8; 4], body: &[u8]) -> Vec<u8> {
        let mut v = ((body.len() + BOX_HEADER_LEN) as u32).to_be_bytes().to_vec();
        v.extend_from_slice(kind);
        v.extend_from_slice(body);
        v
    }

    fn ispe(width: u32, height: u32) -> Vec<u8> {
        let mut body = vec![0u8; 4]; // version + flags
        body.extend_from_slice(&width.to_be_bytes());
        body.extend_from_slice(&height.to_be_bytes());
        bx(b"ispe", &body)
    }

    fn meta_with(children: &[u8]) -> Vec<u8> {
        let mut body = vec![0u8; 4]; // FullBox version + flags
        body.extend_from_slice(children);
        bx(b"meta", &body)
    }

    #[test]
    fn finds_ispe_through_meta_iprp_ipco() {
        let ipco = bx(b"ipco", &ispe(1920, 1080));
        let iprp = bx(b"iprp", &ipco);
        let meta = meta_with(&iprp);
        let mut file = bx(b"ftyp", b"heic\0\0\0\0heicmif1");
        file.extend_from_slice(&meta);
        assert_eq!(dimensions(&file), Some((1920, 1080)));
    }

    #[test]
    fn skips_unrelated_property_boxes_inside_ipco() {
        let unrelated = bx(b"pixi", &[3, 8, 8, 8]);
        let mut ipco_body = unrelated.clone();
        ipco_body.extend_from_slice(&ispe(640, 480));
        let ipco = bx(b"ipco", &ipco_body);
        let iprp = bx(b"iprp", &ipco);
        let meta = meta_with(&iprp);
        assert_eq!(dimensions(&meta), Some((640, 480)));
    }

    #[test]
    fn skips_sibling_meta_children_before_iprp() {
        let mut children = bx(b"hdlr", &[0u8; 20]);
        children.extend_from_slice(&bx(b"pitm", &[0, 0]));
        let ipco = bx(b"ipco", &ispe(4032, 3024));
        children.extend_from_slice(&bx(b"iprp", &ipco));
        let meta = meta_with(&children);
        assert_eq!(dimensions(&meta), Some((4032, 3024)));
    }

    #[test]
    fn no_ispe_anywhere_returns_none() {
        let meta = meta_with(&bx(b"hdlr", &[0u8; 8]));
        assert_eq!(dimensions(&meta), None);
    }

    #[test]
    fn empty_bytes_return_none() {
        assert_eq!(dimensions(&[]), None);
    }

    #[test]
    fn truncated_box_header_does_not_panic() {
        assert_eq!(dimensions(&[0, 0, 0]), None);
    }

    #[test]
    fn size_zero_box_stops_instead_of_looping_forever() {
        // size == 0 ("box extends to end of file") is unsupported; must not spin.
        let mut file = vec![0, 0, 0, 0];
        file.extend_from_slice(b"meta");
        assert_eq!(dimensions(&file), None);
    }

    #[test]
    fn declared_size_past_end_of_buffer_is_rejected() {
        let mut file = 999u32.to_be_bytes().to_vec();
        file.extend_from_slice(b"meta");
        assert_eq!(dimensions(&file), None);
    }

    #[test]
    fn truncated_ispe_body_returns_none_instead_of_panicking() {
        // ispe box header claims a body but the buffer ends before the fields.
        let ispe_box = bx(b"ispe", &[0, 0, 0, 0, 0, 0]);
        let ipco = bx(b"ipco", &ispe_box);
        let iprp = bx(b"iprp", &ipco);
        let meta = meta_with(&iprp);
        assert_eq!(dimensions(&meta), None);
    }
}
