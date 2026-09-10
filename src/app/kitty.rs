//! Minimal Kitty graphics protocol output.
//!
//! An image is transmitted once as PNG, as a *virtual placement* spanning a fixed cell
//! grid, and then drawn with unicode placeholders that the terminal replaces with the
//! image. The terminal does the scaling onto the grid, so the app never has to resample
//! and never sends more than the image's native pixels.

use ratatui::buffer::{Buffer, CellDiffOption};
use ratatui::layout::Rect;
use std::fmt::Write;
use std::num::NonZeroU16;
use std::sync::atomic::{AtomicU32, Ordering};

const PLACEHOLDER: char = '\u{10EEEE}';
/// Kitty limits each escape to 4096 bytes of payload.
const CHARS_PER_CHUNK: usize = 4096;
const BYTES_PER_CHUNK: usize = CHARS_PER_CHUNK / 4 * 3;
const UNIT_WIDTH: CellDiffOption = CellDiffOption::ForcedWidth(NonZeroU16::new(1).unwrap());

/// True when running under tmux, where graphics escapes must be wrapped in a passthrough
/// sequence (`allow-passthrough` is switched on by the terminal probe at startup).
pub fn in_tmux() -> bool {
    std::env::var("TERM").is_ok_and(|t| t.starts_with("tmux"))
        || std::env::var("TERM_PROGRAM").is_ok_and(|p| p == "tmux")
}

/// Returns a fresh image id. Ids only have to be unique within this terminal, so a
/// process-seeded counter is enough; zero is reserved by the protocol.
pub fn next_id() -> u32 {
    static NEXT: AtomicU32 = AtomicU32::new(0);
    let mut id = NEXT.fetch_add(1, Ordering::Relaxed);
    if id == 0 {
        // First call: seed from time and pid so two processes sharing a terminal differ.
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.subsec_nanos())
            .unwrap_or(0);
        id = (nanos ^ std::process::id().rotate_left(16)) | 1;
        NEXT.store(id.wrapping_add(1), Ordering::Relaxed);
    }
    id
}

/// An image the terminal holds, shown over a `cols × rows` cell grid.
pub struct Placement {
    cols: u16,
    rows: u16,
    /// Transmit sequence, sent with the first render only.
    transmit: Option<String>,
    /// Foreground colour carrying the low 24 bits of the id.
    fg: String,
    /// High byte of the id, carried by a third diacritic.
    id_extra: u16,
}

impl Placement {
    /// Prepares `png` for display over `cols × rows` cells. `replaces` is the id of the
    /// previous image, deleted in the same write so the terminal does not hoard textures.
    pub fn new(
        id: u32,
        png: &[u8],
        cols: u16,
        rows: u16,
        replaces: Option<u32>,
        tmux: bool,
    ) -> Self {
        let [id_extra, r, g, b] = id.to_be_bytes();
        Self {
            cols,
            rows,
            transmit: Some(transmit_png(png, id, cols, rows, replaces, tmux)),
            fg: format!("\x1b[38;2;{r};{g};{b}m"),
            id_extra: u16::from(id_extra),
        }
    }

