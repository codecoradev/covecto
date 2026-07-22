//! Pixel-exact vectorization engine.
//!
//! Flood-fills contiguous regions of identical RGBA color, traces their
//! boundaries, and emits compact rectilinear SVG paths (M/h/v/Z commands only).
//! Zero information loss — every pixel is exactly reproduced.

use crate::Result;
use image::RgbaImage;
use std::collections::BTreeMap;
use std::fmt::Write;

/// Vectorize an image using pixel-exact algorithm.
pub fn vectorize(img: &RgbaImage) -> Result<String> {
    let width = img.width();
    let height = img.height();
    let raw = img.as_raw();

    let get_rgba = |x: u32, y: u32| -> [u8; 4] {
        let i = ((y * width + x) * 4) as usize;
        [raw[i], raw[i + 1], raw[i + 2], raw[i + 3]]
    };

    let mut visited = vec![false; (width * height) as usize];

    // (offset_to_neighbor, (edge_start_corner, edge_end_corner))
    let edges_offsets = [
        ((-1i32, 0i32), ((0u32, 0u32), (0u32, 1u32))),
        ((0, 1), ((0, 1), (1, 1))),
        ((1, 0), ((1, 1), (1, 0))),
        ((0, -1), ((1, 0), (0, 0))),
    ];

    let mut queue = Vec::new();
    let mut current_edges = Vec::new();
    let mut used = Vec::new();
    let mut piece = Vec::new();
    // BTreeMap keeps emission order deterministic across runs
    let mut paths_by_color: BTreeMap<[u8; 4], (String, (i32, i32))> = BTreeMap::new();

    for y in 0..height {
        for x in 0..width {
            let idx = (y * width + x) as usize;
            if visited[idx] {
                continue;
            }

            let colors = get_rgba(x, y);
            if colors[3] == 0 {
                visited[idx] = true;
                continue;
            }

            queue.clear();
            queue.push((x as i32, y as i32));
            visited[idx] = true;

            current_edges.clear();

            while let Some(here) = queue.pop() {
                for &(offset, (start_offset, end_offset)) in &edges_offsets {
                    let nx = here.0 + offset.0;
                    let ny = here.1 + offset.1;

                    let is_boundary =
                        if nx < 0 || nx >= width as i32 || ny < 0 || ny >= height as i32 {
                            true
                        } else {
                            let (nx_u, ny_u) = (nx as u32, ny as u32);
                            if get_rgba(nx_u, ny_u) == colors {
                                let n_idx = (ny_u * width + nx_u) as usize;
                                if !visited[n_idx] {
                                    visited[n_idx] = true;
                                    queue.push((nx, ny));
                                }
                                false
                            } else {
                                true
                            }
                        };

                    if is_boundary {
                        let start = (
                            here.0 + start_offset.0 as i32,
                            here.1 + start_offset.1 as i32,
                        );
                        let end = (here.0 + end_offset.0 as i32, here.1 + end_offset.1 as i32);
                        current_edges.push((start, end));
                    }
                }
            }

            if current_edges.is_empty() {
                continue;
            }

            current_edges.sort_unstable();

            used.clear();
            used.resize(current_edges.len(), false);

            let directions = [(0i32, 1i32), (1, 0), (0, -1), (-1, 0)];
            let (buf, last_start) = paths_by_color.entry(colors).or_default();

            for i in 0..current_edges.len() {
                if used[i] {
                    continue;
                }
                used[i] = true;
                let first_edge = current_edges[i];

                piece.clear();
                piece.push(first_edge.0);
                piece.push(first_edge.1);

                loop {
                    let last_point = *piece.last().unwrap();
                    let mut found = false;

                    for &direction in &directions {
                        let next_point = (last_point.0 + direction.0, last_point.1 + direction.1);
                        let next_edge = (last_point, next_point);

                        if let Ok(idx) = current_edges.binary_search(&next_edge)
                            && !used[idx]
                        {
                            used[idx] = true;

                            // Collapse consecutive same-direction moves
                            if piece.len() >= 2 {
                                let prev_direction = (
                                    piece[piece.len() - 1].0 - piece[piece.len() - 2].0,
                                    piece[piece.len() - 1].1 - piece[piece.len() - 2].1,
                                );
                                if prev_direction == direction {
                                    piece.pop();
                                }
                            }
                            piece.push(next_point);
                            found = true;
                            break;
                        }
                    }

                    if !found || piece.first() == piece.last() {
                        break;
                    }
                }

                // Close path if looped back
                if piece.first() == piece.last() {
                    piece.pop();
                }

                if piece.is_empty() {
                    continue;
                }

                // Write moveto
                let (sx, sy) = piece[0];
                if buf.is_empty() {
                    let _ = write!(buf, "M{},{}", sx, sy);
                } else {
                    let dx = sx - last_start.0;
                    let dy = sy - last_start.1;
                    if dy < 0 {
                        let _ = write!(buf, "m{}{}", dx, dy);
                    } else {
                        let _ = write!(buf, "m{},{}", dx, dy);
                    }
                }
                *last_start = (sx, sy);

                // Write line segments
                let mut prev = piece[0];
                for &p in &piece[1..] {
                    let dx = p.0 - prev.0;
                    let dy = p.1 - prev.1;
                    if dy == 0 {
                        let _ = write!(buf, "h{}", dx);
                    } else {
                        let _ = write!(buf, "v{}", dy);
                    }
                    prev = p;
                }
                buf.push('Z');
            }
        }
    }

    // Assemble SVG
    let mut svg = String::with_capacity((width * height * 3) as usize);
    svg.push_str(&svg_header(width, height));

    // Group translucent paths by alpha
    let mut translucent_by_alpha: BTreeMap<u8, Vec<([u8; 4], String)>> = BTreeMap::new();

    for (color, (data, _)) in paths_by_color {
        let [r, g, b, a] = color;
        if a == 255 {
            let _ = write!(
                svg,
                r##"<path fill="#{:02x}{:02x}{:02x}" d="{}"/>"##,
                r, g, b, data
            );
        } else {
            translucent_by_alpha
                .entry(a)
                .or_default()
                .push((color, data));
        }
    }

    for (a, paths) in &translucent_by_alpha {
        let opacity = f32::from(*a) / 255.0;
        let _ = write!(svg, r##"<g fill-opacity="{:.3}">"##, opacity);
        for ([r, g, b, _], data) in paths {
            let _ = write!(
                svg,
                r##"<path fill="#{:02x}{:02x}{:02x}" d="{}"/>"##,
                r, g, b, data
            );
        }
        svg.push_str("</g>");
    }

    svg.push_str("</svg>\n");
    Ok(svg)
}

fn svg_header(width: u32, height: u32) -> String {
    format!(
        r#"<svg width="{}" height="{}" xmlns="http://www.w3.org/2000/svg">"#,
        width, height
    )
}