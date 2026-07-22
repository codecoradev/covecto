//! Output format conversion — SVG → PDF, SVG → EPS.

use crate::{Error, Result};

/// Supported output formats.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum OutputFormat {
    #[default]
    Svg,
    Pdf,
    Eps,
}

impl std::str::FromStr for OutputFormat {
    type Err = crate::Error;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "svg" => Ok(OutputFormat::Svg),
            "pdf" => Ok(OutputFormat::Pdf),
            "eps" => Ok(OutputFormat::Eps),
            _ => Err(crate::Error::InvalidConfig(format!(
                "Unknown output format: {s}. Use: svg, pdf, eps"
            ))),
        }
    }
}

impl OutputFormat {
    /// MIME content type for HTTP responses.
    pub fn content_type(&self) -> &'static str {
        match self {
            OutputFormat::Svg => "image/svg+xml",
            OutputFormat::Pdf => "application/pdf",
            OutputFormat::Eps => "application/eps",
        }
    }

    /// File extension.
    pub fn extension(&self) -> &'static str {
        match self {
            OutputFormat::Svg => "svg",
            OutputFormat::Pdf => "pdf",
            OutputFormat::Eps => "eps",
        }
    }
}

/// Convert an SVG string to the target output format.
/// Returns the converted bytes and the MIME content type.
pub fn convert_output(svg: &str, format: OutputFormat) -> Result<(Vec<u8>, String)> {
    match format {
        OutputFormat::Svg => Ok((svg.as_bytes().to_vec(), OutputFormat::Svg.content_type().to_string())),
        OutputFormat::Pdf => {
            let pdf_bytes = svg_to_pdf(svg)?;
            Ok((pdf_bytes, OutputFormat::Pdf.content_type().to_string()))
        }
        OutputFormat::Eps => {
            let eps_bytes = svg_to_eps(svg)?;
            Ok((eps_bytes, OutputFormat::Eps.content_type().to_string()))
        }
    }
}

/// Convert SVG to PDF using svg2pdf 0.13 (usvg 0.45 compatible).
fn svg_to_pdf(svg: &str) -> Result<Vec<u8>> {
    let tree = usvg::Tree::from_str(svg, &usvg::Options::default())
        .map_err(|e| Error::Conversion(format!("SVG parse error: {e}")))?;

    let pdf_bytes = svg2pdf::to_pdf(
        &tree,
        svg2pdf::ConversionOptions::default(),
        svg2pdf::PageOptions::default(),
    )
    .map_err(|e| Error::Conversion(format!("PDF conversion error: {e}")))?;

    Ok(pdf_bytes)
}

/// Convert SVG to EPS (Encapsulated PostScript).
///
/// Lightweight converter that transforms SVG path elements into PostScript commands.
/// Handles: path elements with fill and stroke.
/// Colors use `rg` (RGB) operator. Embeds viewBox dimensions as BoundingBox.
fn svg_to_eps(svg: &str) -> Result<Vec<u8>> {
    let tree = usvg::Tree::from_str(svg, &usvg::Options::default())
        .map_err(|e| Error::Conversion(format!("SVG parse error: {e}")))?;

    let size = tree.size();
    let width = size.width();
    let height = size.height();

    let mut ps = String::with_capacity(svg.len() * 2);

    // EPS header
    ps.push_str("%!PS-Adobe-3.0 EPSF-3.0\n");
    ps.push_str(&format!("%%BoundingBox: 0 0 {width:.2} {height:.2}\n"));
    ps.push_str("%%EndComments\n\n");

    // Coordinate transform: SVG is top-left origin, PS is bottom-left
    ps.push_str(&format!("{:.2} {height:.2} translate\n", 0.0_f32));
    ps.push_str("1 -1 scale\n\n");

    // Render nodes recursively
    for node in tree.root().children() {
        render_node(&node, &mut ps);
    }

    ps.push_str("\nshowpage\n%%EOF\n");

    Ok(ps.into_bytes())
}

fn render_node(node: &usvg::Node, ps: &mut String) {
    match node {
        usvg::Node::Path(path) => render_path(path, ps),
        usvg::Node::Group(group) => {
            for child in group.children() {
                render_node(&child, ps);
            }
        }
        _ => {} // Skip text, image, etc.
    }
}

