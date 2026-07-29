//! F-FMT step 2 (`DE_PASCALIZE_PLAN.md` Part IV.2 §F-FMT): the fixed-width
//! `Show` reports assembled as **row data**, rendered by the lane.
//!
//! Pascal's `ShowResults.pas` writes its tables by hand — `Pad(S, W)`,
//! `PadDots(S, W)` and `Format('%W.Df', …)` interleaved with literal spaces and
//! commas — so every report module carried its own column arithmetic. This
//! module holds that arithmetic **once**, as data:
//!
//! * a [`Cell`] is one field — its text, the Pascal field width, which padding
//!   primitive fills it, and the literal separator that follows it on the line;
//! * a [`Row`] is a line's worth of cells;
//! * a [`Report`] interleaves free text (titles, rulers, blank lines) with runs
//!   of rows, and renders each run through the lane's kernel.
//!
//! # The two kernels
//!
//! * parity — [`render_rows_pad_impl`]: replays the Pascal primitives in order,
//!   so the emitted bytes are exactly what the hand-built `String` produced.
//! * default — [`render_rows_table_impl`]: hands the run to `comfy-table` with
//!   the `NOTHING` preset (no borders, no colour, no wrapping) and lets it size
//!   each column from its own content.
//!
//! Both are always compiled; `crate::compat::render_rows` selects one
//! (the Stage F mechanism — see `compat`'s module doc).
//!
//! # Why the default kernel cannot lose a token
//!
//! The default-lane goldens are compared **parsed-numeric** against the same
//! committed oracle captures (`tests/harness/lane.rs`), which tokenizes on
//! whitespace and commas. A table renderer that dropped a field, or merged two,
//! would fail that compare — but only on whichever report a fixture happens to
//! cover. The model closes the hole structurally instead:
//!
//! * **every token lives in a cell.** [`Cell::sep`] rejects anything that is not
//!   whitespace or a comma, i.e. exactly the characters the comparator treats as
//!   field separators. So the separators the default kernel discards are, by
//!   construction, characters that carry no token. A separator that tried to
//!   smuggle content (`" kW"`) panics at the call site.
//! * **no cell may contain a line break**, so a row is always one line in both
//!   kernels.
//!
//! What remains lane-visible is padding — and the one case where padding is
//! *not* cosmetic: a text that overflows its Pascal width glues onto its
//! neighbour when the separator is empty (`Show BusFlow`'s
//! `Pad(name, 0 + 2) + IntToStr(term)`, F.4b). The parity kernel reproduces the
//! glue; the default kernel's columns are sized from content, so it never glues.
//! That difference is a *lane row*, already enumerated at the goldens that
//! observe it, and `tests::overflow_glue_is_the_only_token_difference` pins
//! that it is the only shape in which the two kernels' token streams can differ.

use std::borrow::Cow;

use crate::report::format;

/// Which Pascal padding primitive fills a [`Cell`]'s field.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Fill {
    /// Pascal `Pad(S, W)` (`Common/Utilities.pas`): left-justified, **space**
    /// filled; a text already `>= W` is emitted unchanged.
    Left,
    /// Pascal `PadDots(S, W)` (`Common/ShowResults.pas`): left-justified, filled
    /// from `' ................…'` — one leading space, then dots.
    Dots,
    /// Pascal `Format('%W.Df' / '%Wd' / '%W.Pg', …)`: right-justified in `W`
    /// spaces.
    Right,
}

/// One field of a `Show` row: the token(s) it carries, the Pascal field width it
/// occupies, how that width is filled, and the literal separator that follows it
/// on the Pascal line.
#[derive(Clone, Debug)]
pub struct Cell {
    text: String,
    width: usize,
    fill: Fill,
    sep: Cow<'static, str>,
}

impl Cell {
    /// A `Pad(text, width)` field.
    pub fn left(text: impl Into<String>, width: usize) -> Self {
        Self::build(text.into(), width, Fill::Left)
    }

    /// A `PadDots(text, width)` field.
    pub fn dots(text: impl Into<String>, width: usize) -> Self {
        Self::build(text.into(), width, Fill::Dots)
    }

    /// A right-justified `Format('%W…', …)` field.
    pub fn right(text: impl Into<String>, width: usize) -> Self {
        Self::build(text.into(), width, Fill::Right)
    }

    /// A field with no width of its own — the last column of a row, or a Pascal
    /// `%s` with no width prefix.
    pub fn plain(text: impl Into<String>) -> Self {
        Self::build(text.into(), 0, Fill::Left)
    }

