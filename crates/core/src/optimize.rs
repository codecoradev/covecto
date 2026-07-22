//! Lightweight SVG optimization.
//!
//! Covers the most impactful optimizations for covecto-generated SVGs:
//! - Remove comments, metadata, editors data
//! - Remove empty groups
//! - Shorten numeric values (remove trailing zeros)
//! - Remove redundant whitespace

use crate::Result;
use crate::config::{OptimizeConfig, OptimizePreset};

/// Optimize an SVG string.
pub fn optimize_svg(svg: &str, config: &OptimizeConfig) -> Result<String> {
    if matches!(config.preset, OptimizePreset::None) {
        return Ok(svg.to_string());
    }

    let mut output = svg.to_string();

    // String-level passes
    output = remove_xml_prolog(&output);

    if matches!(
        config.preset,
        OptimizePreset::Default | OptimizePreset::Safe
    ) {
        output = remove_comments(&output);
        output = remove_metadata_elements(&output);
        output = remove_empty_groups(&output);
    }

    if matches!(config.preset, OptimizePreset::Default) {
        output = shorten_path_numbers(&output);
        output = clean_empty_attributes(&output);
    }

    output = trim_whitespace(&output);

    // Multipass
    if config.multipass {
        let mut prev = output;
        for _ in 0..config.multipass_iterations {
            let next = optimize_svg(
                &prev,
                &OptimizeConfig {
                    preset: config.preset,
                    multipass: false,
                    multipass_iterations: 0,
                },
            )?;
            if next.len() >= prev.len() {
                break;
            }
            prev = next;
        }
        return Ok(prev);
    }

    Ok(output)
}

/// Remove XML prolog `<?xml ...?>`.
fn remove_xml_prolog(s: &str) -> String {
    let Some(idx) = s.find("<?xml") else {
        return s.to_string();
    };
    let Some(end) = s[idx..].find("?>") else {
        return s.to_string();
    };
    let mut result = String::with_capacity(s.len());
    result.push_str(&s[..idx]);
    result.push_str(&s[idx + end + 2..]);
    result.trim_start().to_string()
}

/// Remove XML comments `<!-- ... -->`.
fn remove_comments(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut search_from = 0;
    while let Some(start) = s[search_from..].find("<!--") {
        result.push_str(&s[search_from..search_from + start]);
        if let Some(end) = s[search_from + start..].find("-->") {
            search_from = search_from + start + end + 3;
        } else {
            // Unclosed comment, keep as-is
            search_from = s.len();
        }
    }
    result.push_str(&s[search_from..]);
    result
}

/// Remove `<metadata>`, `<title>`, `<desc>` elements.
fn remove_metadata_elements(s: &str) -> String {
    let mut result = s.to_string();
    for tag in &["metadata", "title", "desc"] {
        let open = format!("<{tag}");
        let close = format!("</{tag}>");
        while let Some(start) = result.find(&open) {
            // Find closing tag
            if let Some(end) = result[start..].find(&close) {
                let end = start + end + close.len();
                // Also include trailing whitespace/newline
                let trimmed_end = result[end..]
                    .char_indices()
                    .find(|(_, c)| !c.is_whitespace())
                    .map(|(i, _)| end + i)
                    .unwrap_or(end);
                result.replace_range(start..trimmed_end, "");
            } else {
                break;
            }
        }
    }
    // Remove self-closing empty defs
    while let Some(start) = result.find("<defs") {
        let Some(end) = result[start..].find("/>") else {
            break;
        };
        let end = start + end + 2;
        let trimmed_end = result[end..]
            .char_indices()
            .find(|(_, c)| !c.is_whitespace())
            .map(|(i, _)| end + i)
            .unwrap_or(end);
        result.replace_range(start..trimmed_end, "");
    }
    result
}

/// Remove empty self-closing `<g .../>` elements.
fn remove_empty_groups(s: &str) -> String {
    let mut result = s.to_string();
    while let Some(start) = result.find("<g ") {
        let Some(end) = result[start..].find("/>") else {
            break;
        };
        let end = start + end + 2;
        let trimmed_end = result[end..]
            .char_indices()
            .find(|(_, c)| !c.is_whitespace())
            .map(|(i, _)| end + i)
            .unwrap_or(end);
        result.replace_range(start..trimmed_end, "");
    }
    result
}

/// Shorten floating point numbers in SVG path data: "1.000" → "1", "2.50" → "2.5".
/// Only operates within `d="..."` attribute values to avoid corrupting URLs
/// (e.g. `xmlns="http://www.w3.org/2000/svg"`).
fn shorten_path_numbers(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let bytes = s.as_bytes();
    let len = bytes.len();
    let mut i = 0;

    while i < len {
        // Look for d="..." attribute — only shorten numbers inside path data
        if i + 3 < len && bytes[i] == b'd' && bytes[i + 1] == b'=' && bytes[i + 2] == b'"' {
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

    result
}

/// Remove empty class="" and id="" attributes.
fn clean_empty_attributes(s: &str) -> String {
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
    result
}

/// Trim leading/trailing whitespace and collapse multiple newlines.
fn trim_whitespace(s: &str) -> String {
    let trimmed = s.trim();
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
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_remove_xml_prolog() {
        let input = r#"<?xml version="1.0" encoding="UTF-8"?>
<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100"></svg>"#;
        let expected = r#"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100"></svg>"#;
        assert_eq!(remove_xml_prolog(input), expected);
    }

    #[test]
    fn test_remove_comments() {
        let input = r#"<svg><!-- comment --><rect/></svg>"#;
        let expected = "<svg><rect/></svg>";
        assert_eq!(remove_comments(input), expected);
    }

    #[test]
    fn test_shorten_numbers() {
        let input = r#"<path d="M1.000 2.50 3.140 4.0"/>"#;
        let expected = r#"<path d="M1 2.5 3.14 4"/>"#;
        assert_eq!(shorten_path_numbers(input), expected);
    }

    #[test]
    fn test_shorten_numbers_preserves_urls() {
        // Regression: shorten_path_numbers must NOT strip dots from URLs like xmlns
        let input =
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="100"><path d="M1.0 2.0"/></svg>"#;
        let result = shorten_path_numbers(input);
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
}
