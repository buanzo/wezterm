use std::cmp::Ordering;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use anyhow::{anyhow, bail, ensure, Context};
use lazy_static::lazy_static;
use serde::Deserialize;
use termwiz::cell::{AttributeChange, CellAttributes};
use termwiz::color::{ColorAttribute, RgbColor};
use termwiz::image::{ImageData, ImageDataType, TextureCoordinate};
use termwiz::input::{InputEvent, KeyCode, KeyEvent, Modifiers, MouseButtons, MouseEvent};
use termwiz::surface::{Change, Image, Position};
use termwiz::terminal::Terminal;
use termwiz_funcs::truncate_right;
use wezterm_term::OwtTranscriptEvent;

const DATAVIEW_KIND: &str = "owt.lcars.dataview";
const DATAVIEW_VERSION: u16 = 1;
const MAX_DATAVIEW_BYTES: usize = 256 * 1024;
const MAX_DATAVIEW_ROWS: usize = 2000;
const MAX_DATAVIEW_COLUMNS: usize = 24;
const MAX_CELL_CHARS: usize = 200;
const MAX_COLUMN_CHARS: usize = 64;
const MAX_TITLE_CHARS: usize = 120;
const MAX_ID_CHARS: usize = 96;
const MAX_PENDING_DATAVIEWS: usize = 16;
const COMPACT_RAIL_WIDTH: usize = 10;
const RAIL_WIDTH: usize = 15;
const HEADER_ROWS: usize = 6;
const TABLE_HEADER_Y: usize = 5;
const TABLE_ROWS_Y: usize = 6;
const FOOTER_ROWS: usize = 2;
const ROW_TAG_WIDTH: usize = 5;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LcarsDataViewDocument {
    pub id: String,
    pub title: String,
    pub columns: Vec<String>,
    pub rows: Vec<Vec<String>>,
}

#[derive(Debug, Deserialize)]
struct RawDataViewDocument {
    version: u16,
    kind: String,
    id: String,
    title: String,
    columns: Vec<String>,
    rows: Vec<Vec<String>>,
}

#[derive(Debug, Deserialize)]
struct DataViewBeginPayload {
    version: u16,
    id: String,
    kind: String,
    chunks: usize,
    bytes: usize,
}

#[derive(Debug, Deserialize)]
struct DataViewChunkPayload {
    version: u16,
    id: String,
    index: usize,
    data: String,
}

#[derive(Debug, Deserialize)]
struct DataViewEndPayload {
    version: u16,
    id: String,
}

#[derive(Debug)]
struct PendingDataView {
    chunks: Vec<Option<String>>,
    expected_bytes: usize,
    received: usize,
}

lazy_static! {
    static ref PENDING_DATAVIEWS: Mutex<HashMap<String, PendingDataView>> =
        Mutex::new(HashMap::new());
}

pub fn accept_owt_dataview_event(event: &OwtTranscriptEvent) -> Option<LcarsDataViewDocument> {
    match event.event.as_str() {
        "owt.dataview.begin" => {
            if let Err(err) = accept_dataview_begin(&event.payload_json) {
                log::warn!("Rejected OWT DataView begin: {err:#}");
            }
            None
        }
        "owt.dataview.chunk" => {
            if let Err(err) = accept_dataview_chunk(&event.payload_json) {
                log::warn!("Rejected OWT DataView chunk: {err:#}");
            }
            None
        }
        "owt.dataview.end" => match accept_dataview_end(&event.payload_json) {
            Ok(document) => document,
            Err(err) => {
                log::warn!("Rejected OWT DataView end: {err:#}");
                None
            }
        },
        _ => None,
    }
}

fn accept_dataview_begin(payload_json: &str) -> anyhow::Result<()> {
    let payload: DataViewBeginPayload =
        serde_json::from_str(payload_json).context("parse DataView begin payload")?;
    ensure!(
        payload.version == DATAVIEW_VERSION,
        "unsupported DataView begin version"
    );
    ensure!(
        payload.kind == DATAVIEW_KIND,
        "DataView begin kind must be {DATAVIEW_KIND}"
    );
    validate_id(&payload.id)?;
    ensure!(
        payload.chunks > 0 && payload.chunks <= MAX_DATAVIEW_BYTES / 512,
        "invalid DataView chunk count"
    );
    ensure!(
        payload.bytes > 0 && payload.bytes <= MAX_DATAVIEW_BYTES,
        "invalid DataView byte count"
    );

    let mut pending = PENDING_DATAVIEWS
        .lock()
        .map_err(|_| anyhow!("DataView pending map lock poisoned"))?;
    if pending.len() >= MAX_PENDING_DATAVIEWS && !pending.contains_key(&payload.id) {
        pending.clear();
    }
    pending.insert(
        payload.id,
        PendingDataView {
            chunks: vec![None; payload.chunks],
            expected_bytes: payload.bytes,
            received: 0,
        },
    );
    Ok(())
}

fn accept_dataview_chunk(payload_json: &str) -> anyhow::Result<()> {
    let payload: DataViewChunkPayload =
        serde_json::from_str(payload_json).context("parse DataView chunk payload")?;
    ensure!(
        payload.version == DATAVIEW_VERSION,
        "unsupported DataView chunk version"
    );
    validate_id(&payload.id)?;
    ensure!(
        payload.data.len() <= 4096,
        "DataView chunk text is too large"
    );

    let mut pending = PENDING_DATAVIEWS
        .lock()
        .map_err(|_| anyhow!("DataView pending map lock poisoned"))?;
    let Some(assembly) = pending.get_mut(&payload.id) else {
        return Ok(());
    };
    if payload.index >= assembly.chunks.len() {
        pending.remove(&payload.id);
        bail!("DataView chunk index is out of range");
    }
    if assembly.chunks[payload.index].is_some() {
        pending.remove(&payload.id);
        bail!("duplicate DataView chunk");
    }
    assembly.chunks[payload.index] = Some(payload.data);
    assembly.received += 1;
    Ok(())
}

fn accept_dataview_end(payload_json: &str) -> anyhow::Result<Option<LcarsDataViewDocument>> {
    let payload: DataViewEndPayload =
        serde_json::from_str(payload_json).context("parse DataView end payload")?;
    ensure!(
        payload.version == DATAVIEW_VERSION,
        "unsupported DataView end version"
    );
    validate_id(&payload.id)?;

    let assembly = {
        let mut pending = PENDING_DATAVIEWS
            .lock()
            .map_err(|_| anyhow!("DataView pending map lock poisoned"))?;
        pending.remove(&payload.id)
    };
    let Some(assembly) = assembly else {
        return Ok(None);
    };
    ensure!(
        assembly.received == assembly.chunks.len(),
        "DataView ended before all chunks arrived"
    );

    let mut json = String::new();
    for chunk in assembly.chunks {
        let Some(chunk) = chunk else {
            bail!("DataView has a missing chunk");
        };
        json.push_str(&chunk);
        ensure!(
            json.len() <= MAX_DATAVIEW_BYTES,
            "DataView assembled payload is too large"
        );
    }
    ensure!(
        json.len() == assembly.expected_bytes,
        "DataView assembled byte count mismatch"
    );
    validate_dataview_document(&json).map(Some)
}