    /// The literal that follows this cell on the Pascal line.
    ///
    /// **Separators may only contain whitespace and commas** — the characters
    /// the golden comparator splits fields on. Content in a separator would be
    /// dropped by the table kernel (which supplies its own gutters), so putting
    /// it there is a call-site bug and panics here rather than silently losing a
    /// token in one lane.
    pub fn sep(mut self, sep: impl Into<Cow<'static, str>>) -> Self {
        let sep = sep.into();
        assert!(
            sep.chars().all(|c| c.is_whitespace() || c == ','),
            "a Show-table separator carries no token, so it may only hold \
             whitespace and commas — {sep:?} must be a Cell of its own"
        );
        assert!(
            !sep.contains('\n'),
            "a Show-table separator may not break the line: {sep:?}"
        );
        self.sep = sep;
        self
    }

    /// The text this cell contributes to the report, without padding.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Whether the cell is right-justified in its field.
    pub fn is_right(&self) -> bool {
        self.fill == Fill::Right
    }

    /// This cell as the Pascal primitive wrote it: the text padded to its field
    /// width, followed by its separator.
    fn padded(&self) -> String {
        let body = match self.fill {
            Fill::Left => format::pad(&self.text, self.width),
            Fill::Dots => format::pad_dots(&self.text, self.width),
            Fill::Right => format!("{:>1$}", self.text, self.width),
        };
        format!("{body}{}", self.sep)
    }

    fn build(text: String, width: usize, fill: Fill) -> Self {
        assert!(
            !text.contains('\n'),
            "a Show-table cell is one line: {text:?}"
        );
        Self {
            text,
            width,
            fill,
            sep: Cow::Borrowed(""),
        }
    }
}

/// One line of a fixed-width `Show` table.
#[derive(Clone, Debug, Default)]
pub struct Row {
    cells: Vec<Cell>,
}

impl Row {
    /// An empty row, to be filled by [`Row::cell`].
    pub fn new() -> Self {
        Self::default()
    }

    /// A blank line **inside** a run of rows: `ncols` width-less empty cells.
    ///
    /// Both kernels render it as an empty line — the parity one because every
    /// cell is width 0, the table one because trailing padding is trimmed — and
    /// because the cells carry no text they widen no column. This is what lets a
    /// report keep its Pascal blank lines without breaking the run into
    /// separately-sized tables (a `Report::blank` would).
    pub fn blank(ncols: usize) -> Self {
        Self {
            cells: (0..ncols).map(|_| Cell::plain("")).collect(),
        }
    }

    /// Append a cell (builder form).
    #[must_use]
    pub fn cell(mut self, c: Cell) -> Self {
        self.cells.push(c);
        self
    }

    /// The row's cells, in column order.
    pub fn cells(&self) -> &[Cell] {
        &self.cells
    }
}

/// Replay the Pascal padding primitives — the **parity kernel** of the `Show`
/// table layout ([`crate::compat::render_rows`]).
///
/// Each cell is padded to its own field width by the primitive that declared it
/// and followed by its literal separator, so the bytes are exactly those the
/// hand-built `Format`/`Pad` concatenation produced.
pub fn render_rows_pad_impl(rows: &[Row]) -> String {
    let mut out = String::new();
    for row in rows {
        for c in &row.cells {
            out.push_str(&c.padded());
        }
        out.push('\n');
    }
    out
}

/// Size every column from its own content — the **default kernel** of the
/// `Show` table layout ([`crate::compat::render_rows`]), F-FMT step 2's table
/// crate.
///
/// `comfy-table` 7 with the `NOTHING` preset: no borders, no separator lines, no
/// colour, and [`ContentArrangement::Disabled`] so a wide cell widens its column
/// instead of wrapping onto a second line (wrapping would split a row's tokens
/// across lines). Columns carry no left padding and a two-space gutter, and
/// trailing padding is trimmed, so the block has no ragged right edge.
///
/// [`ContentArrangement::Disabled`]: comfy_table::ContentArrangement::Disabled
pub fn render_rows_table_impl(rows: &[Row]) -> String {
    use comfy_table::{Cell as TCell, CellAlignment, ContentArrangement, Table, presets::NOTHING};

    if rows.is_empty() {
        return String::new();
    }
    let mut table = Table::new();
    table.load_preset(NOTHING);
    table.set_content_arrangement(ContentArrangement::Disabled);
    for row in rows {
        table.add_row(row.cells.iter().map(|c| {
            TCell::new(&c.text).set_alignment(if c.is_right() {
                CellAlignment::Right
            } else {
                CellAlignment::Left
            })
        }));
    }
    for col in table.column_iter_mut() {
        col.set_padding((0, 2));
    }
    let mut out = String::new();
    for line in table.to_string().split('\n') {
        out.push_str(line.trim_end());
        out.push('\n');
    }
    out
}

