//! Lightweight SVG optimization.
//!
//! Covers the most impactful optimizations for covecto-generated SVGs:
//! - Remove comments, metadata, editors data
//! - Remove empty groups
//! - Shorten numeric values (remove trailing zeros)
//! - Remove redundant whitespace

use crate::Result;
use crate::config::{OptimizeConfig, OptimizePreset};
use std::borrow::Cow;

/// Optimize an SVG string.
pub fn optimize_svg(svg: &str, config: &OptimizeConfig) -> Result<String> {
    if matches!(config.preset, OptimizePreset::None) {
        return Ok(svg.to_string());
    }

    let output = optimize_once(svg, config.preset);

    // Multipass
    if config.multipass {
        let mut prev = output;
        for _ in 0..config.multipass_iterations {
            let next = optimize_once(&prev, config.preset);
            if next.len() >= prev.len() {
                break;
            }
            prev = Cow::Owned(next.into_owned());
        }
        return Ok(prev.into_owned());
    }

    Ok(output.into_owned())
}

/// One optimization pass sequence. Each pass returns `None` when it has
/// nothing to change, so already-minimal input is never copied.
fn optimize_once(svg: &str, preset: OptimizePreset) -> Cow<'_, str> {
    let mut out = Cow::Borrowed(svg);
    let mut apply = |pass: fn(&str) -> Option<String>| {
        if let Some(next) = pass(&out) {
            out = Cow::Owned(next);
        }
    };

    apply(remove_xml_prolog);

    if matches!(preset, OptimizePreset::Default | OptimizePreset::Safe) {
        apply(remove_comments);
        apply(remove_metadata_elements);
        apply(remove_empty_groups);
    }

    if matches!(preset, OptimizePreset::Default) {
        apply(shorten_path_numbers);
        apply(clean_empty_attributes);
    }

    apply(trim_whitespace);
    out
}

/// Positions of `<name` where `name` is a whole tag name
/// (followed by whitespace, `>` or `/`).
fn find_tag(s: &str, name: &str, from: usize) -> Option<usize> {
    let bytes = s.as_bytes();
    let mut search = from;
    while let Some(rel) = s[search..].find('<') {
        let pos = search + rel;
        let after = pos + 1 + name.len();
        if s[pos + 1..].starts_with(name)
            && after <= bytes.len()
            && bytes
                .get(after)
                .is_none_or(|&b| b == b'>' || b == b'/' || b.is_ascii_whitespace())
        {
            return Some(pos);
        }
        search = pos + 1;
    }
    None
}

/// Index just past the `>` that closes the tag starting at `start`,
/// skipping quoted attribute values. Returns `(end, self_closing)`.
fn tag_end(s: &str, start: usize) -> Option<(usize, bool)> {
    let bytes = s.as_bytes();
    let mut quote: Option<u8> = None;
    let mut i = start + 1;
    while i < bytes.len() {
        let b = bytes[i];
        match quote {
            Some(q) if b == q => quote = None,
            Some(_) => {}
            None if b == b'"' || b == b'\'' => quote = Some(b),
            None if b == b'>' => return Some((i + 1, bytes[i - 1] == b'/')),
            None => {}
        }
        i += 1;
    }
    None
}

/// Skip whitespace starting at `i`.
fn skip_ws(s: &str, i: usize) -> usize {
    s[i..]
        .char_indices()
        .find(|(_, c)| !c.is_whitespace())
        .map(|(j, _)| i + j)
        .unwrap_or(s.len())
}

/// Remove elements named `name` that are self-closing (`<x .../>`) or
/// have no content (`<x ...></x>`). Non-empty elements are kept.
fn remove_empty_elements(s: &str, name: &str) -> Option<String> {
    let close = format!("</{name}>");
    let mut result: Option<String> = None;
    // Repeat so that nested empties (`<g><g/></g>`) collapse fully.
    loop {
        let cur: &str = result.as_deref().unwrap_or(s);
        let mut out = String::new();
        let mut copied = 0;
        let mut search = 0;
        let mut changed = false;
        while let Some(start) = find_tag(cur, name, search) {
            let Some((end, self_closing)) = tag_end(cur, start) else {
                break;
            };
            let remove_to = if self_closing {
                Some(end)
            } else {
                let after = skip_ws(cur, end);
                cur[after..]
                    .starts_with(&close)
                    .then_some(after + close.len())
            };
            match remove_to {
                Some(to) => {
                    out.push_str(&cur[copied..start]);
                    copied = skip_ws(cur, to);
                    search = copied;
                    changed = true;
                }
                None => search = end,
            }
        }
        if !changed {
            return result;
        }
        out.push_str(&cur[copied..]);
        result = Some(out);
    }
}