pub(crate) fn validate_dataview_document(
    payload_json: &str,
) -> anyhow::Result<LcarsDataViewDocument> {
    ensure!(
        payload_json.len() <= MAX_DATAVIEW_BYTES,
        "DataView payload is too large"
    );
    let raw: RawDataViewDocument =
        serde_json::from_str(payload_json).context("parse DataView document")?;
    ensure!(
        raw.version == DATAVIEW_VERSION,
        "unsupported DataView version"
    );
    ensure!(
        raw.kind == DATAVIEW_KIND,
        "DataView kind must be {DATAVIEW_KIND}"
    );
    validate_id(&raw.id)?;
    validate_short_text("DataView title", &raw.title, MAX_TITLE_CHARS)?;
    ensure!(
        !raw.columns.is_empty() && raw.columns.len() <= MAX_DATAVIEW_COLUMNS,
        "DataView must have 1..={MAX_DATAVIEW_COLUMNS} columns"
    );
    ensure!(
        raw.rows.len() <= MAX_DATAVIEW_ROWS,
        "DataView row limit exceeded"
    );

    let mut columns = Vec::with_capacity(raw.columns.len());
    for column in raw.columns {
        validate_short_text("DataView column", &column, MAX_COLUMN_CHARS)?;
        columns.push(column.trim().to_string());
    }

    let mut rows = Vec::with_capacity(raw.rows.len());
    for mut row in raw.rows {
        ensure!(
            row.len() <= columns.len(),
            "DataView row has more cells than columns"
        );
        while row.len() < columns.len() {
            row.push(String::new());
        }
        for cell in &row {
            ensure!(
                cell.chars().count() <= MAX_CELL_CHARS,
                "DataView cell text is too long"
            );
        }
        rows.push(row);
    }

    Ok(LcarsDataViewDocument {
        id: raw.id,
        title: raw.title.trim().to_string(),
        columns,
        rows,
    })
}

fn validate_id(id: &str) -> anyhow::Result<()> {
    validate_short_text("DataView id", id, MAX_ID_CHARS)?;
    ensure!(
        id.bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-' | b'_')),
        "DataView id contains unsupported characters"
    );
    Ok(())
}

