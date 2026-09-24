//! Literal text runs out of a PDF's uncompressed content streams.
//!
//! No decoding, no font mapping: this walks `(...)` string literals that
//! precede a `Tj` or `TJ` show-text operator and concatenates them with a
//! space between operators. Most real-world PDFs wrap their content
//! streams in `FlateDecode`, which this does not inflate, so it can find
//! nothing in a compressed file and returns `None` in that case exactly
//! as it does for a non-PDF buffer — the caller cannot tell "compressed"
//! from "not a PDF" from this alone, matching `pdf::page_count`'s stance
//! that a coarse best-effort reader beats no reader at all.

/// Concatenated literal text runs found in `bytes`, in stream order, or
/// `None` if `bytes` does not start with a PDF header or no `Tj`/`TJ`
/// operator with a preceding string literal was found.
pub fn extract_text(bytes: &[u8]) -> Option<String> {
    if !bytes.starts_with(b"%PDF-") {
        return None;
    }
    let mut out = String::new();
    let mut i = 0;
    while let Some(rel) = find(&bytes[i..], b"Tj") {
        let op_end = i + rel;
        if let Some(text) = literal_before(bytes, op_end) {
            if !out.is_empty() {
                out.push(' ');
            }
            out.push_str(&text);
        }
        i = op_end + b"Tj".len();
    }
    let mut i = 0;
    while let Some(rel) = find(&bytes[i..], b"TJ") {
        let op_end = i + rel;
        if let Some(array_start) = bytes[..op_end].iter().rposition(|&b| b == b'[') {
            let mut j = array_start;
            while let Some(text) = next_literal(bytes, j, op_end) {
                if !out.is_empty() {
                    out.push(' ');
                }
                out.push_str(&text.0);
                j = text.1;
            }
        }
        i = op_end + b"TJ".len();
    }
    if out.is_empty() {
        None
    } else {
        Some(out)
    }
}

/// The `(...)` literal immediately preceding `op_end` (a `Tj` operator's
/// position), skipping whitespace, or `None` if the operator is not
/// preceded by a closing paren.
fn literal_before(bytes: &[u8], op_end: usize) -> Option<String> {
    let mut end = op_end;
    while end > 0 && matches!(bytes[end - 1], b' ' | b'\n' | b'\r' | b'\t') {
        end -= 1;
    }
    if end == 0 || bytes[end - 1] != b')' {
        return None;
    }
    let close = end - 1;
    let open = find_matching_open(bytes, close)?;
    Some(decode_literal(&bytes[open + 1..close]))
}

/// The next `(...)` literal at or after `from`, up to `before`, returning
/// the decoded text and the byte offset just past its closing paren.
fn next_literal(bytes: &[u8], from: usize, before: usize) -> Option<(String, usize)> {
    let open = from + bytes.get(from..before)?.iter().position(|&b| b == b'(')?;
    let close = find_matching_close(bytes, open, before)?;
    Some((decode_literal(&bytes[open + 1..close]), close + 1))
}

fn find_matching_open(bytes: &[u8], close: usize) -> Option<usize> {
    let mut depth = 0i32;
    let mut i = close;
    loop {
        let escaped = i > 0 && bytes[i - 1] == b'\\' && !is_escaped(bytes, i - 1);
        if !escaped {
            match bytes[i] {
                b')' => depth += 1,
                b'(' => {
                    depth -= 1;
                    if depth == 0 {
                        return Some(i);
                    }
                }
                _ => {}
            }
        }
        if i == 0 {
            return None;
        }
        i -= 1;
    }
}

fn find_matching_close(bytes: &[u8], open: usize, limit: usize) -> Option<usize> {
    let mut depth = 0i32;
    let mut i = open;
    while i < limit {
        if bytes[i] == b'\\' {
            i += 2;
            continue;
        }
        match bytes[i] {
            b'(' => depth += 1,
            b')' => {
                depth -= 1;
                if depth == 0 {
                    return Some(i);
                }
            }
            _ => {}
        }
        i += 1;
    }
    None
}

/// True if the backslash at `i` is itself escaped by a preceding run of
/// backslashes with odd parity (i.e. it is a literal `\`, not an escape
/// introducer).
fn is_escaped(bytes: &[u8], i: usize) -> bool {
    let mut count = 0;
    let mut j = i;
    while j > 0 && bytes[j - 1] == b'\\' {
        count += 1;
        j -= 1;
    }
    count % 2 == 1
}

fn decode_literal(raw: &[u8]) -> String {
    let mut s = String::with_capacity(raw.len());
    let mut i = 0;
    while i < raw.len() {
        if raw[i] == b'\\' && i + 1 < raw.len() {
            match raw[i + 1] {
                b'n' => s.push('\n'),
                b'r' => s.push('\r'),
                b't' => s.push('\t'),
                b'(' => s.push('('),
                b')' => s.push(')'),
                b'\\' => s.push('\\'),
                other => s.push(other as char),
            }
            i += 2;
        } else {
            s.push(raw[i] as char);
            i += 1;
        }
    }
    s
}

fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len()).position(|w| w == needle)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn non_pdf_bytes_have_no_text() {
        assert_eq!(extract_text(b"not a pdf"), None);
    }

    #[test]
    fn compressed_stream_has_no_text_found() {
        let pdf = b"%PDF-1.4\n1 0 obj\n<< /Filter /FlateDecode >>\nstream\n\x78\x9c\x00\nendstream\nendobj\n";
        assert_eq!(extract_text(pdf), None);
    }

    #[test]
    fn single_tj_literal_is_extracted() {
        let pdf = b"%PDF-1.4\nBT /F1 12 Tf (Hello particle) Tj ET\n";
        assert_eq!(extract_text(pdf), Some("Hello particle".to_string()));
    }

    #[test]
    fn multiple_tj_operators_join_with_space() {
        let pdf = b"%PDF-1.4\nBT (First line) Tj (Second line) Tj ET\n";
        assert_eq!(extract_text(pdf), Some("First line Second line".to_string()));
    }

    #[test]
    fn tj_array_concatenates_its_literals() {
        let pdf = b"%PDF-1.4\nBT [(Hello) -250 (world)] TJ ET\n";
        assert_eq!(extract_text(pdf), Some("Hello world".to_string()));
    }

    #[test]
    fn escaped_parens_do_not_break_the_literal() {
        let pdf = b"%PDF-1.4\nBT (a \\(nested\\) run) Tj ET\n";
        assert_eq!(extract_text(pdf), Some("a (nested) run".to_string()));
    }

    #[test]
    fn escape_sequences_decode() {
        let pdf = b"%PDF-1.4\nBT (line one\\nline two) Tj ET\n";
        assert_eq!(extract_text(pdf), Some("line one\nline two".to_string()));
    }
}