/// A `Show` report under construction: free text (titles, rulers, blank lines)
/// interleaved with runs of [`Row`]s.
///
/// Each **contiguous** run of rows is one table: it is handed to the lane's
/// kernel ([`crate::compat::render_rows`]) when the next free text arrives, or
/// at [`Report::finish`]. So a report's sections size their columns
/// independently, exactly as the Pascal per-section `Pad` widths did.
#[derive(Debug, Default)]
pub struct Report {
    out: String,
    pending: Vec<Row>,
}

impl Report {
    /// An empty report.
    pub fn new() -> Self {
        Self::default()
    }

    /// Append verbatim text (it must carry its own line breaks), closing any
    /// open run of rows first.
    pub fn text(&mut self, s: &str) {
        self.flush();
        self.out.push_str(s);
    }

    /// Append one verbatim line, closing any open run of rows first.
    pub fn line(&mut self, s: &str) {
        self.text(s);
        self.out.push('\n');
    }

    /// Append an empty line.
    pub fn blank(&mut self) {
        self.line("");
    }

    /// Append a table row to the open run.
    pub fn row(&mut self, r: Row) {
        self.pending.push(r);
    }

    /// The finished report text.
    pub fn finish(mut self) -> String {
        self.flush();
        self.out
    }

    /// Render and append the open run of rows, if any.
    fn flush(&mut self) {
        if self.pending.is_empty() {
            return;
        }
        let rows = std::mem::take(&mut self.pending);
        self.out.push_str(&crate::compat::render_rows(&rows));
    }
}

#[cfg(test)]
mod tests {
    use super::{Cell, Report, Row, render_rows_pad_impl, render_rows_table_impl};

    /// The golden comparator's field split, mirrored: whitespace **or** comma,
    /// dropping empties and pure dot-runs (`harness::split_fields`, `sep == ' '`
    /// — the mode every `Show` golden uses). The two kernels are equivalent
    /// exactly when they agree here.
    fn tokens(text: &str) -> Vec<Vec<String>> {
        text.lines()
            .map(|l| l.trim_end().to_string())
            .filter(|l| !l.trim().is_empty())
            .map(|l| {
                l.split(|c: char| c.is_whitespace() || c == ',')
                    .filter(|f| !f.is_empty() && !f.bytes().all(|b| b == b'.'))
                    .map(str::to_string)
                    .collect()
            })
            .collect()
    }

    /// A row in the shape `Show Losses` writes: a `Pad`ded quoted name, two
    /// right-justified numeric fields separated by `", "`, and a trailing `%g`.
    fn losses_row(name: &str, kw: &str, pct: &str, kvar: &str) -> Row {
        Row::new()
            .cell(Cell::left(format!("\"{name}\""), 16))
            .cell(Cell::right(kw, 10).sep(", "))
            .cell(Cell::right(pct, 8).sep("     "))
            .cell(Cell::plain(kvar))
    }

    /// The parity kernel is the Pascal primitives, byte for byte: `Pad` fills
    /// with spaces and never truncates, `PadDots` fills from the leading-space
    /// dot string, a right field is space-justified, and each separator follows
    /// its own cell verbatim.
    #[test]
    fn pad_kernel_replays_the_pascal_primitives() {
        let rows = [
            Row::new()
                .cell(Cell::left("bus1", 8))
                .cell(Cell::dots("650", 12))
                .cell(Cell::right("1.5", 7).sep(", "))
                .cell(Cell::plain("tail")),
            // `Pad` never truncates: an over-wide text is emitted as-is and the
            // next field glues onto it (the `Show BusFlow` shape).
            Row::new()
                .cell(Cell::left("a_very_wide_name", 8))
                .cell(Cell::plain("1")),
        ];
        assert_eq!(
            render_rows_pad_impl(&rows),
            "bus1    650 ........    1.5, tail\na_very_wide_name1\n"
        );
    }

    /// The default kernel sizes each column from its own content: the widest
    /// cell sets the column, shorter cells are justified per their alignment,
    /// the gutter is two spaces and no line keeps trailing padding.
    #[test]
    fn table_kernel_sizes_columns_from_content() {
        let rows = [
            losses_row("Line.a", "1.5", "12.25", "0.5"),
            losses_row("Transformer.long_one", "-1234.5", "7.0", "-0.25"),
        ];
        assert_eq!(
            render_rows_table_impl(&rows),
            concat!(
                "\"Line.a\"                    1.5  12.25  0.5\n",
                "\"Transformer.long_one\"  -1234.5    7.0  -0.25\n",
            )
        );
    }

