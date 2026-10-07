//! PDF page size, read straight off the first `/MediaBox` array.
//!
//! No decoding, no rendering: alongside a page count (spark#4, open,
//! `pdf.rs`) a page size is the second fact a reader needs before it can
//! lay out a page at all — the shell needs an aspect ratio to reserve
//! space for before the first page is decoded. This is a sibling reader
//! of the same shape as `pdf::page_count`, kept in its own module only
//! because spark#4 is still unmerged and both PRs are independent
//! branches off `origin/main`; once spark#4 lands this can move into
//! `pdf.rs` next to `page_count`.
//!
//! A PDF can give each page its own `/MediaBox`, inherited down the
//! `/Pages` tree when a page omits one. This reads the first `/MediaBox`
//! found in the file, which is the whole-document size for the common
//! case of one size for every page; it does not walk the page tree to
//! resolve per-page overrides.

/// Page width and height in PDF points (1/72 inch), or `None` if `bytes`
/// does not start with a PDF header or no `/MediaBox` array is found.
pub fn dimensions(bytes: &[u8]) -> Option<(f64, f64)> {
    if !bytes.starts_with(b"%PDF-") {
        return None;
    }
    let at = find(bytes, b"/MediaBox")?;
    let mut i = at + b"/MediaBox".len();
    i = skip_ws(bytes, i);
    if bytes.get(i) != Some(&b'[') {
        return None;
    }
    i += 1;
    let mut numbers = [0f64; 4];
    for slot in numbers.iter_mut() {
        i = skip_ws(bytes, i);
        let start = i;
        while matches!(bytes.get(i), Some(b) if b.is_ascii_digit() || *b == b'.' || *b == b'-' || *b == b'+') {
            i += 1;
        }
        if i == start {
            return None;
        }
        *slot = core::str::from_utf8(&bytes[start..i]).ok()?.parse().ok()?;
    }
    let width = (numbers[2] - numbers[0]).abs();
    let height = (numbers[3] - numbers[1]).abs();
    Some((width, height))
}

fn skip_ws(bytes: &[u8], mut i: usize) -> usize {
    while matches!(bytes.get(i), Some(b' ' | b'\n' | b'\r' | b'\t')) {
        i += 1;
    }
    i
}

fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len()).position(|w| w == needle)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn non_pdf_bytes_have_no_dimensions() {
        assert_eq!(dimensions(b"not a pdf"), None);
    }

    #[test]
    fn no_media_box_is_none() {
        let pdf = b"%PDF-1.4\n1 0 obj\n<< /Type /Catalog >>\nendobj\n";
        assert_eq!(dimensions(pdf), None);
    }

    #[test]
    fn reads_us_letter_media_box() {
        let pdf = b"%PDF-1.7\n1 0 obj\n<< /Type /Page /MediaBox [0 0 612 792] >>\nendobj\n";
        assert_eq!(dimensions(pdf), Some((612.0, 792.0)));
    }

    #[test]
    fn reads_a_media_box_with_a_nonzero_origin() {
        let pdf = b"%PDF-1.7\n<< /MediaBox [-10 -20 590 770] >>\n";
        assert_eq!(dimensions(pdf), Some((600.0, 790.0)));
    }

    #[test]
    fn reads_fractional_coordinates() {
        let pdf = b"%PDF-1.7\n<< /MediaBox [0.0 0.0 595.32 841.92] >>\n";
        assert_eq!(dimensions(pdf), Some((595.32, 841.92)));
    }

    #[test]
    fn tolerates_missing_whitespace_after_the_key() {
        let pdf = b"%PDF-1.7\n<< /MediaBox[0 0 100 200] >>\n";
        assert_eq!(dimensions(pdf), Some((100.0, 200.0)));
    }

    #[test]
    fn truncated_array_is_none() {
        let pdf = b"%PDF-1.7\n<< /MediaBox [0 0 100";
        assert_eq!(dimensions(pdf), None);
    }

    #[test]
    fn uses_the_first_media_box_in_a_multi_page_document() {
        let pdf = b"%PDF-1.7\n\
                    1 0 obj\n<< /Type /Page /MediaBox [0 0 300 400] >>\nendobj\n\
                    2 0 obj\n<< /Type /Page /MediaBox [0 0 500 600] >>\nendobj\n";
        assert_eq!(dimensions(pdf), Some((300.0, 400.0)));
    }
}