/// Remove XML prolog `<?xml ...?>`.
fn remove_xml_prolog(s: &str) -> Option<String> {
    let idx = s.find("<?xml")?;
    let end = s[idx..].find("?>")?;
    let mut result = String::with_capacity(s.len());
    result.push_str(&s[..idx]);
    result.push_str(&s[idx + end + 2..]);
    Some(result.trim_start().to_string())
}

/// Remove XML comments `<!-- ... -->`.
fn remove_comments(s: &str) -> Option<String> {
    if !s.contains("<!--") {
        return None;
    }
    let mut result = String::with_capacity(s.len());
    let mut search_from = 0;
    while let Some(start) = s[search_from..].find("<!--") {
        result.push_str(&s[search_from..search_from + start]);
        if let Some(end) = s[search_from + start..].find("-->") {
            search_from = search_from + start + end + 3;
        } else {
            // Unclosed comment, keep as-is
            result.push_str(&s[search_from + start..]);
            search_from = s.len();
        }
    }
    result.push_str(&s[search_from..]);
    Some(result)
}

/// Remove `<metadata>`, `<title>`, `<desc>` elements and empty `<defs>`.
fn remove_metadata_elements(s: &str) -> Option<String> {
    let mut result: Option<String> = None;
    for tag in ["metadata", "title", "desc"] {
        let close = format!("</{tag}>");
        loop {
            let cur: &str = result.as_deref().unwrap_or(s);
            let Some(start) = find_tag(cur, tag, 0) else {
                break;
            };
            let Some((open_end, self_closing)) = tag_end(cur, start) else {
                break;
            };
            let end = if self_closing {
                open_end
            } else {
                match cur[open_end..].find(&close) {
                    Some(rel) => open_end + rel + close.len(),
                    None => break,
                }
            };
            let trimmed_end = skip_ws(cur, end);
            let mut next = String::with_capacity(cur.len());
            next.push_str(&cur[..start]);
            next.push_str(&cur[trimmed_end..]);
            result = Some(next);
        }
    }
    let cur: &str = result.as_deref().unwrap_or(s);
    if let Some(next) = remove_empty_elements(cur, "defs") {
        result = Some(next);
    }
    result
}

/// Remove empty `<g>` elements (self-closing or with no content).
fn remove_empty_groups(s: &str) -> Option<String> {
    remove_empty_elements(s, "g")
}

/// Shorten floating point numbers in SVG path data: "1.000" → "1", "2.50" → "2.5".
/// Only operates within `d="..."` attribute values to avoid corrupting URLs
/// (e.g. `xmlns="http://www.w3.org/2000/svg"`).
fn shorten_path_numbers(s: &str) -> Option<String> {
    if !path_data_has_dot(s) {
        return None;
    }
    let mut result = String::with_capacity(s.len());
    let bytes = s.as_bytes();
    let len = bytes.len();
    let mut i = 0;

    while i < len {
        // Look for d="..." attribute — only shorten numbers inside path data
        if i + 3 < len
            && bytes[i] == b'd'
            && bytes[i + 1] == b'='
            && bytes[i + 2] == b'"'
            && (i == 0 || bytes[i - 1].is_ascii_whitespace())
        {
            // Copy d=" literally
            result.push_str("d=\"");
            i += 3;

            // Find closing quote
            let attr_start = i;
            let attr_end = match bytes[attr_start..].iter().position(|&b| b == b'"') {
                Some(pos) => attr_start + pos,
                None => len,
            };

            // Process only the attribute value
            while i < attr_end {
                if bytes[i].is_ascii_digit()
                    || (bytes[i] == b'-' && i + 1 < attr_end && bytes[i + 1].is_ascii_digit())
                {
                    let start = i;
                    if bytes[i] == b'-' {
                        i += 1;
                    }
                    while i < attr_end && bytes[i].is_ascii_digit() {
                        i += 1;
                    }
                    if i < attr_end && bytes[i] == b'.' {
                        i += 1;
                        let decimal_start = i;
                        while i < attr_end && bytes[i].is_ascii_digit() {
                            i += 1;
                        }
                        if i < attr_end && (bytes[i] == b'e' || bytes[i] == b'E') {
                            i += 1;
                            if i < attr_end && (bytes[i] == b'+' || bytes[i] == b'-') {
                                i += 1;
                            }
                            while i < attr_end && bytes[i].is_ascii_digit() {
                                i += 1;
                            }
                            result.push_str(&s[start..i]);
                            continue;
                        }
                        let decimal_end = i;
                        let mut trim_to = decimal_end;
                        while trim_to > decimal_start && bytes[trim_to - 1] == b'0' {
                            trim_to -= 1;
                        }
                        if trim_to == decimal_start {
                            result.push_str(&s[start..decimal_start - 1]); // skip the dot
                        } else {
                            result.push_str(&s[start..trim_to]);
                        }
                    } else {
                        result.push_str(&s[start..i]);
                    }
                } else {
                    result.push(bytes[i] as char);
                    i += 1;
                }
            }

            // Copy closing quote
            if i < len && bytes[i] == b'"' {
                result.push('"');
                i += 1;
            }
        } else {
            result.push(bytes[i] as char);
            i += 1;
        }
    }

    Some(result)
}