    /// The whole point of the model: for every row shape the reports build, the
    /// two kernels produce the **same token stream** — the padding moves, the
    /// fields do not. Asserted per row *and* per line, so a merged or dropped
    /// field fails.
    #[test]
    fn both_kernels_tokenize_alike() {
        let rows = vec![
            // header row (multi-word cells) …
            Row::new()
                .cell(Cell::left("Element", 25))
                .cell(Cell::left("kW Losses", 13))
                .cell(Cell::left("% of Power", 13))
                .cell(Cell::plain("kvar Losses")),
            losses_row("Line.a", "1.5", "12.25", "0.5"),
            losses_row("Transformer.long_one", "-1234.5", "7.0", "-0.25"),
            // … a dot-padded name, an empty cell, a comma separator, and a
            // negative exponent-notation number.
            Row::new()
                .cell(Cell::dots("650", 12))
                .cell(Cell::right("", 6).sep(","))
                .cell(Cell::right("-1.19304E-6", 12).sep(",  "))
                .cell(Cell::plain("0")),
        ];
        assert_eq!(
            tokens(&render_rows_pad_impl(&rows)),
            tokens(&render_rows_table_impl(&rows)),
            "the two Show-table kernels must carry the same fields"
        );
    }

    /// The one shape in which the kernels' tokens *can* differ, isolated: a text
    /// wider than its field with an **empty** separator glues onto the next cell
    /// in the parity kernel (Pascal `Pad` does not truncate) and cannot glue in
    /// the table kernel, which sizes the column from that very text. Give the
    /// cell any separator at all and the two agree again.
    #[test]
    fn overflow_glue_is_the_only_token_difference() {
        let glued = [Row::new()
            .cell(Cell::left("\"Capacitor.cap1\"", 2))
            .cell(Cell::plain("1"))];
        assert_eq!(
            render_rows_pad_impl(&glued),
            "\"Capacitor.cap1\"1\n",
            "the parity kernel reproduces the glue"
        );
        assert_ne!(
            tokens(&render_rows_pad_impl(&glued)),
            tokens(&render_rows_table_impl(&glued)),
            "…and the table kernel, sizing the column from the name, cannot"
        );

        let spaced = [Row::new()
            .cell(Cell::left("\"Capacitor.cap1\"", 2).sep(" "))
            .cell(Cell::plain("1"))];
        assert_eq!(
            tokens(&render_rows_pad_impl(&spaced)),
            tokens(&render_rows_table_impl(&spaced)),
            "a separated overflow is token-identical in both kernels"
        );
    }

    /// A separator carries no token, so it may not carry content: the guard
    /// fires at the call site instead of letting the table kernel drop the text.
    #[test]
    #[should_panic(expected = "must be a Cell of its own")]
    fn separator_rejects_content() {
        let _ = Cell::right("1.5", 10).sep(" kW");
    }

    /// Cells and separators are single-line, so a row is one line in both
    /// kernels.
    #[test]
    #[should_panic(expected = "is one line")]
    fn cell_rejects_a_line_break() {
        let _ = Cell::plain("a\nb");
    }

    /// A report's sections size independently: free text closes the open run, so
    /// a wide name in the second section does not widen the first section's
    /// column. (In the parity kernel the runs never interact at all — each cell
    /// carries its own Pascal width — so this asserts the *default* kernel's
    /// grouping.)
    #[test]
    fn text_closes_the_open_run_of_rows() {
        let mut r = Report::new();
        r.line("SECTION ONE");
        r.row(Row::new().cell(Cell::left("a", 4)).cell(Cell::plain("1")));
        r.line("SECTION TWO");
        r.row(
            Row::new()
                .cell(Cell::left("a_much_longer_name", 4))
                .cell(Cell::plain("2")),
        );
        let out = r.finish();
        assert!(
            out.contains("SECTION ONE\na"),
            "the first section keeps its own width: {out:?}"
        );
        assert!(
            out.lines().count() == 4,
            "one line per text line and per row: {out:?}"
        );
    }

    /// An empty run renders to nothing at all (no stray blank line), in both
    /// kernels — the `Show` reports emit header text for sections that turn out
    /// to have no rows.
    #[test]
    fn an_empty_run_renders_to_nothing() {
        assert_eq!(render_rows_pad_impl(&[]), "");
        assert_eq!(render_rows_table_impl(&[]), "");
        let mut r = Report::new();
        r.line("EMPTY SECTION");
        assert_eq!(r.finish(), "EMPTY SECTION\n");
    }
}