fn validate_short_text(field: &'static str, value: &str, max_chars: usize) -> anyhow::Result<()> {
    let trimmed = value.trim();
    ensure!(!trimmed.is_empty(), "{field} must not be empty");
    ensure!(trimmed.chars().count() <= max_chars, "{field} is too long");
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SortDirection {
    Ascending,
    Descending,
}

#[derive(Debug)]
struct LcarsDataViewState {
    document: LcarsDataViewDocument,
    query: String,
    editing_search: bool,
    selected_col: usize,
    selected_index: usize,
    scroll_offset: usize,
    sort: Option<(usize, SortDirection)>,
}

impl LcarsDataViewState {
    fn new(document: LcarsDataViewDocument) -> Self {
        Self {
            document,
            query: String::new(),
            editing_search: false,
            selected_col: 0,
            selected_index: 0,
            scroll_offset: 0,
            sort: None,
        }
    }

    fn visible_indices(&self) -> Vec<usize> {
        visible_row_indices(&self.document, &self.query, self.sort)
    }

    fn page_rows(size_rows: usize) -> usize {
        size_rows.saturating_sub(TABLE_ROWS_Y + FOOTER_ROWS).max(1)
    }

    fn clamp_selection(&mut self, visible_len: usize) {
        if visible_len == 0 {
            self.selected_index = 0;
            self.scroll_offset = 0;
            return;
        }
        self.selected_index = self.selected_index.min(visible_len - 1);
        self.selected_col = self
            .selected_col
            .min(self.document.columns.len().saturating_sub(1));
    }

    fn ensure_selected_visible(&mut self, page_rows: usize, visible_len: usize) {
        self.clamp_selection(visible_len);
        if visible_len == 0 {
            return;
        }
        if self.selected_index < self.scroll_offset {
            self.scroll_offset = self.selected_index;
        } else if self.selected_index >= self.scroll_offset + page_rows {
            self.scroll_offset = self
                .selected_index
                .saturating_sub(page_rows.saturating_sub(1));
        }
        self.scroll_offset = self
            .scroll_offset
            .min(visible_len.saturating_sub(page_rows.min(visible_len)));
    }

    fn move_selection(&mut self, delta: isize) {
        let visible_len = self.visible_indices().len();
        if visible_len == 0 {
            return;
        }
        let next = if delta.is_negative() {
            self.selected_index.saturating_sub(delta.unsigned_abs())
        } else {
            (self.selected_index + delta as usize).min(visible_len - 1)
        };
        self.selected_index = next;
    }

    fn move_column(&mut self, delta: isize) {
        if self.document.columns.is_empty() {
            return;
        }
        self.selected_col = if delta.is_negative() {
            self.selected_col.saturating_sub(delta.unsigned_abs())
        } else {
            (self.selected_col + delta as usize).min(self.document.columns.len() - 1)
        };
    }

    fn cycle_sort(&mut self) {
        let col = self.selected_col;
        self.sort = match self.sort {
            Some((current, SortDirection::Ascending)) if current == col => {
                Some((col, SortDirection::Descending))
            }
            Some((current, SortDirection::Descending)) if current == col => None,
            _ => Some((col, SortDirection::Ascending)),
        };
        self.selected_index = 0;
        self.scroll_offset = 0;
    }

    fn update_query(&mut self, query: String) {
        self.query = query;
        self.selected_index = 0;
        self.scroll_offset = 0;
    }

    fn render(&mut self, term: &mut mux::termwiztermtab::TermWizTerminal) -> termwiz::Result<()> {
        let size = term.get_screen_size()?;
        let layout = DataViewLayout::new(size.cols, size.rows, size.xpixel, size.ypixel);
        let page_rows = Self::page_rows(size.rows);
        let visible = self.visible_indices();
        self.ensure_selected_visible(page_rows, visible.len());
        let widths = column_widths(layout.table_width(), self.document.columns.len());
        let mut changes = vec![
            Change::ClearScreen(lcars_black()),
            Change::CursorPosition {
                x: Position::Absolute(0),
                y: Position::Absolute(0),
            },
        ];
        self.render_chassis(&layout, visible.len(), &mut changes);
        self.render_columns(&layout, &widths, &mut changes);
        self.render_rows(&layout, &visible, page_rows, &widths, &mut changes);
        changes.push(Change::AllAttributes(CellAttributes::default()));
        term.render(&changes)
    }

    fn render_chassis(
        &self,
        layout: &DataViewLayout,
        visible_len: usize,
        changes: &mut Vec<Change>,
    ) {
        if layout.supports_image_chrome() {
            self.render_image_chassis(layout, visible_len, changes);
        } else {
            self.render_cell_chassis(layout, visible_len, changes);
        }
    }

    fn render_image_chassis(
        &self,
        layout: &DataViewLayout,
        visible_len: usize,
        changes: &mut Vec<Change>,
    ) {
        let sort_label = match self.sort {
            Some((column, SortDirection::Ascending)) => {
                format!("{} asc", self.document.columns[column])
            }
            Some((column, SortDirection::Descending)) => {
                format!("{} desc", self.document.columns[column])
            }
            None => "original".to_string(),
        };
        let search_mode = if self.editing_search {
            "SEARCH*"
        } else {
            "search"
        };

        draw_fill(changes, 0, 0, layout.cols, layout.rows, lcars_black());
        self.render_image_left_rail(layout, changes);
        self.render_image_top_deck(layout, changes);
        self.render_image_bay_frame(layout, changes);
        self.render_image_footer(layout, changes);

        let top_x = layout.content_x.saturating_sub(1);
        let top_w = layout.cols.saturating_sub(top_x + 1);
        draw_cell(
            changes,
            top_x,
            0,
            top_w.min(20),
            "LCARS DATAVIEW",
            lcars_black(),
            lcars_orange(),
        );
        draw_cell(
            changes,
            top_x,
            1,
            (top_w.saturating_mul(2) / 5).min(22),
            "ACTIVE DATA BAY",
            lcars_black(),
            lcars_butterscotch(),
        );
        draw_cell(
            changes,
            top_x + top_w.saturating_mul(2) / 5 + 1,
            1,
            top_w.saturating_mul(3) / 5 - 1,
            &format!(
                "{}  ROWS {}  VISIBLE {}",
                self.document.title.to_ascii_uppercase(),
                self.document.rows.len(),
                visible_len
            ),
            lcars_cream(),
            lcars_steel(),
        );

        draw_cell(
            changes,
            top_x,
            2,
            11,
            "Q CLOSE",
            lcars_black(),
            lcars_salmon(),
        );
        draw_cell(
            changes,
            top_x + 12,
            2,
            12,
            "/ SEARCH",
            lcars_black(),
            lcars_gold(),
        );
        draw_cell(
            changes,
            top_x + 25,
            2,
            10,
            "S SORT",
            lcars_black(),
            lcars_violet(),
        );
        draw_cell(
            changes,
            top_x + 36,
            2,
            top_w.saturating_sub(36),
            &format!("{search_mode}: '{}'   SORT: {}", self.query, sort_label),
            lcars_cream(),
            lcars_black(),
        );
        draw_cell(
            changes,
            top_x + top_w / 2 + 1,
            3,
            top_w.saturating_sub(top_w / 2 + 1),
            "1701 / LOCAL / SANDBOX",
            lcars_black(),
            lcars_gold(),
        );

        let bay_w = layout.content_w;
        draw_cell(
            changes,
            layout.content_x,
            4,
            bay_w,
            &format!("{} :: {}", self.document.id, self.document.title),
            lcars_black(),
            lcars_cream(),
        );
        draw_cell(
            changes,
            layout.content_x,
            layout.footer_y(),
            (bay_w / 3).min(36),
            "",
            lcars_black(),
            lcars_violet(),
        );
        draw_cell(
            changes,
            layout.content_x + bay_w / 3 + 1,
            layout.footer_y(),
            bay_w.saturating_sub(bay_w / 3 + 1),
            "READ ONLY  NO MCP  NO FILE ACCESS",
            lcars_black(),
            lcars_orange(),
        );
        self.render_left_rail_labels(layout, changes);
    }

    fn render_cell_chassis(
        &self,
        layout: &DataViewLayout,
        visible_len: usize,
        changes: &mut Vec<Change>,
    ) {
        let sort_label = match self.sort {
            Some((column, SortDirection::Ascending)) => {
                format!("{} asc", self.document.columns[column])
            }
            Some((column, SortDirection::Descending)) => {
                format!("{} desc", self.document.columns[column])
            }
            None => "original".to_string(),
        };
        let search_mode = if self.editing_search {
            "SEARCH*"
        } else {
            "search"
        };

        draw_fill(changes, 0, 0, layout.cols, layout.rows, lcars_black());
        self.render_left_rail(layout, changes);

        let top_x = layout.content_x.saturating_sub(1);
        let top_w = layout.cols.saturating_sub(top_x + 1);
        draw_cell(
            changes,
            top_x,
            0,
            top_w,
            "LCARS DATAVIEW",
            lcars_black(),
            lcars_orange(),
        );
        draw_cell(
            changes,
            top_x,
            1,
            top_w.saturating_mul(2) / 5,
            "ACTIVE DATA BAY",
            lcars_black(),
            lcars_butterscotch(),
        );
        draw_cell(
            changes,
            top_x + top_w.saturating_mul(2) / 5 + 1,
            1,
            top_w.saturating_mul(3) / 5 - 1,
            &format!(
                "{}  ROWS {}  VISIBLE {}",
                self.document.title.to_ascii_uppercase(),
                self.document.rows.len(),
                visible_len
            ),
            lcars_cream(),
            lcars_steel(),
        );

        draw_cell(
            changes,
            top_x,
            2,
            11,
            "Q CLOSE",
            lcars_black(),
            lcars_salmon(),
        );
        draw_cell(
            changes,
            top_x + 12,
            2,
            12,
            "/ SEARCH",
            lcars_black(),
            lcars_gold(),
        );
        draw_cell(
            changes,
            top_x + 25,
            2,
            10,
            "S SORT",
            lcars_black(),
            lcars_violet(),
        );
        draw_cell(
            changes,
            top_x + 36,
            2,
            top_w.saturating_sub(36),
            &format!("{search_mode}: '{}'   SORT: {}", self.query, sort_label),
            lcars_cream(),
            lcars_black(),
        );
        draw_cell(
            changes,
            top_x,
            3,
            top_w / 2,
            "",
            lcars_black(),
            lcars_blue(),
        );
        draw_cell(
            changes,
            top_x + top_w / 2 + 1,
            3,
            top_w.saturating_sub(top_w / 2 + 1),
            "1701 / LOCAL / SANDBOX",
            lcars_black(),
            lcars_gold(),
        );

        let bay_w = layout.content_w;
        draw_cell(
            changes,
            layout.content_x,
            4,
            bay_w,
            &format!("{} :: {}", self.document.id, self.document.title),
            lcars_black(),
            lcars_cream(),
        );
        draw_cell(
            changes,
            layout.content_x,
            layout.footer_y(),
            bay_w / 3,
            "",
            lcars_black(),
            lcars_violet(),
        );
        draw_cell(
            changes,
            layout.content_x + bay_w / 3 + 1,
            layout.footer_y(),
            bay_w.saturating_sub(bay_w / 3 + 1),
            "READ ONLY  NO MCP  NO FILE ACCESS",
            lcars_black(),
            lcars_orange(),
        );
    }

    fn render_left_rail(&self, layout: &DataViewLayout, changes: &mut Vec<Change>) {
        draw_cell(
            changes,
            0,
            0,
            layout.rail_w,
            "OWT",
            lcars_black(),
            lcars_gold(),
        );
        draw_cell(
            changes,
            0,
            1,
            layout.rail_w,
            "DATA",
            lcars_black(),
            lcars_butterscotch(),
        );
        draw_cell(
            changes,
            0,
            2,
            layout.rail_w,
            "VIEW",
            lcars_black(),
            lcars_blue(),
        );
        draw_cell(
            changes,
            0,
            4,
            layout.rail_w,
            "1701",
            lcars_black(),
            lcars_violet(),
        );
        draw_cell(
            changes,
            0,
            5,
            layout.rail_w,
            "LOCAL",
            lcars_black(),
            lcars_orange(),
        );
        let rail_body_start = HEADER_ROWS;
        let rail_body_rows = layout.rows.saturating_sub(rail_body_start + 1);
        for row in 0..rail_body_rows {
            let y = rail_body_start + row;
            let color = match row % 7 {
                0 | 1 => lcars_steel(),
                2 => lcars_gold(),
                3 | 4 => lcars_butterscotch(),
                5 => lcars_violet(),
                _ => lcars_black(),
            };
            draw_cell(changes, 0, y, layout.rail_w, "", lcars_black(), color);
        }
        if layout.rows > 2 {
            draw_cell(
                changes,
                0,
                layout.rows - 2,
                layout.rail_w,
                "INDEX",
                lcars_black(),
                lcars_salmon(),
            );
        }
    }

    fn render_left_rail_labels(&self, layout: &DataViewLayout, changes: &mut Vec<Change>) {
        draw_cell(
            changes,
            0,
            0,
            layout.rail_w,
            "OWT",
            lcars_black(),
            lcars_gold(),
        );
        draw_cell(
            changes,
            0,
            1,
            layout.rail_w,
            "DATA",
            lcars_black(),
            lcars_butterscotch(),
        );
        draw_cell(
            changes,
            0,
            2,
            layout.rail_w,
            "VIEW",
            lcars_black(),
            lcars_blue(),
        );
        draw_cell(
            changes,
            0,
            4,
            layout.rail_w,
            "1701",
            lcars_black(),
            lcars_violet(),
        );
        draw_cell(
            changes,
            0,
            5,
            layout.rail_w,
            "LOCAL",
            lcars_black(),
            lcars_orange(),
        );
        if layout.rows > 2 {
            draw_cell(
                changes,
                0,
                layout.rows - 2,
                layout.rail_w,
                "INDEX",
                lcars_black(),
                lcars_salmon(),
            );
        }
    }

    fn render_image_left_rail(&self, layout: &DataViewLayout, changes: &mut Vec<Change>) {
        let height = layout.rows.saturating_sub(1);
        draw_lcars_image_block(changes, layout, 0, 0, layout.rail_w, height, |raster| {
            let ch = layout.cell_px_h as f32;
            let w = raster.width as f32;
            let h = raster.height as f32;
            let radius = ch * 0.55;

            raster.fill_rounded_rect(0.0, 0.0, w, ch * 3.0, radius, BYTE_GOLD);
            raster.fill_rect(0.0, ch, w, ch, BYTE_BUTTERSCOTCH);
            raster.fill_rect(0.0, ch * 2.0, w, ch, BYTE_BLUE);
            raster.fill_rounded_rect(0.0, ch * 4.0, w, ch * 2.0, radius, BYTE_VIOLET);
            raster.fill_rect(0.0, ch * 5.0, w, ch, BYTE_ORANGE);

            let body_y = ch * HEADER_ROWS as f32;
            let body_h = (h - body_y - ch * 2.0).max(ch);
            raster.fill_rounded_rect(0.0, body_y, w, body_h, radius, BYTE_STEEL);

            for gap_y in [10.0, 18.0, 29.0, 41.0] {
                let y = gap_y * ch;
                if y < h - ch * 4.0 {
                    raster.fill_rect(0.0, y, w, ch * 1.2, BYTE_BLACK);
                }
            }

            for (cell_y, cell_h, color) in [
                (7.0, 2.0, BYTE_GOLD),
                (12.0, 3.0, BYTE_BUTTERSCOTCH),
                (21.0, 3.0, BYTE_VIOLET),
                (33.0, 4.0, BYTE_GOLD),
                (45.0, 3.0, BYTE_BUTTERSCOTCH),
            ] {
                let y = cell_y * ch;
                if y < h - ch * 5.0 {
                    raster.fill_rounded_rect(0.0, y, w * 0.94, cell_h * ch, radius, color);
                }
            }

            if h > ch * 3.0 {
                raster.fill_rounded_rect(0.0, h - ch * 2.0, w, ch, radius, BYTE_SALMON);
            }
        });
    }

    fn render_image_top_deck(&self, layout: &DataViewLayout, changes: &mut Vec<Change>) {
        let top_x = layout.content_x.saturating_sub(1);
        let top_w = layout.cols.saturating_sub(top_x + 1);
        draw_lcars_image_block(
            changes,
            layout,
            top_x,
            0,
            top_w,
            HEADER_ROWS - 1,
            |raster| {
                let ch = layout.cell_px_h as f32;
                let cw = layout.cell_px_w as f32;
                let w = raster.width as f32;
                let radius = ch * 0.48;
                let split = w * 0.40;

                raster.fill_rounded_rect(0.0, 0.0, w, ch * 0.95, radius, BYTE_ORANGE);
                raster.fill_rounded_rect(
                    0.0,
                    ch * 1.05,
                    split,
                    ch * 0.95,
                    radius,
                    BYTE_BUTTERSCOTCH,
                );
                raster.fill_rounded_rect(
                    split + cw,
                    ch * 1.05,
                    (w - split - cw).max(cw),
                    ch * 0.95,
                    radius,
                    BYTE_STEEL,
                );

                raster.fill_rounded_rect(0.0, ch * 2.05, cw * 11.0, ch * 0.95, radius, BYTE_SALMON);
                raster.fill_rounded_rect(
                    cw * 12.0,
                    ch * 2.05,
                    cw * 12.0,
                    ch * 0.95,
                    radius,
                    BYTE_GOLD,
                );
                raster.fill_rounded_rect(
                    cw * 25.0,
                    ch * 2.05,
                    cw * 10.0,
                    ch * 0.95,
                    radius,
                    BYTE_VIOLET,
                );
                raster.fill_rounded_rect(
                    cw * 36.0,
                    ch * 2.05,
                    (w - cw * 36.0).max(cw),
                    ch * 0.95,
                    radius,
                    BYTE_BLACK,
                );

                raster.fill_rounded_rect(0.0, ch * 3.08, w * 0.50, ch * 0.90, radius, BYTE_BLUE);
                raster.fill_rounded_rect(
                    w * 0.50 + cw,
                    ch * 3.08,
                    (w * 0.50 - cw).max(cw),
                    ch * 0.90,
                    radius,
                    BYTE_GOLD,
                );
                raster.fill_rounded_rect(0.0, ch * 4.08, w, ch * 0.90, radius, BYTE_CREAM);
            },
        );
    }

    fn render_image_bay_frame(&self, layout: &DataViewLayout, changes: &mut Vec<Change>) {
        let footer_y = layout.footer_y();
        let bay_y = 4;
        let bay_h = footer_y.saturating_sub(bay_y);
        if bay_h < 4 {
            return;
        }
        let frame_x = layout.content_x.saturating_sub(1);
        let frame_w = ROW_TAG_WIDTH + 2;
        draw_lcars_image_block(changes, layout, frame_x, bay_y, frame_w, bay_h, |raster| {
            let ch = layout.cell_px_h as f32;
            let w = raster.width as f32;
            let h = raster.height as f32;
            let radius = ch * 0.45;
            raster.fill_rounded_rect(0.0, 0.0, w * 0.70, h, radius, BYTE_STEEL);
            raster.fill_rect(w * 0.70, 0.0, w * 0.30, h, BYTE_BLACK);
            raster.fill_rounded_rect(0.0, 0.0, w, ch, radius, BYTE_CREAM);
        });
    }

    fn render_image_footer(&self, layout: &DataViewLayout, changes: &mut Vec<Change>) {
        let footer_y = layout.footer_y();
        let width = layout.content_w;
        draw_lcars_image_block(
            changes,
            layout,
            layout.content_x,
            footer_y,
            width,
            1,
            |raster| {
                let ch = layout.cell_px_h as f32;
                let w = raster.width as f32;
                let radius = ch * 0.48;
                let left_w = w * 0.34;
                raster.fill_rounded_rect(0.0, 0.0, left_w, ch, radius, BYTE_VIOLET);
                raster.fill_rounded_rect(
                    left_w + layout.cell_px_w as f32,
                    0.0,
                    w - left_w,
                    ch,
                    radius,
                    BYTE_ORANGE,
                );
            },
        );
    }

    fn render_columns(&self, layout: &DataViewLayout, widths: &[usize], changes: &mut Vec<Change>) {
        draw_cell(
            changes,
            layout.content_x,
            TABLE_HEADER_Y,
            ROW_TAG_WIDTH,
            "IDX",
            lcars_black(),
            lcars_orange(),
        );
        let mut x = layout.table_x();
        for (idx, column) in self.document.columns.iter().enumerate() {
            let width = widths.get(idx).copied().unwrap_or(8);
            let background = if idx == self.selected_col {
                lcars_gold()
            } else if idx % 2 == 0 {
                lcars_steel()
            } else {
                lcars_blue()
            };
            draw_cell(
                changes,
                x,
                TABLE_HEADER_Y,
                width,
                column,
                lcars_black(),
                background,
            );
            x += width + 1;
        }
    }

    fn render_rows(
        &self,
        layout: &DataViewLayout,
        visible: &[usize],
        page_rows: usize,
        widths: &[usize],
        changes: &mut Vec<Change>,
    ) {
        let rows = visible
            .iter()
            .skip(self.scroll_offset)
            .take(page_rows)
            .enumerate();
        for (screen_index, row_index) in rows {
            let selected = self.scroll_offset + screen_index == self.selected_index;
            let y = TABLE_ROWS_Y + screen_index;
            let tag_bg = match screen_index % 5 {
                0 => lcars_gold(),
                1 => lcars_butterscotch(),
                2 => lcars_blue(),
                3 => lcars_violet(),
                _ => lcars_salmon(),
            };
            draw_cell(
                changes,
                layout.content_x,
                y,
                ROW_TAG_WIDTH,
                &format!("{:03}", self.scroll_offset + screen_index + 1),
                lcars_black(),
                tag_bg,
            );
            let row_bg = if selected {
                lcars_cream()
            } else {
                lcars_black()
            };
            let row_fg = if selected {
                lcars_black()
            } else {
                lcars_silver()
            };
            draw_cell(
                changes,
                layout.table_x(),
                y,
                layout.table_width(),
                "",
                row_fg,
                row_bg,
            );
            let accent_fg = if selected {
                lcars_black()
            } else {
                lcars_aqua()
            };
            let emphasis_fg = if selected {
                lcars_black()
            } else {
                lcars_gold()
            };
            let mut x = layout.table_x();
            let status_column = self.document.columns.len().saturating_sub(1);
            if selected {
                draw_cell(
                    changes,
                    layout.table_x().saturating_sub(1),
                    y,
                    1,
                    "",
                    lcars_black(),
                    lcars_salmon(),
                );
            }
            if let Some(row) = self.document.rows.get(*row_index) {
                for (idx, cell) in row.iter().enumerate() {
                    let width = widths.get(idx).copied().unwrap_or(8);
                    let row_color = match idx {
                        0 => accent_fg,
                        index if index == self.selected_col => emphasis_fg,
                        index if index == status_column => lcars_cream(),
                        _ => row_fg,
                    };
                    draw_cell(changes, x, y, width, cell, row_color, row_bg);
                    x += width + 1;
                }
            }
        }
        let rendered = visible
            .len()
            .saturating_sub(self.scroll_offset)
            .min(page_rows);
        for screen_index in rendered..page_rows {
            let y = TABLE_ROWS_Y + screen_index;
            draw_cell(
                changes,
                layout.content_x,
                y,
                ROW_TAG_WIDTH,
                "",
                lcars_black(),
                lcars_steel(),
            );
            draw_cell(
                changes,
                layout.table_x(),
                y,
                layout.table_width(),
                "",
                lcars_silver(),
                lcars_black(),
            );
        }
    }

    fn column_from_x(&self, x: usize, layout: &DataViewLayout, widths: &[usize]) -> Option<usize> {
        let mut cursor = layout.table_x();
        for (index, width) in widths.iter().copied().enumerate() {
            let next = cursor + width + 1;
            if x >= cursor && x < next {
                return Some(index);
            }
            cursor = next;
        }
        None
    }

    fn run_loop(&mut self, term: &mut mux::termwiztermtab::TermWizTerminal) -> anyhow::Result<()> {
        self.render(term)?;
        while let Ok(Some(event)) = term.poll_input(None) {
            let size = term.get_screen_size()?;
            let layout = DataViewLayout::new(size.cols, size.rows, size.xpixel, size.ypixel);
            let page_rows = Self::page_rows(size.rows);
            let mut should_close = false;
            match event {
                InputEvent::Key(key) if self.editing_search => {
                    should_close = self.handle_search_key(key);
                }
                InputEvent::Key(KeyEvent {
                    key: KeyCode::Escape,
                    ..
                })
                | InputEvent::Key(KeyEvent {
                    key: KeyCode::Char('q'),
                    ..
                }) => should_close = true,
                InputEvent::Key(KeyEvent {
                    key: KeyCode::Char('/'),
                    ..
                }) => self.editing_search = true,
                InputEvent::Key(KeyEvent {
                    key: KeyCode::DownArrow,
                    ..
                })
                | InputEvent::Key(KeyEvent {
                    key: KeyCode::Char('j'),
                    ..
                })
                | InputEvent::Key(KeyEvent {
                    key: KeyCode::Char('n'),
                    ..
                }) => self.move_selection(1),
                InputEvent::Key(KeyEvent {
                    key: KeyCode::UpArrow,
                    ..
                })
                | InputEvent::Key(KeyEvent {
                    key: KeyCode::Char('k'),
                    ..
                })
                | InputEvent::Key(KeyEvent {
                    key: KeyCode::Char('N'),
                    ..
                }) => self.move_selection(-1),
                InputEvent::Key(KeyEvent {
                    key: KeyCode::LeftArrow,
                    ..
                })
                | InputEvent::Key(KeyEvent {
                    key: KeyCode::Char('h'),
                    ..
                }) => self.move_column(-1),
                InputEvent::Key(KeyEvent {
                    key: KeyCode::RightArrow,
                    ..
                })
                | InputEvent::Key(KeyEvent {
                    key: KeyCode::Char('l'),
                    ..
                }) => self.move_column(1),
                InputEvent::Key(KeyEvent {
                    key: KeyCode::PageDown,
                    ..
                }) => self.move_selection(page_rows as isize),
                InputEvent::Key(KeyEvent {
                    key: KeyCode::PageUp,
                    ..
                }) => self.move_selection(-(page_rows as isize)),
                InputEvent::Key(KeyEvent {
                    key: KeyCode::Home, ..
                }) => self.selected_index = 0,
                InputEvent::Key(KeyEvent {
                    key: KeyCode::End, ..
                }) => {
                    let len = self.visible_indices().len();
                    self.selected_index = len.saturating_sub(1);
                }
                InputEvent::Key(KeyEvent {
                    key: KeyCode::Char('s'),
                    ..
                }) => self.cycle_sort(),
                InputEvent::Mouse(MouseEvent { mouse_buttons, .. })
                    if mouse_buttons.contains(MouseButtons::VERT_WHEEL) =>
                {
                    if mouse_buttons.contains(MouseButtons::WHEEL_POSITIVE) {
                        self.move_selection(-1);
                    } else {
                        self.move_selection(1);
                    }
                }
                InputEvent::Mouse(MouseEvent {
                    x,
                    y,
                    mouse_buttons,
                    ..
                }) if mouse_buttons == MouseButtons::LEFT => {
                    if y as usize == TABLE_HEADER_Y {
                        let widths =
                            column_widths(layout.table_width(), self.document.columns.len());
                        if let Some(column) = self.column_from_x(x as usize, &layout, &widths) {
                            self.selected_col = column;
                            self.cycle_sort();
                        }
                    } else if y as usize >= TABLE_ROWS_Y {
                        let clicked = self.scroll_offset + y as usize - TABLE_ROWS_Y;
                        if clicked < self.visible_indices().len() {
                            self.selected_index = clicked;
                        }
                    }
                }
                _ => {}
            }
            if should_close {
                break;
            }
            let len = self.visible_indices().len();
            self.ensure_selected_visible(page_rows, len);
            self.render(term)?;
        }
        Ok(())
    }

    fn handle_search_key(&mut self, key: KeyEvent) -> bool {
        let mods = key.modifiers.remove_positional_mods();
        match (key.key, mods) {
            (KeyCode::Escape, Modifiers::NONE) => self.editing_search = false,
            (KeyCode::Enter, Modifiers::NONE) => self.editing_search = false,
            (KeyCode::Backspace, Modifiers::NONE) => {
                let mut query = self.query.clone();
                query.pop();
                self.update_query(query);
            }
            (KeyCode::Char(c), Modifiers::NONE) | (KeyCode::Char(c), Modifiers::SHIFT) => {
                let mut query = self.query.clone();
                query.push(c);
                self.update_query(query);
            }
            (KeyCode::Char('q'), Modifiers::CTRL) => return true,
            _ => {}
        }
        false
    }
}

pub fn owt_dataview_overlay(
    mut term: mux::termwiztermtab::TermWizTerminal,
    document: LcarsDataViewDocument,
) -> anyhow::Result<()> {
    let mut state = LcarsDataViewState::new(document);
    term.set_raw_mode()?;
    term.render(&[Change::Title("OWT LCARS DataView".to_string())])?;
    state.run_loop(&mut term)
}

#[derive(Debug, Clone, Copy)]
struct DataViewLayout {
    cols: usize,
    rows: usize,
    cell_px_w: usize,
    cell_px_h: usize,
    rail_w: usize,
    content_x: usize,
    content_w: usize,
}

impl DataViewLayout {
    fn new(cols: usize, rows: usize, cell_px_w: usize, cell_px_h: usize) -> Self {
        let rail_w = if cols < 96 {
            COMPACT_RAIL_WIDTH
        } else {
            RAIL_WIDTH
        }
        .min(cols.saturating_sub(8).max(1));
        let content_x = rail_w.saturating_add(2).min(cols.saturating_sub(1));
        let content_w = cols.saturating_sub(content_x + 1).max(1);
        Self {
            cols,
            rows,
            cell_px_w,
            cell_px_h,
            rail_w,
            content_x,
            content_w,
        }
    }

    fn supports_image_chrome(&self) -> bool {
        self.cell_px_w > 0 && self.cell_px_h > 0 && self.cols >= 52 && self.rows >= 14
    }

    fn table_x(&self) -> usize {
        self.content_x + ROW_TAG_WIDTH + 1
    }

    fn table_width(&self) -> usize {
        self.content_w.saturating_sub(ROW_TAG_WIDTH + 1).max(1)
    }

    fn footer_y(&self) -> usize {
        self.rows.saturating_sub(FOOTER_ROWS)
    }
}

#[derive(Debug, Clone, Copy)]
struct LcarsByteColor {
    red: u8,
    green: u8,
    blue: u8,
    alpha: u8,
}

impl LcarsByteColor {
    const fn opaque(red: u8, green: u8, blue: u8) -> Self {
        Self {
            red,
            green,
            blue,
            alpha: 255,
        }
    }
}

const BYTE_BLACK: LcarsByteColor = LcarsByteColor::opaque(0, 0, 0);
const BYTE_ORANGE: LcarsByteColor = LcarsByteColor::opaque(255, 153, 0);
const BYTE_GOLD: LcarsByteColor = LcarsByteColor::opaque(255, 204, 102);
const BYTE_BUTTERSCOTCH: LcarsByteColor = LcarsByteColor::opaque(204, 153, 102);
const BYTE_VIOLET: LcarsByteColor = LcarsByteColor::opaque(180, 150, 255);
const BYTE_BLUE: LcarsByteColor = LcarsByteColor::opaque(102, 153, 255);
const BYTE_SALMON: LcarsByteColor = LcarsByteColor::opaque(255, 102, 102);
const BYTE_STEEL: LcarsByteColor = LcarsByteColor::opaque(50, 70, 96);
const BYTE_CREAM: LcarsByteColor = LcarsByteColor::opaque(255, 238, 204);

struct LcarsRaster {
    width: u32,
    height: u32,
    pixels: Vec<u8>,
}

impl LcarsRaster {
    fn new(width: u32, height: u32) -> Self {
        let len = width.saturating_mul(height).saturating_mul(4) as usize;
        Self {
            width,
            height,
            pixels: vec![0; len],
        }
    }

    fn fill_rect(&mut self, x: f32, y: f32, width: f32, height: f32, color: LcarsByteColor) {
        if width <= 0.0 || height <= 0.0 {
            return;
        }
        let x0 = x.floor().max(0.0) as i32;
        let y0 = y.floor().max(0.0) as i32;
        let x1 = (x + width).ceil().min(self.width as f32) as i32;
        let y1 = (y + height).ceil().min(self.height as f32) as i32;
        for py in y0..y1 {
            for px in x0..x1 {
                let coverage = rect_coverage(px as f32 + 0.5, py as f32 + 0.5, x, y, width, height);
                self.blend_pixel(px, py, color, coverage);
            }
        }
    }

    fn fill_rounded_rect(
        &mut self,
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        radius: f32,
        color: LcarsByteColor,
    ) {
        if width <= 0.0 || height <= 0.0 {
            return;
        }
        let x0 = x.floor().max(0.0) as i32;
        let y0 = y.floor().max(0.0) as i32;
        let x1 = (x + width).ceil().min(self.width as f32) as i32;
        let y1 = (y + height).ceil().min(self.height as f32) as i32;
        for py in y0..y1 {
            for px in x0..x1 {
                let coverage = rounded_rect_coverage(
                    px as f32 + 0.5,
                    py as f32 + 0.5,
                    x,
                    y,
                    width,
                    height,
                    radius,
                );
                self.blend_pixel(px, py, color, coverage);
            }
        }
    }

    fn blend_pixel(&mut self, x: i32, y: i32, color: LcarsByteColor, coverage: f32) {
        if x < 0 || y < 0 || x >= self.width as i32 || y >= self.height as i32 {
            return;
        }
        let src_alpha = (color.alpha as f32 / 255.0) * coverage.clamp(0.0, 1.0);
        if src_alpha <= 0.0 {
            return;
        }
        let idx = ((y as u32 * self.width + x as u32) * 4) as usize;
        let dst_alpha = self.pixels[idx + 3] as f32 / 255.0;
        let out_alpha = src_alpha + dst_alpha * (1.0 - src_alpha);
        if out_alpha <= 0.0 {
            return;
        }

        let blend_channel = |src: u8, dst: u8| -> u8 {
            let src = src as f32 / 255.0;
            let dst = dst as f32 / 255.0;
            (((src * src_alpha) + (dst * dst_alpha * (1.0 - src_alpha))) / out_alpha * 255.0)
                .round()
                .clamp(0.0, 255.0) as u8
        };

        self.pixels[idx] = blend_channel(color.red, self.pixels[idx]);
        self.pixels[idx + 1] = blend_channel(color.green, self.pixels[idx + 1]);
        self.pixels[idx + 2] = blend_channel(color.blue, self.pixels[idx + 2]);
        self.pixels[idx + 3] = (out_alpha * 255.0).round().clamp(0.0, 255.0) as u8;
    }

    fn into_png_bytes(self) -> Option<Vec<u8>> {
        use image::ImageEncoder;

        let mut encoded = Vec::new();
        image::codecs::png::PngEncoder::new(&mut encoded)
            .write_image(
                &self.pixels,
                self.width,
                self.height,
                image::ColorType::Rgba8,
            )
            .ok()?;
        Some(encoded)
    }
}

fn draw_lcars_image_block<F>(
    changes: &mut Vec<Change>,
    layout: &DataViewLayout,
    x: usize,
    y: usize,
    width_cells: usize,
    height_cells: usize,
    draw: F,
) where
    F: FnOnce(&mut LcarsRaster),
{
    if width_cells == 0
        || height_cells == 0
        || !layout.supports_image_chrome()
        || x >= layout.cols
        || y >= layout.rows
    {
        return;
    }
    let width_cells = width_cells.min(layout.cols.saturating_sub(x));
    let height_cells = height_cells.min(layout.rows.saturating_sub(y));
    let pixel_width = width_cells.saturating_mul(layout.cell_px_w).max(1) as u32;
    let pixel_height = height_cells.saturating_mul(layout.cell_px_h).max(1) as u32;
    let mut raster = LcarsRaster::new(pixel_width, pixel_height);
    draw(&mut raster);
    let Some(encoded) = raster.into_png_bytes() else {
        return;
    };
    changes.push(Change::CursorPosition {
        x: Position::Absolute(x),
        y: Position::Absolute(y),
    });
    changes.push(Change::Image(Image {
        width: width_cells,
        height: height_cells,
        top_left: TextureCoordinate::new_f32(0.0, 0.0),
        bottom_right: TextureCoordinate::new_f32(1.0, 1.0),
        image: Arc::new(ImageData::with_data(ImageDataType::EncodedFile(encoded))),
    }));
}

fn rounded_rect_coverage(
    sample_x: f32,
    sample_y: f32,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    radius: f32,
) -> f32 {
    if width <= 0.0 || height <= 0.0 {
        return 0.0;
    }
    let radius = radius.clamp(0.0, width.min(height) * 0.5);
    if radius < 0.5 {
        return rect_coverage(sample_x, sample_y, x, y, width, height);
    }
    let center_x = clamp_ordered(sample_x, x + radius, x + width - radius);
    let center_y = clamp_ordered(sample_y, y + radius, y + height - radius);
    let dx = sample_x - center_x;
    let dy = sample_y - center_y;
    (radius + 0.5 - (dx * dx + dy * dy).sqrt()).clamp(0.0, 1.0)
}

fn rect_coverage(sample_x: f32, sample_y: f32, x: f32, y: f32, width: f32, height: f32) -> f32 {
    if sample_x < x - 0.5
        || sample_y < y - 0.5
        || sample_x > x + width + 0.5
        || sample_y > y + height + 0.5
    {
        return 0.0;
    }
    let left = sample_x - x + 0.5;
    let right = x + width - sample_x + 0.5;
    let top = sample_y - y + 0.5;
    let bottom = y + height - sample_y + 0.5;
    left.min(right).min(top).min(bottom).clamp(0.0, 1.0)
}

fn clamp_ordered(value: f32, min: f32, max: f32) -> f32 {
    if min <= max {
        value.clamp(min, max)
    } else {
        (min + max) * 0.5
    }
}

fn column_widths(total_cols: usize, column_count: usize) -> Vec<usize> {
    if column_count == 0 {
        return Vec::new();
    }
    let usable = total_cols
        .saturating_sub(column_count + 1)
        .max(column_count);
    let base = (usable / column_count).clamp(1, 28);
    let mut widths = vec![base; column_count];
    let mut remainder = usable.saturating_sub(base * column_count);
    for width in &mut widths {
        if remainder == 0 {
            break;
        }
        *width += 1;
        remainder -= 1;
    }
    widths
}

fn draw_fill(
    changes: &mut Vec<Change>,
    x: usize,
    y: usize,
    width: usize,
    height: usize,
    background: ColorAttribute,
) {
    for row in 0..height {
        draw_cell(changes, x, y + row, width, "", lcars_black(), background);
    }
}

fn draw_cell(
    changes: &mut Vec<Change>,
    x: usize,
    y: usize,
    width: usize,
    text: &str,
    foreground: ColorAttribute,
    background: ColorAttribute,
) {
    if width == 0 {
        return;
    }
    changes.push(Change::CursorPosition {
        x: Position::Absolute(x),
        y: Position::Absolute(y),
    });
    changes.push(AttributeChange::Foreground(foreground).into());
    changes.push(AttributeChange::Background(background).into());
    let label = truncate_right(text, width);
    changes.push(Change::Text(format!("{label:<width$}")));
}

fn rgb(red: u8, green: u8, blue: u8) -> ColorAttribute {
    ColorAttribute::TrueColorWithDefaultFallback(RgbColor::new_8bpc(red, green, blue).into())
}

fn lcars_black() -> ColorAttribute {
    rgb(0, 0, 0)
}

fn lcars_orange() -> ColorAttribute {
    rgb(255, 153, 0)
}

fn lcars_gold() -> ColorAttribute {
    rgb(255, 204, 102)
}

fn lcars_butterscotch() -> ColorAttribute {
    rgb(204, 153, 102)
}

fn lcars_violet() -> ColorAttribute {
    rgb(180, 150, 255)
}

fn lcars_blue() -> ColorAttribute {
    rgb(102, 153, 255)
}

fn lcars_salmon() -> ColorAttribute {
    rgb(255, 102, 102)
}

fn lcars_steel() -> ColorAttribute {
    rgb(50, 70, 96)
}

fn lcars_cream() -> ColorAttribute {
    rgb(255, 238, 204)
}

fn lcars_silver() -> ColorAttribute {
    rgb(210, 210, 210)
}

fn lcars_aqua() -> ColorAttribute {
    rgb(0, 230, 230)
}

fn visible_row_indices(
    document: &LcarsDataViewDocument,
    query: &str,
    sort: Option<(usize, SortDirection)>,
) -> Vec<usize> {
    let query = query.trim().to_ascii_lowercase();
    let mut indices = document
        .rows
        .iter()
        .enumerate()
        .filter_map(|(index, row)| {
            if query.is_empty()
                || row
                    .iter()
                    .any(|cell| cell.to_ascii_lowercase().contains(&query))
            {
                Some(index)
            } else {
                None
            }
        })
        .collect::<Vec<_>>();

    if let Some((column, direction)) = sort {
        let numeric = indices.iter().all(|index| {
            document
                .rows
                .get(*index)
                .and_then(|row| row.get(column))
                .map(|value| value.trim().parse::<f64>().is_ok())
                .unwrap_or(false)
        });
        indices.sort_by(|left, right| {
            let left_value = document.rows[*left]
                .get(column)
                .map(String::as_str)
                .unwrap_or("");
            let right_value = document.rows[*right]
                .get(column)
                .map(String::as_str)
                .unwrap_or("");
            let ordering = if numeric {
                let left_number = left_value.trim().parse::<f64>().unwrap_or(0.0);
                let right_number = right_value.trim().parse::<f64>().unwrap_or(0.0);
                left_number
                    .partial_cmp(&right_number)
                    .unwrap_or(Ordering::Equal)
            } else {
                left_value
                    .to_ascii_lowercase()
                    .cmp(&right_value.to_ascii_lowercase())
            }
            .then_with(|| left.cmp(right));

            match direction {
                SortDirection::Ascending => ordering,
                SortDirection::Descending => ordering.reverse(),
            }
        });
    }

    indices
}

#[cfg(test)]
fn clear_pending_dataviews_for_tests() {
    PENDING_DATAVIEWS.lock().unwrap().clear();
}

#[cfg(test)]
mod tests {
    use super::*;
    use wezterm_term::StableRowIndex;

    fn event(name: &str, payload_json: &str) -> OwtTranscriptEvent {
        OwtTranscriptEvent {
            row: 0 as StableRowIndex,
            col: 0,
            seqno: 0,
            event: name.to_string(),
            payload_json: payload_json.to_string(),
        }
    }

    fn sample_json() -> String {
        r#"{"version":1,"kind":"owt.lcars.dataview","id":"fleet.demo","title":"Fleet Demo","columns":["Ship","Registry","Crew"],"rows":[["Enterprise","1701","430"],["Voyager","74656","150"],["Defiant","74205","50"]]}"#.to_string()
    }

    #[test]
    fn dataview_validation_accepts_minimal_document() {
        let document = validate_dataview_document(&sample_json()).unwrap();
        assert_eq!(document.id, "fleet.demo");
        assert_eq!(document.columns.len(), 3);
        assert_eq!(document.rows.len(), 3);
    }

    #[test]
    fn dataview_validation_rejects_extra_cells() {
        let json = r#"{"version":1,"kind":"owt.lcars.dataview","id":"bad","title":"Bad","columns":["A"],"rows":[["1","2"]]}"#;
        assert!(validate_dataview_document(json).is_err());
    }

    #[test]
    fn dataview_assembler_accepts_chunked_payload() {
        clear_pending_dataviews_for_tests();
        let json = sample_json();
        let split = json.len() / 2;
        accept_owt_dataview_event(&event(
            "owt.dataview.begin",
            &format!(
                r#"{{"version":1,"id":"fleet.demo","kind":"owt.lcars.dataview","chunks":2,"bytes":{}}}"#,
                json.len()
            ),
        ));
        accept_owt_dataview_event(&event(
            "owt.dataview.chunk",
            &format!(
                r#"{{"version":1,"id":"fleet.demo","index":0,"data":{}}}"#,
                serde_json::to_string(&json[..split]).unwrap()
            ),
        ));
        accept_owt_dataview_event(&event(
            "owt.dataview.chunk",
            &format!(
                r#"{{"version":1,"id":"fleet.demo","index":1,"data":{}}}"#,
                serde_json::to_string(&json[split..]).unwrap()
            ),
        ));
        let document = accept_owt_dataview_event(&event(
            "owt.dataview.end",
            r#"{"version":1,"id":"fleet.demo"}"#,
        ))
        .unwrap();
        assert_eq!(document.title, "Fleet Demo");
    }

    #[test]
    fn dataview_assembler_rejects_duplicate_chunk() {
        clear_pending_dataviews_for_tests();
        let json = sample_json();
        accept_owt_dataview_event(&event(
            "owt.dataview.begin",
            &format!(
                r#"{{"version":1,"id":"dup.demo","kind":"owt.lcars.dataview","chunks":1,"bytes":{}}}"#,
                json.len()
            ),
        ));
        let chunk = format!(
            r#"{{"version":1,"id":"dup.demo","index":0,"data":{}}}"#,
            serde_json::to_string(&json).unwrap()
        );
        accept_owt_dataview_event(&event("owt.dataview.chunk", &chunk));
        accept_owt_dataview_event(&event("owt.dataview.chunk", &chunk));
        assert!(accept_owt_dataview_event(&event(
            "owt.dataview.end",
            r#"{"version":1,"id":"dup.demo"}"#,
        ))
        .is_none());
    }

    #[test]
    fn dataview_sort_uses_numeric_order_when_possible() {
        let document = validate_dataview_document(&sample_json()).unwrap();
        let indices = visible_row_indices(&document, "", Some((2, SortDirection::Ascending)));
        let names = indices
            .iter()
            .map(|index| document.rows[*index][0].as_str())
            .collect::<Vec<_>>();
        assert_eq!(names, vec!["Defiant", "Voyager", "Enterprise"]);
    }

    #[test]
    fn dataview_layout_requires_native_pixel_metrics_for_image_chrome() {
        let plain = DataViewLayout::new(120, 40, 0, 0);
        assert!(!plain.supports_image_chrome());

        let native = DataViewLayout::new(120, 40, 8, 18);
        assert!(native.supports_image_chrome());
    }

    #[test]
    fn lcars_raster_generates_rounded_alpha_chrome() {
        let mut raster = LcarsRaster::new(24, 12);
        raster.fill_rounded_rect(0.0, 0.0, 24.0, 12.0, 6.0, BYTE_ORANGE);

        let center = ((6 * 24 + 12) * 4) as usize;
        assert_eq!(raster.pixels[center + 3], 255);
        assert!(raster.pixels[3] < raster.pixels[center + 3]);
    }

    #[test]
    fn lcars_raster_encodes_png_for_terminfo_image_transport() {
        let mut raster = LcarsRaster::new(8, 8);
        raster.fill_rect(0.0, 0.0, 8.0, 8.0, BYTE_ORANGE);
        let encoded = raster.into_png_bytes().unwrap();
        assert_eq!(&encoded[..8], b"\x89PNG\r\n\x1a\n");
    }
}