/// Whether any `d="..."` attribute value contains a decimal point.
/// Like [`shorten_path_numbers`], only a standalone `d` attribute counts
/// (start of input or preceded by whitespace), not e.g. `id="..."`.
fn path_data_has_dot(s: &str) -> bool {
    let mut from = 0;
    while let Some(rel) = s[from..].find("d=\"") {
        let i = from + rel;
        let value = &s[i + 3..];
        let end = value.find('"').unwrap_or(value.len());
        let standalone = i == 0 || s.as_bytes()[i - 1].is_ascii_whitespace();
        if standalone && value[..end].contains('.') {
            return true;
        }
        from = i + 3 + end;
    }
    false
}

/// Remove empty class="" and id="" attributes.
fn clean_empty_attributes(s: &str) -> Option<String> {
    if !s.contains("class=\"\"") && !s.contains("id=\"\"") {
        return None;
    }
    let mut result = s.to_string();
    for attr in &["class=\"\"", "id=\"\""] {
        while let Some(idx) = result.find(attr) {
            // Check if preceded by whitespace
            let ws_start = result[..idx]
                .char_indices()
                .rev()
                .find(|(_, c)| !c.is_whitespace())
                .map(|(i, _)| i + 1)
                .unwrap_or(0);
            result.replace_range(ws_start..idx + attr.len(), "");
        }
    }
    Some(result)
}

