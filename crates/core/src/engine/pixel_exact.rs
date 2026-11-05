//! Pixel-exact vectorization engine.
//!
//! Flood-fills contiguous regions of identical RGBA color, traces their
//! boundaries, and emits compact rectilinear SVG paths (M/h/v/Z commands only).
//! Zero information loss — every pixel is exactly reproduced.

use crate::Result;
use image::RgbaImage;
use std::collections::BTreeMap;
use std::fmt::Write;
use std::io::Write as _;

/// Vectorize an image using pixel-exact algorithm.
pub fn vectorize(img: &RgbaImage) -> Result<String> {
    let mut buf = Vec::with_capacity(img.width() as usize * img.height() as usize * 3);
    vectorize_to(&mut buf, img)?;
    // SVG output is always valid UTF-8 (ASCII path commands + hex colors)
    Ok(unsafe { String::from_utf8_unchecked(buf) })
}

/// Metadata returned by streaming vectorize (without the SVG string).
#[derive(Debug, Clone)]
pub struct VectorizeMeta {
    pub byte_count: usize,
    pub path_count: usize,
}

/// Vectorize an image using pixel-exact algorithm, writing SVG directly to a writer.
/// This avoids building the full SVG string in memory.
pub fn vectorize_to<W: std::io::Write>(writer: &mut W, img: &RgbaImage) -> Result<VectorizeMeta> {
    let width = img.width();
    let height = img.height();
    let raw = img.as_raw();

    let get_rgba = |x: u32, y: u32| -> [u8; 4] {
        let i = (y as usize * width as usize + x as usize) * 4;
        [raw[i], raw[i + 1], raw[i + 2], raw[i + 3]]
    };

    let mut visited = BitSet::new(width as usize * height as usize);

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
            let idx = y as usize * width as usize + x as usize;
            if visited.get(idx) {
                continue;
            }

            let colors = get_rgba(x, y);
            if colors[3] == 0 {
                visited.set(idx);
                continue;
            }

            queue.clear();
            queue.push((x as i32, y as i32));
            visited.set(idx);

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
                                let n_idx = ny_u as usize * width as usize + nx_u as usize;
                                if !visited.get(n_idx) {
                                    visited.set(n_idx);
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

                        match current_edges.binary_search(&next_edge) {
                            Ok(idx) if !used[idx] => {
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
                            _ => {}
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

    // Stream SVG output through a counting adapter so metadata can't drift
    // from what was actually written.
    let mut writer = CountingWriter {
        inner: writer,
        bytes: 0,
        paths: 0,
    };

    // SVG header
    writer.write_all(svg_header(width, height).as_bytes())?;

    // Group translucent paths by alpha (borrowing the path data, no copies)
    let mut translucent_by_alpha: BTreeMap<u8, Vec<([u8; 4], &str)>> = BTreeMap::new();

    // One reusable buffer per element; each element is still emitted with a
    // single `write_all`, so the `<path` counter never sees a split tag.
    let mut line: Vec<u8> = Vec::with_capacity(256);
    let mut write_path = |writer: &mut CountingWriter<'_, W>,
                          [r, g, b, _]: [u8; 4],
                          data: &str|
     -> std::io::Result<()> {
        line.clear();
        write!(
            line,
            r##"<path fill="#{r:02x}{g:02x}{b:02x}" d="{data}"/>"##
        )?;
        writer.write_all(&line)
    };

    for (color, (data, _)) in &paths_by_color {
        if color[3] == 255 {
            write_path(&mut writer, *color, data)?;
        } else {
            translucent_by_alpha
                .entry(color[3])
                .or_default()
                .push((*color, data.as_str()));
        }
    }

    for (a, paths) in &translucent_by_alpha {
        let opacity = f32::from(*a) / 255.0;
        let open = format!(r#"<g fill-opacity="{:.3}">"#, opacity);
        writer.write_all(open.as_bytes())?;
        for &(color, data) in paths {
            write_path(&mut writer, color, data)?;
        }
        writer.write_all(b"</g>")?;
    }

    writer.write_all(b"</svg>\n")?;

    Ok(VectorizeMeta {
        byte_count: writer.bytes,
        path_count: writer.paths,
    })
}

/// Compact visited-set: one bit per pixel (8x smaller than `Vec<bool>`).
struct BitSet {
    words: Vec<u64>,
}

impl BitSet {
    fn new(len: usize) -> Self {
        Self {
            words: vec![0; len.div_ceil(64)],
        }
    }

    #[inline]
    fn get(&self, i: usize) -> bool {
        self.words[i / 64] & (1 << (i % 64)) != 0
    }

    #[inline]
    fn set(&mut self, i: usize) {
        self.words[i / 64] |= 1 << (i % 64);
    }
}

/// Counts bytes and `<path` elements as they are written. Every emitted element
/// goes through one `write_all` call, so a `<path` tag is never split.
struct CountingWriter<'a, W: std::io::Write> {
    inner: &'a mut W,
    bytes: usize,
    paths: usize,
}

impl<W: std::io::Write> std::io::Write for CountingWriter<'_, W> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.write_all(buf)?;
        Ok(buf.len())
    }

    fn write_all(&mut self, buf: &[u8]) -> std::io::Result<()> {
        self.inner.write_all(buf)?;
        self.bytes += buf.len();
        self.paths += buf.windows(5).filter(|w| *w == b"<path").count();
        Ok(())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.inner.flush()
    }
}

fn svg_header(width: u32, height: u32) -> String {
    format!(
        r#"<svg width="{}" height="{}" xmlns="http://www.w3.org/2000/svg">"#,
        width, height
    )
}
