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

/// Whether a `<name` tag (whole tag name, followed by whitespace, `>` or
/// `/`) starts exactly at `pos`. O(name length).
fn is_tag_at(s: &str, name: &str, pos: usize) -> bool {
    let bytes = s.as_bytes();
    let after = pos + 1 + name.len();
    bytes.get(pos) == Some(&b'<')
        && s.get(pos + 1..).is_some_and(|rest| rest.starts_with(name))
        && bytes
            .get(after)
            .is_none_or(|&b| b == b'>' || b == b'/' || b.is_ascii_whitespace())
}

/// Position of the next `<name` tag at or after `from` (see [`is_tag_at`]).
fn find_tag(s: &str, name: &str, from: usize) -> Option<usize> {
    let mut search = from;
    while let Some(rel) = s[search..].find('<') {
        let pos = search + rel;
        if is_tag_at(s, name, pos) {
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
/// have no content (`<x ...></x>`, whitespace only). Non-empty elements are
/// kept. Nested empties (`<g><g/></g>`) collapse fully.
///
/// Single linear pass with a stack of open elements, so deeply nested input
/// cannot make this quadratic.
fn remove_empty_elements(s: &str, name: &str) -> Option<String> {
    let close = format!("</{name}>");
    let mut out = String::with_capacity(s.len());
    // (output length before the open tag, element has kept content)
    let mut stack: Vec<(usize, bool)> = Vec::new();
    let mut changed = false;
    let mut i = 0;

    let mark_content = |stack: &mut Vec<(usize, bool)>| {
        if let Some(top) = stack.last_mut() {
            top.1 = true;
        }
    };

    while i < s.len() {
        let Some(rel) = s[i..].find('<') else {
            if !s[i..].trim().is_empty() {
                mark_content(&mut stack);
            }
            out.push_str(&s[i..]);
            break;
        };
        let lt = i + rel;
        if !s[i..lt].trim().is_empty() {
            mark_content(&mut stack);
        }
        out.push_str(&s[i..lt]);

        if s[lt..].starts_with(&close) {
            let after = lt + close.len();
            match stack.pop() {
                Some((open_at, false)) => {
                    out.truncate(open_at);
                    i = skip_ws(s, after);
                    changed = true;
                }
                Some((_, true)) => {
                    out.push_str(&close);
                    mark_content(&mut stack);
                    i = after;
                }
                None => {
                    out.push_str(&close);
                    i = after;
                }
            }
            continue;
        }

        if is_tag_at(s, name, lt)
            && let Some((end, self_closing)) = tag_end(s, lt)
        {
            if self_closing {
                i = skip_ws(s, end);
                changed = true;
            } else {
                stack.push((out.len(), false));
                out.push_str(&s[lt..end]);
                i = end;
            }
            continue;
        }

        // Any other markup counts as content of the enclosing element.
        mark_content(&mut stack);
        let end = tag_end(s, lt).map_or(lt + 1, |(end, _)| end);
        out.push_str(&s[lt..end]);
        i = end;
    }
    changed.then_some(out)
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
        let cur: &str = result.as_deref().unwrap_or(s);
        let close = format!("</{tag}>");
        let mut out = String::new();
        let mut copied = 0;
        let mut search = 0;
        while let Some(start) = find_tag(cur, tag, search) {
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
            out.push_str(&cur[copied..start]);
            copied = skip_ws(cur, end);
            search = copied;
        }
        if copied > 0 {
            out.push_str(&cur[copied..]);
            result = Some(out);
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
/// Only operates within standalone `d="..."` attribute values to avoid
/// corrupting URLs (e.g. `xmlns="http://www.w3.org/2000/svg"`) or other
/// attributes such as `id="v1.0"`. Non-ASCII text is copied untouched.
fn shorten_path_numbers(s: &str) -> Option<String> {
    if !path_data_has_dot(s) {
        return None;
    }
    let bytes = s.as_bytes();
    let mut result = String::with_capacity(s.len());
    let mut i = 0;
    while let Some(rel) = s[i..].find("d=\"") {
        let at = i + rel;
        let value_start = at + 3;
        result.push_str(&s[i..value_start]);
        i = value_start;
        if at > 0 && !bytes[at - 1].is_ascii_whitespace() {
            continue; // e.g. `id="`, not path data
        }
        let value_end = s[i..].find('"').map_or(s.len(), |p| i + p);
        shorten_numbers_in(&s[i..value_end], &mut result);
        i = value_end;
    }
    result.push_str(&s[i..]);
    Some(result)
}

/// Copy path data `d` into `out`, trimming trailing fractional zeros.
fn shorten_numbers_in(d: &str, out: &mut String) {
    let bytes = d.as_bytes();
    let len = bytes.len();
    let mut i = 0;
    while i < len {
        // A number starts with a digit, or `.digit`, optionally after `-`.
        let digit_at = |j: usize| j < len && bytes[j].is_ascii_digit();
        let unsigned_at =
            |j: usize| digit_at(j) || (j < len && bytes[j] == b'.' && digit_at(j + 1));
        let starts_number = unsigned_at(i) || (bytes[i] == b'-' && unsigned_at(i + 1));
        if !starts_number {
            // Copy everything up to the next possible number start verbatim
            // (keeps multi-byte UTF-8 intact).
            let next = d[i + 1..]
                .find(|c: char| c.is_ascii_digit() || c == '-' || c == '.')
                .map_or(len, |p| i + 1 + p);
            out.push_str(&d[i..next]);
            i = next;
            continue;
        }
        let start = i;
        if bytes[i] == b'-' {
            i += 1;
        }
        let int_start = i;
        while i < len && bytes[i].is_ascii_digit() {
            i += 1;
        }
        let has_int = i > int_start;
        if i >= len || bytes[i] != b'.' {
            out.push_str(&d[start..i]);
            continue;
        }
        i += 1;
        let decimal_start = i;
        while i < len && bytes[i].is_ascii_digit() {
            i += 1;
        }
        if i < len && (bytes[i] == b'e' || bytes[i] == b'E') {
            i += 1;
            if i < len && (bytes[i] == b'+' || bytes[i] == b'-') {
                i += 1;
            }
            while i < len && bytes[i].is_ascii_digit() {
                i += 1;
            }
            out.push_str(&d[start..i]);
            continue;
        }
        let mut trim_to = i;
        while trim_to > decimal_start && bytes[trim_to - 1] == b'0' {
            trim_to -= 1;
        }
        if trim_to == decimal_start {
            // Whole fraction is zero. Keep the number as-is when dropping it
            // would change the token stream: a following "." starts the next
            // number ("1.0.5" = 1.0 and .5, not 1.5), and ".0" has no integer
            // digits left to emit.
            if !has_int || (i < len && bytes[i] == b'.') {
                out.push_str(&d[start..i]);
            } else {
                out.push_str(&d[start..decimal_start - 1]); // skip the dot
            }
        } else {
            out.push_str(&d[start..trim_to]);
        }
    }
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

/// Remove empty `class=""` and `id=""` attributes (standalone attribute
/// names only, so `data-id=""` is kept), with their leading whitespace.
fn clean_empty_attributes(s: &str) -> Option<String> {
    if !s.contains("class=\"\"") && !s.contains("id=\"\"") {
        return None;
    }
    let bytes = s.as_bytes();
    let mut out = String::with_capacity(s.len());
    let mut copied = 0;
    let mut i = 0;
    while i < s.len() {
        let Some(rel) = s[i..].find("=\"\"") else {
            break;
        };
        let eq = i + rel;
        i = eq + 3;
        let name_start = ["class", "id"]
            .iter()
            .find(|n| s[..eq].ends_with(*n))
            .map(|n| eq - n.len());
        let Some(name_start) = name_start else {
            continue;
        };
        if name_start == 0 || !bytes[name_start - 1].is_ascii_whitespace() {
            continue; // e.g. `data-id=""`
        }
        let ws_start = s[..name_start].trim_end().len();
        if ws_start < copied {
            continue;
        }
        out.push_str(&s[copied..ws_start]);
        copied = eq + 3;
    }
    if copied == 0 {
        return None;
    }
    out.push_str(&s[copied..]);
    Some(out)
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

    #[test]
    fn test_deeply_nested_groups_are_linear() {
        // Regression: one nesting level per pass made this quadratic.
        let depth = 200_000;
        let input = format!("<svg>{}{}</svg>", "<g>".repeat(depth), "</g>".repeat(depth));
        let t = std::time::Instant::now();
        assert_eq!(remove_empty_groups(&input).unwrap(), "<svg></svg>");
        assert!(
            t.elapsed() < std::time::Duration::from_secs(2),
            "{:?}",
            t.elapsed()
        );

        let kept = format!("<svg>{}<path/>{}</svg>", "<g>".repeat(3), "</g>".repeat(3));
        assert_eq!(remove_empty_groups(&kept), None);
    }

    #[test]
    fn test_shorten_numbers_keeps_utf8() {
        let input = "<svg><text>café ½</text><path d=\"M1.50 2\"/></svg>";
        assert_eq!(
            shorten_path_numbers(input).unwrap(),
            "<svg><text>café ½</text><path d=\"M1.5 2\"/></svg>"
        );
    }

    #[test]
    fn test_shorten_numbers_compact_form() {
        // "1.0.5" is two numbers (1.0 and .5); must not become "1.5".
        let input = r#"<path d="M1.0.5L2.50.25 3.0 4.0"/>"#;
        assert_eq!(
            shorten_path_numbers(input).unwrap(),
            r#"<path d="M1.0.5L2.5.25 3 4"/>"#
        );
    }

    #[test]
    fn test_shorten_numbers_leading_dot() {
        // ".5.0" is two numbers (.5 and .0); must not lose the second one.
        let input = r#"<path d="M.5.0L-.50 .250 .0 1.0.0"/>"#;
        assert_eq!(
            shorten_path_numbers(input).unwrap(),
            r#"<path d="M.5.0L-.5 .25 .0 1.0.0"/>"#
        );
    }

    #[test]
    fn test_remove_empty_groups_linear_without_matches() {
        // Regression: checking each '<' with find_tag rescanned the rest of
        // the document when it contained no matching tag (quadratic).
        let input = format!("<svg>{}</svg>", "<path d=\"M0 0\"/>".repeat(300_000));
        let t = std::time::Instant::now();
        assert_eq!(remove_empty_elements(&input, "defs"), None);
        assert_eq!(remove_empty_groups(&input), None);
        assert!(
            t.elapsed() < std::time::Duration::from_secs(2),
            "{:?}",
            t.elapsed()
        );
    }

    #[test]
    fn test_clean_empty_attributes_standalone_only() {
        let input = r#"<g id="" data-id=""><path class="" d="M0 0"/></g>"#;
        assert_eq!(
            clean_empty_attributes(input).unwrap(),
            r#"<g data-id=""><path d="M0 0"/></g>"#
        );
        assert_eq!(clean_empty_attributes(r#"<g data-id="">"#), None);
    }

    #[test]
    fn test_metadata_removed_in_one_pass() {
        let input = "<svg><title>a</title><desc/>\n<metadata>m</metadata><path d=\"M0 0\"/></svg>";
        assert_eq!(
            remove_metadata_elements(input).unwrap(),
            "<svg><path d=\"M0 0\"/></svg>"
        );
    }
}