/// Trim leading/trailing whitespace and collapse multiple newlines.
fn trim_whitespace(s: &str) -> Option<String> {
    let trimmed = s.trim();
    // Already normalized: single line ending in exactly one newline.
    if s.len() == trimmed.len() + 1 && s.ends_with('\n') && !trimmed.contains('\n') {
        return None;
    }
    let mut result = String::with_capacity(trimmed.len());
    let mut prev_newline = false;
    for c in trimmed.chars() {
        if c == '\n' {
            if !prev_newline {
                result.push(c);
            }
            prev_newline = true;
        } else if c.is_whitespace() && prev_newline {
            continue;
        } else {
            result.push(c);
            prev_newline = false;
        }
    }
    if !result.ends_with('\n') {
        result.push('\n');
    }
    Some(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_remove_xml_prolog() {
        let input = r#"<?xml version="1.0" encoding="UTF-8"?>
<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100"></svg>"#;
        let expected = r#"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100"></svg>"#;
        assert_eq!(remove_xml_prolog(input).unwrap(), expected);
    }

    #[test]
    fn test_remove_comments() {
        let input = r#"<svg><!-- comment --><rect/></svg>"#;
        let expected = "<svg><rect/></svg>";
        assert_eq!(remove_comments(input).unwrap(), expected);
    }

    #[test]
    fn test_shorten_numbers() {
        let input = r#"<path d="M1.000 2.50 3.140 4.0"/>"#;
        let expected = r#"<path d="M1 2.5 3.14 4"/>"#;
        assert_eq!(shorten_path_numbers(input).unwrap(), expected);
    }

    #[test]
    fn test_shorten_numbers_preserves_urls() {
        // Regression: shorten_path_numbers must NOT strip dots from URLs like xmlns
        let input =
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="100"><path d="M1.0 2.0"/></svg>"#;
        let result = shorten_path_numbers(input).unwrap();
        assert!(
            result.contains("w3.org"),
            "URL should not be corrupted: {result}"
        );
        assert!(
            result.contains("http://www.w3.org/2000/svg"),
            "Full URL intact: {result}"
        );
        assert!(
            result.contains(r#"d="M1 2""#),
            "Path numbers should be shortened: {result}"
        );
    }

    #[test]
    fn test_shorten_numbers_ignores_id_attributes() {
        let input = r#"<path id="v1.0" data-id="2.50" d="M1.500 2.0"/>"#;
        assert_eq!(
            shorten_path_numbers(input).unwrap(),
            r#"<path id="v1.0" data-id="2.50" d="M1.5 2"/>"#
        );
        assert!(!path_data_has_dot(r#"<path id="v1.0" d="M1,2"/>"#));
        assert_eq!(shorten_path_numbers(r#"<path id="v1.0" d="M1,2"/>"#), None);
    }

    #[test]
    fn test_full_optimize() {
        let input = r##"<?xml version="1.0"?>
<!-- Generated by covecto -->
<svg width="100" height="100" xmlns="http://www.w3.org/2000/svg">
  <title>Test</title>
  <path d="M1.000 2.000 L3.500 4.000" fill="#ff0000"/>
</svg>"##;
        let config = OptimizeConfig::default();
        let result = optimize_svg(input, &config).unwrap();
        assert!(!result.contains("<?xml"));
        assert!(!result.contains("<!--"));
        assert!(!result.contains("<title"));
        assert!(!result.contains("1.000"));
    }

    #[test]
    fn test_non_empty_group_is_kept() {
        // Regression (#59): `<g attr>` with children used to be deleted up to
        // the first child's `/>`, dropping the group and its first path.
        let input = r##"<svg xmlns="http://www.w3.org/2000/svg"><g fill-opacity="0.502"><path fill="#0000ff" d="M2,0v2h2v-2Z"/><path fill="#ff0000" d="M0,0v2h2v-2Z"/></g></svg>
"##;
        for preset in [OptimizePreset::Default, OptimizePreset::Safe] {
            let config = OptimizeConfig {
                preset,
                ..Default::default()
            };
            assert_eq!(optimize_svg(input, &config).unwrap(), input);
        }
    }

    #[test]
    fn test_empty_groups_removed() {
        let input = r#"<svg><g id="a"/><g class="x">  </g><g><g/></g><path d="M0 0"/></svg>"#;
        assert_eq!(
            remove_empty_groups(input).unwrap(),
            r#"<svg><path d="M0 0"/></svg>"#
        );
    }

    #[test]
    fn test_group_attr_with_gt_in_quotes() {
        let input = r#"<svg><g data-x="a/>b"><path d="M0 0"/></g></svg>"#;
        assert_eq!(remove_empty_groups(input), None);
    }

    #[test]
    fn test_defs_with_children_kept() {
        let input = r#"<svg><defs><linearGradient id="g"/></defs><defs/><path d="M0 0"/></svg>"#;
        assert_eq!(
            remove_metadata_elements(input).unwrap(),
            r#"<svg><defs><linearGradient id="g"/></defs><path d="M0 0"/></svg>"#
        );
    }

    #[test]
    fn test_tag_name_prefix_not_matched() {
        // `<titlex>` / `<description>` are not `<title>` / `<desc>`.
        let input = r#"<svg><titlex/><description>x</description><title>t</title></svg>"#;
        assert_eq!(
            remove_metadata_elements(input).unwrap(),
            r#"<svg><titlex/><description>x</description></svg>"#
        );
    }

    #[test]
    fn test_minimal_input_is_untouched() {
        let input = "<svg width=\"1\" height=\"1\" xmlns=\"http://www.w3.org/2000/svg\"><path fill=\"#ff0000\" d=\"M0,0v1h1v-1Z\"/></svg>\n";
        let out = optimize_once(input, OptimizePreset::Default);
        assert!(matches!(out, Cow::Borrowed(_)), "no pass should allocate");
    }

    #[test]
    fn test_unclosed_comment_kept() {
        assert_eq!(
            remove_comments("<svg/><!-- open").unwrap(),
            "<svg/><!-- open"
        );
    }
}