    /// Draws the placeholders for the top-left `cols × rows` cells of `area`. Each row's
    /// escape sequence is stored in its first cell; the remaining cells are marked as
    /// skipped so ratatui does not overwrite the image.
    pub fn render(&mut self, area: Rect, buf: &mut Buffer) {
        let width = area.width.min(self.cols);
        let height = area.height.min(self.rows).min(DIACRITICS.len() as u16);
        if width == 0 || height == 0 {
            return;
        }
        let transmit = self.transmit.take();

        let row_fill: String = std::iter::repeat_n(PLACEHOLDER, usize::from(width) - 1).collect();
        // The placeholder run leaves the terminal cursor at the row's end, while ratatui
        // assumes it advanced by one cell. Restoring the saved cursor and parking it at the
        // area's far corner keeps the next absolute move cheap and avoids a pending wrap.
        let park = format!("\x1b[u\x1b[{}C\x1b[{}B", area.width - 1, area.height - 1);

        let mut symbol = String::with_capacity(
            transmit.as_ref().map_or(0, String::len) + self.fg.len() + row_fill.len() + 64,
        );
        for y in 0..height {
            symbol.clear();
            if y == 0 {
                if let Some(seq) = &transmit {
                    symbol.push_str(seq);
                }
            }
            write!(
                symbol,
                "\x1b[s{}{PLACEHOLDER}{}{}{}",
                self.fg,
                diacritic(y),
                diacritic(0),
                diacritic(self.id_extra)
            )
            .unwrap();
            symbol.push_str(&row_fill);
            symbol.push_str(&park);

            for x in 1..width {
                if let Some(cell) = buf.cell_mut((area.x + x, area.y + y)) {
                    cell.set_diff_option(CellDiffOption::Skip);
                }
            }
            if let Some(cell) = buf.cell_mut((area.x, area.y + y)) {
                cell.set_symbol(&symbol).set_diff_option(UNIT_WIDTH);
            }
            if y == 0 {
                symbol.shrink_to(self.fg.len() + row_fill.len() + park.len() + 32);
            }
        }
    }
}

/// Builds the escape sequence that uploads `png` as image `id` and creates a virtual
/// placement over `cols × rows` cells. Optionally deletes image `replaces` first.
pub fn transmit_png(
    png: &[u8],
    id: u32,
    cols: u16,
    rows: u16,
    replaces: Option<u32>,
    tmux: bool,
) -> String {
    let (start, esc, end) = if tmux {
        ("\x1bPtmux;", "\x1b\x1b", "\x1b\\")
    } else {
        ("", "\x1b", "")
    };

    let chunks = png.chunks(BYTES_PER_CHUNK);
    let chunk_count = chunks.len();
    let per_chunk = start.len() + esc.len() * 2 + 24 + CHARS_PER_CHUNK + end.len();
    let mut out = String::with_capacity(chunk_count * per_chunk + 128);

    if let Some(old) = replaces {
        write!(out, "{start}{esc}_Ga=d,d=I,i={old},q=2{esc}\\{end}").unwrap();
    }

    for (i, chunk) in chunks.enumerate() {
        out.push_str(start);
        write!(out, "{esc}_Gq=2,").unwrap();
        if i == 0 {
            write!(out, "a=T,U=1,t=d,f=100,i={id},c={cols},r={rows},").unwrap();
        }
        let more = u8::from(i + 1 < chunk_count);
        write!(out, "m={more};").unwrap();
        base64_encode(chunk, &mut out);
        write!(out, "{esc}\\").unwrap();
        out.push_str(end);
    }
    out
}

/// Standard base64 with padding, appended to `out`.
pub fn base64_encode(data: &[u8], out: &mut String) {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    out.reserve(data.len().div_ceil(3) * 4);
    let mut chunks = data.chunks_exact(3);
    for c in &mut chunks {
        let n = u32::from(c[0]) << 16 | u32::from(c[1]) << 8 | u32::from(c[2]);
        out.push(TABLE[(n >> 18) as usize & 63] as char);
        out.push(TABLE[(n >> 12) as usize & 63] as char);
        out.push(TABLE[(n >> 6) as usize & 63] as char);
        out.push(TABLE[n as usize & 63] as char);
    }
    let rest = chunks.remainder();
    match rest.len() {
        1 => {
            let n = u32::from(rest[0]) << 16;
            out.push(TABLE[(n >> 18) as usize & 63] as char);
            out.push(TABLE[(n >> 12) as usize & 63] as char);
            out.push_str("==");
        }
        2 => {
            let n = u32::from(rest[0]) << 16 | u32::from(rest[1]) << 8;
            out.push(TABLE[(n >> 18) as usize & 63] as char);
            out.push(TABLE[(n >> 12) as usize & 63] as char);
            out.push(TABLE[(n >> 6) as usize & 63] as char);
            out.push('=');
        }
        _ => {}
    }
}

#[inline]
fn diacritic(i: u16) -> char {
    DIACRITICS
        .get(usize::from(i))
        .copied()
        .unwrap_or(DIACRITICS[0])
}

