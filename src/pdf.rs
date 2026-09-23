//! PDF page count, read straight off the object table.
//!
//! No decoding, no rendering: a page count is the first fact a reader
//! needs before it can page through a file at all. This walks the raw
//! bytes for `/Type /Page` object markers, skipping `/Type /Pages` (the
//! page-tree node, not a page). Object streams and compressed
//! cross-reference tables (common in PDF 1.5+) hide their `/Type /Page`
//! markers inside a deflate stream this does not decompress, so the
//! count can undercount on those files; a false PDF header still
//! resolves cleanly to `Some(0)` rather than an error.

/// Counts `/Type /Page` object markers in a PDF's raw bytes. Returns
/// `None` if `bytes` does not start with a PDF header.
pub fn page_count(bytes: &[u8]) -> Option<usize> {
    if !bytes.starts_with(b"%PDF-") {
        return None;
    }
    let mut count = 0;
    let mut i = 0;
    while let Some(offset) = find(&bytes[i..], b"/Type") {
        let after_type = i + offset + b"/Type".len();
        let mut j = after_type;
        while matches!(bytes.get(j), Some(b' ' | b'\n' | b'\r' | b'\t')) {
            j += 1;
        }
        if bytes[j..].starts_with(b"/Page") {
            let end = j + b"/Page".len();
            let followed_by_word_char = bytes.get(end).is_some_and(u8::is_ascii_alphabetic);
            if !followed_by_word_char {
                count += 1;
            }
        }
        i = after_type;
    }
    Some(count)
}

fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len()).position(|w| w == needle)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn non_pdf_bytes_have_no_page_count() {
        assert_eq!(page_count(b"not a pdf"), None);
    }

    #[test]
    fn pages_node_is_not_a_page() {
        let pdf = b"%PDF-1.4\n1 0 obj\n<< /Type /Pages /Count 1 >>\nendobj\n";
        assert_eq!(page_count(pdf), Some(0));
    }

    #[test]
    fn counts_page_objects_with_and_without_space() {
        let pdf = b"%PDF-1.7\n\
                    1 0 obj\n<< /Type /Pages /Count 3 /Kids [2 0 R 3 0 R 4 0 R] >>\nendobj\n\
                    2 0 obj\n<< /Type /Page /Parent 1 0 R >>\nendobj\n\
                    3 0 obj\n<< /Type/Page /Parent 1 0 R >>\nendobj\n\
                    4 0 obj\n<< /Type  /Page /Parent 1 0 R >>\nendobj\n";
        assert_eq!(page_count(pdf), Some(3));
    }

    #[test]
    fn empty_document_has_zero_pages() {
        assert_eq!(page_count(b"%PDF-1.4\n%%EOF"), Some(0));
    }
}