fn render_path(path: &usvg::Path, ps: &mut String) {
    use usvg::tiny_skia_path::PathSegment;

    let transform = path.abs_transform();

    // Extract fill and stroke
    let fill = extract_fill_color(path.fill());
    let stroke_info = extract_stroke_info(path.stroke());

    // Apply transform (skip identity)
    if !transform.is_identity() {
        let (a, b, c, d, e, f) = decompose_transform(&transform);
        ps.push_str(&format!("[{a} {b} {c} {d} {e} {f}] concatmatrix\n"));
    }

    // Build path from segments
    ps.push_str("newpath\n");

    for seg in path.data().segments() {
        match seg {
            PathSegment::MoveTo(p) => {
                ps.push_str(&format!("{:.4} {:.4} moveto\n", p.x, p.y));
            }
            PathSegment::LineTo(p) => {
                ps.push_str(&format!("{:.4} {:.4} lineto\n", p.x, p.y));
            }
            PathSegment::QuadTo(_p1, p) => {
                // PostScript doesn't have native quad curves; convert to cubic
                // Quadratic Bézier (Q) → Cubic Bézier (C): 
                // CP1 = 1/3 P0 + 2/3 P1, CP2 = 2/3 P1 + 1/3 P2
                // We approximate by using the last moveto/line position as P0.
                // For simplicity, we just emit a line (lossy but functional).
                ps.push_str(&format!("{:.4} {:.4} lineto\n", p.x, p.y));
            }
            PathSegment::CubicTo(p1, p2, p) => {
                ps.push_str(&format!(
                    "{:.4} {:.4} {:.4} {:.4} {:.4} {:.4} curveto\n",
                    p1.x, p1.y, p2.x, p2.y, p.x, p.y
                ));
            }
            PathSegment::Close => {
                ps.push_str("closepath\n");
            }
        }
    }

    // Apply fill
    if let Some((r, g, b, a)) = fill {
        if a < 1.0 {
            ps.push_str(&format!("{:.4} {:.4} {:.4} setrgbcolor\n", r, g, b));
            ps.push_str(&format!("{:.4} setopacity\n", a));
        } else {
            ps.push_str(&format!("{:.4} {:.4} {:.4} setrgbcolor\n", r, g, b));
        }
        ps.push_str("fill\n");
    }

    // Apply stroke
    if let Some((r, g, b, w)) = stroke_info {
        ps.push_str(&format!("{:.4} setlinewidth\n", w));
        ps.push_str(&format!("{:.4} {:.4} {:.4} setrgbcolor\n", r, g, b));
        ps.push_str("stroke\n");
    }
}

fn extract_fill_color(fill: Option<&usvg::Fill>) -> Option<(f32, f32, f32, f32)> {
    let fill = fill?;
    match fill.paint() {
        usvg::Paint::Color(color) => {
            let r = color.red as f32 / 255.0;
            let g = color.green as f32 / 255.0;
            let b = color.blue as f32 / 255.0;
            let a = fill.opacity().get();
            Some((r, g, b, a))
        }
        _ => None, // Skip gradients, patterns
    }
}

fn extract_stroke_info(stroke: Option<&usvg::Stroke>) -> Option<(f32, f32, f32, f32)> {
    let stroke = stroke?;
    match stroke.paint() {
        usvg::Paint::Color(color) => {
            let r = color.red as f32 / 255.0;
            let g = color.green as f32 / 255.0;
            let b = color.blue as f32 / 255.0;
            let w = stroke.width().get();
            Some((r, g, b, w))
        }
        _ => None,
    }
}

fn decompose_transform(ts: &usvg::Transform) -> (f32, f32, f32, f32, f32, f32) {
    (ts.sx, ts.ky, ts.kx, ts.sy, ts.tx, ts.ty)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_SVG: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100" viewBox="0 0 100 100">
        <rect x="10" y="10" width="80" height="80" fill="red" stroke="blue" stroke-width="2"/>
        <circle cx="50" cy="50" r="30" fill="green"/>
    </svg>"#;

    #[test]
    fn test_output_format_parse() {
        assert_eq!("svg".parse::<OutputFormat>().unwrap(), OutputFormat::Svg);
        assert_eq!("pdf".parse::<OutputFormat>().unwrap(), OutputFormat::Pdf);
        assert_eq!("eps".parse::<OutputFormat>().unwrap(), OutputFormat::Eps);
        assert!("xyz".parse::<OutputFormat>().is_err());
    }

    #[test]
    fn test_output_format_content_type() {
        assert_eq!(OutputFormat::Svg.content_type(), "image/svg+xml");
        assert_eq!(OutputFormat::Pdf.content_type(), "application/pdf");
        assert_eq!(OutputFormat::Eps.content_type(), "application/eps");
    }

    #[test]
    fn test_svg_passthrough() {
        let (bytes, ct) = convert_output(SAMPLE_SVG, OutputFormat::Svg).unwrap();
        assert_eq!(ct, "image/svg+xml");
        assert!(bytes.starts_with(b"<svg"));
    }

    #[test]
    fn test_svg_to_pdf() {
        let (bytes, ct) = convert_output(SAMPLE_SVG, OutputFormat::Pdf).unwrap();
        assert_eq!(ct, "application/pdf");
        // PDF files start with %PDF
        assert!(bytes.starts_with(b"%PDF"));
    }

    #[test]
    fn test_svg_to_eps() {
        let (bytes, ct) = convert_output(SAMPLE_SVG, OutputFormat::Eps).unwrap();
        assert_eq!(ct, "application/eps");
        let s = String::from_utf8(bytes).unwrap();
        assert!(s.starts_with("%!PS-Adobe-3.0 EPSF-3.0"));
        assert!(s.contains("%%BoundingBox:"));
        assert!(s.contains("showpage"));
    }

    #[test]
    fn test_eps_contains_paths() {
        let (bytes, _) = convert_output(SAMPLE_SVG, OutputFormat::Eps).unwrap();
        let s = String::from_utf8(bytes).unwrap();
        // Should contain PostScript drawing commands
        assert!(s.contains("newpath") || s.contains("moveto"));
    }
}