/// Row/column diacritics from the Kitty specification (rowcolumn-diacritics.txt).
static DIACRITICS: [char; 297] = [
    '\u{305}',
    '\u{30D}',
    '\u{30E}',
    '\u{310}',
    '\u{312}',
    '\u{33D}',
    '\u{33E}',
    '\u{33F}',
    '\u{346}',
    '\u{34A}',
    '\u{34B}',
    '\u{34C}',
    '\u{350}',
    '\u{351}',
    '\u{352}',
    '\u{357}',
    '\u{35B}',
    '\u{363}',
    '\u{364}',
    '\u{365}',
    '\u{366}',
    '\u{367}',
    '\u{368}',
    '\u{369}',
    '\u{36A}',
    '\u{36B}',
    '\u{36C}',
    '\u{36D}',
    '\u{36E}',
    '\u{36F}',
    '\u{483}',
    '\u{484}',
    '\u{485}',
    '\u{486}',
    '\u{487}',
    '\u{592}',
    '\u{593}',
    '\u{594}',
    '\u{595}',
    '\u{597}',
    '\u{598}',
    '\u{599}',
    '\u{59C}',
    '\u{59D}',
    '\u{59E}',
    '\u{59F}',
    '\u{5A0}',
    '\u{5A1}',
    '\u{5A8}',
    '\u{5A9}',
    '\u{5AB}',
    '\u{5AC}',
    '\u{5AF}',
    '\u{5C4}',
    '\u{610}',
    '\u{611}',
    '\u{612}',
    '\u{613}',
    '\u{614}',
    '\u{615}',
    '\u{616}',
    '\u{617}',
    '\u{657}',
    '\u{658}',
    '\u{659}',
    '\u{65A}',
    '\u{65B}',
    '\u{65D}',
    '\u{65E}',
    '\u{6D6}',
    '\u{6D7}',
    '\u{6D8}',
    '\u{6D9}',
    '\u{6DA}',
    '\u{6DB}',
    '\u{6DC}',
    '\u{6DF}',
    '\u{6E0}',
    '\u{6E1}',
    '\u{6E2}',
    '\u{6E4}',
    '\u{6E7}',
    '\u{6E8}',
    '\u{6EB}',
    '\u{6EC}',
    '\u{730}',
    '\u{732}',
    '\u{733}',
    '\u{735}',
    '\u{736}',
    '\u{73A}',
    '\u{73D}',
    '\u{73F}',
    '\u{740}',
    '\u{741}',
    '\u{743}',
    '\u{745}',
    '\u{747}',
    '\u{749}',
    '\u{74A}',
    '\u{7EB}',
    '\u{7EC}',
    '\u{7ED}',
    '\u{7EE}',
    '\u{7EF}',
    '\u{7F0}',
    '\u{7F1}',
    '\u{7F3}',
    '\u{816}',
    '\u{817}',
    '\u{818}',
    '\u{819}',
    '\u{81B}',
    '\u{81C}',
    '\u{81D}',
    '\u{81E}',
    '\u{81F}',
    '\u{820}',
    '\u{821}',
    '\u{822}',
    '\u{823}',
    '\u{825}',
    '\u{826}',
    '\u{827}',
    '\u{829}',
    '\u{82A}',
    '\u{82B}',
    '\u{82C}',
    '\u{82D}',
    '\u{951}',
    '\u{953}',
    '\u{954}',
    '\u{F82}',
    '\u{F83}',
    '\u{F86}',
    '\u{F87}',
    '\u{135D}',
    '\u{135E}',
    '\u{135F}',
    '\u{17DD}',
    '\u{193A}',
    '\u{1A17}',
    '\u{1A75}',
    '\u{1A76}',
    '\u{1A77}',
    '\u{1A78}',
    '\u{1A79}',
    '\u{1A7A}',
    '\u{1A7B}',
    '\u{1A7C}',
    '\u{1B6B}',
    '\u{1B6D}',
    '\u{1B6E}',
    '\u{1B6F}',
    '\u{1B70}',
    '\u{1B71}',
    '\u{1B72}',
    '\u{1B73}',
    '\u{1CD0}',
    '\u{1CD1}',
    '\u{1CD2}',
    '\u{1CDA}',
    '\u{1CDB}',
    '\u{1CE0}',
    '\u{1DC0}',
    '\u{1DC1}',
    '\u{1DC3}',
    '\u{1DC4}',
    '\u{1DC5}',
    '\u{1DC6}',
    '\u{1DC7}',
    '\u{1DC8}',
    '\u{1DC9}',
    '\u{1DCB}',
    '\u{1DCC}',
    '\u{1DD1}',
    '\u{1DD2}',
    '\u{1DD3}',
    '\u{1DD4}',
    '\u{1DD5}',
    '\u{1DD6}',
    '\u{1DD7}',
    '\u{1DD8}',
    '\u{1DD9}',
    '\u{1DDA}',
    '\u{1DDB}',
    '\u{1DDC}',
    '\u{1DDD}',
    '\u{1DDE}',
    '\u{1DDF}',
    '\u{1DE0}',
    '\u{1DE1}',
    '\u{1DE2}',
    '\u{1DE3}',
    '\u{1DE4}',
    '\u{1DE5}',
    '\u{1DE6}',
    '\u{1DFE}',
    '\u{20D0}',
    '\u{20D1}',
    '\u{20D4}',
    '\u{20D5}',
    '\u{20D6}',
    '\u{20D7}',
    '\u{20DB}',
    '\u{20DC}',
    '\u{20E1}',
    '\u{20E7}',
    '\u{20E9}',
    '\u{20F0}',
    '\u{2CEF}',
    '\u{2CF0}',
    '\u{2CF1}',
    '\u{2DE0}',
    '\u{2DE1}',
    '\u{2DE2}',
    '\u{2DE3}',
    '\u{2DE4}',
    '\u{2DE5}',
    '\u{2DE6}',
    '\u{2DE7}',
    '\u{2DE8}',
    '\u{2DE9}',
    '\u{2DEA}',
    '\u{2DEB}',
    '\u{2DEC}',
    '\u{2DED}',
    '\u{2DEE}',
    '\u{2DEF}',
    '\u{2DF0}',
    '\u{2DF1}',
    '\u{2DF2}',
    '\u{2DF3}',
    '\u{2DF4}',
    '\u{2DF5}',
    '\u{2DF6}',
    '\u{2DF7}',
    '\u{2DF8}',
    '\u{2DF9}',
    '\u{2DFA}',
    '\u{2DFB}',
    '\u{2DFC}',
    '\u{2DFD}',
    '\u{2DFE}',
    '\u{2DFF}',
    '\u{A66F}',
    '\u{A67C}',
    '\u{A67D}',
    '\u{A6F0}',
    '\u{A6F1}',
    '\u{A8E0}',
    '\u{A8E1}',
    '\u{A8E2}',
    '\u{A8E3}',
    '\u{A8E4}',
    '\u{A8E5}',
    '\u{A8E6}',
    '\u{A8E7}',
    '\u{A8E8}',
    '\u{A8E9}',
    '\u{A8EA}',
    '\u{A8EB}',
    '\u{A8EC}',
    '\u{A8ED}',
    '\u{A8EE}',
    '\u{A8EF}',
    '\u{A8F0}',
    '\u{A8F1}',
    '\u{AAB0}',
    '\u{AAB2}',
    '\u{AAB3}',
    '\u{AAB7}',
    '\u{AAB8}',
    '\u{AABE}',
    '\u{AABF}',
    '\u{AAC1}',
    '\u{FE20}',
    '\u{FE21}',
    '\u{FE22}',
    '\u{FE23}',
    '\u{FE24}',
    '\u{FE25}',
    '\u{FE26}',
    '\u{10A0F}',
    '\u{10A38}',
    '\u{1D185}',
    '\u{1D186}',
    '\u{1D187}',
    '\u{1D188}',
    '\u{1D189}',
    '\u{1D1AA}',
    '\u{1D1AB}',
    '\u{1D1AC}',
    '\u{1D1AD}',
    '\u{1D242}',
    '\u{1D243}',
    '\u{1D244}',
];
