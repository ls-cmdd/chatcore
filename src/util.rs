//! دوال مساعدة عامة: تحليل query string وfك تشفير percent-encoding في الـ URL.

use std::collections::HashMap;

/// يفصل الـ path عن الـ query string، ويرجع (path, query_map).
/// مثال: "/conversations?limit=10" -> ("/conversations", {"limit": "10"})
pub fn split_path_query(full_path: &str) -> (String, HashMap<String, String>) {
    match full_path.split_once('?') {
        Some((path, query)) => (path.to_string(), parse_query(query)),
        None => (full_path.to_string(), HashMap::new()),
    }
}

fn parse_query(query: &str) -> HashMap<String, String> {
    let mut map = HashMap::new();
    for pair in query.split('&') {
        if pair.is_empty() {
            continue;
        }
        match pair.split_once('=') {
            Some((k, v)) => {
                map.insert(percent_decode(k), percent_decode(v));
            }
            None => {
                map.insert(percent_decode(pair), String::new());
            }
        }
    }
    map
}

/// فك تشفير بسيط لـ percent-encoding (%XX) واستبدال '+' بمسافة (شائع في query strings).
pub fn percent_decode(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'+' => {
                out.push(b' ');
                i += 1;
            }
            b'%' if i + 2 < bytes.len() => {
                let hex = std::str::from_utf8(&bytes[i + 1..i + 3]).unwrap_or("");
                if let Ok(value) = u8::from_str_radix(hex, 16) {
                    out.push(value);
                    i += 3;
                } else {
                    out.push(bytes[i]);
                    i += 1;
                }
            }
            b => {
                out.push(b);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_path_query_without_query_returns_empty_map() {
        let (path, query) = split_path_query("/health");
        assert_eq!(path, "/health");
        assert!(query.is_empty());
    }

    #[test]
    fn split_path_query_decodes_values_and_flags() {
        let (path, query) =
            split_path_query("/messages?limit=10&search=hello+world&flag&empty=&encoded=%2F");

        assert_eq!(path, "/messages");
        assert_eq!(query.get("limit").map(String::as_str), Some("10"));
        assert_eq!(query.get("search").map(String::as_str), Some("hello world"));
        assert_eq!(query.get("flag").map(String::as_str), Some(""));
        assert_eq!(query.get("empty").map(String::as_str), Some(""));
        assert_eq!(query.get("encoded").map(String::as_str), Some("/"));
    }

    #[test]
    fn percent_decode_preserves_malformed_escapes() {
        assert_eq!(percent_decode("%2G%"), "%2G%");
    }

    #[test]
    fn percent_decode_handles_utf8_sequences() {
        assert_eq!(percent_decode("caf%C3%A9"), "café");
    }
}
