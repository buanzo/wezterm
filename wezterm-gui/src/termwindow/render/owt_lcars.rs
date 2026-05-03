use crate::quad::TripleLayerQuadAllocator;
use crate::termwindow::render::RenderScreenLineParams;
use crate::termwindow::{UIItem, UIItemType};
use anyhow::Context;
use mux::renderable::{RenderableDimensions, StableCursorPosition};
use owt_control::{InterfaceDocument, UiNode, UiNodeKind};
use std::sync::Arc;
use termwiz::cell::Intensity;
use termwiz::color::{ColorSpec, RgbColor};
use termwiz::image::{ImageCell, ImageData, ImageDataType, TextureCoordinate};
use wezterm_term::{CellAttributes, Line};
use window::{
    color::LinearRgba, KeyCode, KeyEvent, Modifiers, PhysKeyCode, PixelUnit, RectF, WindowOps,
};

const LCARS_PANEL_MARGIN: f32 = 8.0;
const LCARS_PANEL_GAP: f32 = 12.0;
const LCARS_PANEL_MIN_HEIGHT: f32 = 166.0;
const LCARS_PANEL_MAX_HEIGHT: f32 = 196.0;
const LCARS_STRUCTURAL_PANEL_MIN_HEIGHT: f32 = 326.0;
const LCARS_STRUCTURAL_PANEL_MAX_HEIGHT: f32 = 392.0;
const LCARS_PANEL_ROW_HEIGHT: f32 = 8.2;
const LCARS_STRUCTURAL_PANEL_ROW_HEIGHT: f32 = 20.0;
const LCARS_LEFT_RAIL_RESERVED: f32 = 146.0;
const LCARS_SIDE_PANEL_MIN_WIDTH: f32 = 300.0;
const LCARS_SIDE_PANEL_MAX_WIDTH: f32 = 440.0;
const LCARS_BOTTOM_PANEL_MIN_HEIGHT: f32 = 132.0;
const LCARS_BOTTOM_PANEL_MAX_HEIGHT: f32 = 176.0;
const LCARS_BOTTOM_PANEL_ROW_HEIGHT: f32 = 7.0;
const LCARS_MIN_TERMINAL_REMAINDER: f32 = 420.0;
const LCARS_MAX_PANEL_LINES: usize = 18;
const LCARS_KEY_ACTION_LIMIT: usize = 9;
const LCARS_SIGNAL_TEXT_MAX_LINES: usize = 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LcarsPanelLayout {
    Top,
    Right,
    Bottom,
    Overlay,
}

#[derive(Debug, Default, Clone, Copy)]
struct LcarsReservedPixels {
    left: f32,
    top: f32,
    right: f32,
    bottom: f32,
}

#[derive(Clone, Copy)]
struct LcarsPalette {
    black: LinearRgba,
    orange: LinearRgba,
    amber: LinearRgba,
    peach: LinearRgba,
    violet: LinearRgba,
    blue: LinearRgba,
    cyan: LinearRgba,
    red: LinearRgba,
    dim_blue: LinearRgba,
    dim_violet: LinearRgba,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct LcarsByteColor {
    red: u8,
    green: u8,
    blue: u8,
    alpha: u8,
}

impl LcarsByteColor {
    const fn rgba(red: u8, green: u8, blue: u8, alpha: u8) -> Self {
        Self {
            red,
            green,
            blue,
            alpha,
        }
    }

    const fn rgb(red: u8, green: u8, blue: u8) -> Self {
        Self::rgba(red, green, blue, 255)
    }

    fn with_alpha(self, alpha: u8) -> Self {
        Self { alpha, ..self }
    }
}

const LCARS_BYTE_BLACK: LcarsByteColor = LcarsByteColor::rgb(0, 0, 0);
const LCARS_BYTE_SCREEN: LcarsByteColor = LcarsByteColor::rgb(255, 240, 176);
const LCARS_BYTE_ORANGE: LcarsByteColor = LcarsByteColor::rgb(255, 156, 0);
const LCARS_BYTE_AMBER: LcarsByteColor = LcarsByteColor::rgb(255, 204, 102);
const LCARS_BYTE_PEACH: LcarsByteColor = LcarsByteColor::rgb(255, 153, 102);
const LCARS_BYTE_VIOLET: LcarsByteColor = LcarsByteColor::rgb(204, 153, 255);
const LCARS_BYTE_BLUE: LcarsByteColor = LcarsByteColor::rgb(102, 153, 204);

impl LcarsPalette {
    fn new() -> Self {
        let black = color(0, 0, 0);
        let blue = color(102, 153, 204);
        let violet = color(204, 153, 255);
        Self {
            black,
            orange: color(255, 156, 0),
            amber: color(255, 204, 102),
            peach: color(255, 153, 102),
            violet,
            blue,
            cyan: color(153, 204, 255),
            red: color(255, 102, 102),
            dim_blue: with_alpha(blue, 0.82),
            dim_violet: with_alpha(violet, 0.82),
        }
    }

    fn panel_background(self, overlay: bool) -> LinearRgba {
        if overlay {
            with_alpha(self.black, 0.92)
        } else {
            self.black
        }
    }

    fn action_fill(self, index: usize) -> LinearRgba {
        match index % 4 {
            0 => self.peach,
            1 => self.violet,
            2 => self.blue,
            _ => self.amber,
        }
    }

    fn signal_fill(self, index: usize) -> LinearRgba {
        match index % 4 {
            0 => self.cyan,
            1 => self.amber,
            2 => self.dim_violet,
            _ => self.peach,
        }
    }
}

impl crate::TermWindow {
    pub fn owt_lcars_reserved_left_pixels(&self) -> f32 {
        self.owt_lcars_reserved_pixels().left
    }

    pub fn owt_lcars_reserved_top_pixels(&self) -> f32 {
        self.owt_lcars_reserved_pixels().top
    }

    pub fn owt_lcars_reserved_right_pixels(&self) -> f32 {
        self.owt_lcars_reserved_pixels().right
    }

    pub fn owt_lcars_reserved_bottom_pixels(&self) -> f32 {
        self.owt_lcars_reserved_pixels().bottom
    }

    pub(crate) fn reflow_owt_lcars_layout(&mut self) {
        let Some(window) = self.window.clone() else {
            return;
        };
        let dimensions = self.dimensions;
        self.apply_dimensions(&dimensions, None, &window);
        window.invalidate();
    }

    pub(crate) fn dispatch_owt_lcars_action(&mut self, action_id: &str, context: &dyn WindowOps) {
        match crate::owt_native::dispatch_action(action_id) {
            Ok(dispatched) => {
                log::debug!("OWT LCARS action dispatched: {}", dispatched.action_id);
                context.invalidate();
            }
            Err(err) => {
                log::warn!("OWT LCARS action dispatch failed for {action_id}: {err:#}");
            }
        }
    }

    pub(crate) fn dispatch_owt_lcars_keyboard_shortcut(
        &mut self,
        event: &KeyEvent,
        context: &dyn WindowOps,
    ) -> bool {
        if !event
            .modifiers
            .contains(Modifiers::CTRL | Modifiers::ALT | Modifiers::SHIFT)
        {
            return false;
        }

        let physical_key = event.raw.as_ref().and_then(|raw| raw.phys_code.as_ref());
        let Some(index) = event
            .raw
            .as_ref()
            .and_then(|raw| lcars_shortcut_index(&raw.key, raw.phys_code.as_ref()))
            .or_else(|| lcars_shortcut_index(&event.key, physical_key))
        else {
            return false;
        };
        let Some(document) = crate::owt_native::active_interface_snapshot() else {
            return false;
        };
        let Some(action_id) = lcars_keyboard_action_slots(&document).get(index).cloned() else {
            return false;
        };

        self.dispatch_owt_lcars_action(&action_id, context);
        true
    }

    fn owt_lcars_reserved_pixels(&self) -> LcarsReservedPixels {
        let Some(document) = crate::owt_native::active_interface_snapshot() else {
            return LcarsReservedPixels::default();
        };

        match lcars_panel_layout(&document) {
            LcarsPanelLayout::Top => {
                let available_height =
                    self.dimensions.pixel_height as f32 - (LCARS_PANEL_MARGIN * 2.0);
                let structural = has_structural_lcars_layout(&document);
                let Some(panel_height) =
                    self.owt_lcars_panel_height_for(available_height, structural)
                else {
                    return LcarsReservedPixels::default();
                };
                LcarsReservedPixels {
                    left: if structural {
                        LCARS_PANEL_MARGIN + LCARS_LEFT_RAIL_RESERVED
                    } else {
                        0.0
                    },
                    top: LCARS_PANEL_MARGIN + panel_height + LCARS_PANEL_GAP,
                    ..Default::default()
                }
            }
            LcarsPanelLayout::Right => {
                let available_width =
                    self.dimensions.pixel_width as f32 - (LCARS_PANEL_MARGIN * 2.0);
                let Some(panel_width) = self.owt_lcars_side_panel_width(available_width) else {
                    return LcarsReservedPixels::default();
                };
                LcarsReservedPixels {
                    right: LCARS_PANEL_MARGIN + panel_width + LCARS_PANEL_GAP,
                    ..Default::default()
                }
            }
            LcarsPanelLayout::Bottom => {
                let available_height =
                    self.dimensions.pixel_height as f32 - (LCARS_PANEL_MARGIN * 2.0);
                let Some(panel_height) = self.owt_lcars_bottom_panel_height(available_height)
                else {
                    return LcarsReservedPixels::default();
                };
                LcarsReservedPixels {
                    bottom: LCARS_PANEL_MARGIN + panel_height + LCARS_PANEL_GAP,
                    ..Default::default()
                }
            }
            LcarsPanelLayout::Overlay => LcarsReservedPixels::default(),
        }
    }

    pub fn paint_owt_lcars_panel(
        &mut self,
        layers: &mut TripleLayerQuadAllocator,
    ) -> anyhow::Result<()> {
        let Some(document) = crate::owt_native::active_interface_snapshot() else {
            return Ok(());
        };
        let layout = lcars_panel_layout(&document);
        crate::owt_native::record_lcars_render_pass();

        if layout == LcarsPanelLayout::Right {
            return self.paint_owt_lcars_side_panel(layers, &document);
        }
        if layout == LcarsPanelLayout::Bottom {
            return self.paint_owt_lcars_bottom_panel(layers, &document);
        }

        let cell_width = self.render_metrics.cell_size.width as f32;
        let cell_height = self.render_metrics.cell_size.height as f32;
        let border = self.get_os_border();
        let tab_bar_height = if self.show_tab_bar && !self.config.tab_bar_at_bottom {
            self.tab_bar_pixel_height()
                .context("compute tab bar height for OWT LCARS panel")?
        } else {
            0.0
        };

        let margin = LCARS_PANEL_MARGIN;
        let top = border.top.get() as f32 + tab_bar_height + margin;
        let available_height = self.dimensions.pixel_height as f32 - top - margin;
        if has_structural_lcars_layout(&document) {
            return self.paint_owt_lcars_structural_top_panel(layers, &document, layout);
        }

        let Some(panel_height) = self.owt_lcars_panel_height_for(available_height, false) else {
            return Ok(());
        };

        let panel_width = (self.dimensions.pixel_width as f32 - (margin * 2.0)).max(360.0);
        let left = margin;
        let right = left + panel_width;
        let bottom = top + panel_height;

        let lcars = LcarsPalette::new();
        let panel_bg = lcars.panel_background(layout == LcarsPanelLayout::Overlay);
        let last_action_id =
            crate::owt_native::last_dispatched_action_snapshot().map(|action| action.action_id);

        self.filled_rectangle(
            layers,
            0,
            rect(left, top, panel_width, panel_height),
            panel_bg,
        )?;

        let rail_width = 44.0;
        self.filled_rectangle(
            layers,
            0,
            rect(left + 8.0, top + 8.0, rail_width, panel_height - 16.0),
            lcars.orange,
        )?;
        self.filled_rectangle(
            layers,
            0,
            rect(left + 8.0, top + 8.0, rail_width, 24.0),
            lcars.peach,
        )?;
        self.filled_rectangle(
            layers,
            0,
            rect(left + 8.0, top + 38.0, rail_width, 5.0),
            panel_bg,
        )?;
        self.filled_rectangle(
            layers,
            0,
            rect(left + 8.0, bottom - 42.0, rail_width, 5.0),
            panel_bg,
        )?;
        self.filled_rectangle(
            layers,
            0,
            rect(left + 8.0, bottom - 34.0, rail_width, 26.0),
            lcars.violet,
        )?;

        let content_left = left + 64.0;
        let content_top = top + 10.0;
        let header_height = 20.0;
        let action_width = (panel_width * 0.22).clamp(210.0, 310.0);
        let action_left = (right - action_width - 16.0).max(content_left + 260.0);
        let signal_right = (action_left - 18.0).max(content_left + 260.0);
        let signal_width = signal_right - content_left;
        let primary_header_width = (signal_width * 0.56).max(160.0);
        let header_segment_gap = 10.0;
        let header_segment_width = 92.0;

        paint_lcars_right_cap_bar(
            self,
            layers,
            content_left,
            content_top,
            primary_header_width.min(signal_width),
            header_height,
            LCARS_BYTE_ORANGE,
        )?;
        paint_lcars_right_cap_bar(
            self,
            layers,
            content_left + primary_header_width + header_segment_gap,
            content_top,
            header_segment_width,
            header_height,
            LCARS_BYTE_VIOLET,
        )?;
        let final_header_x =
            content_left + primary_header_width + header_segment_gap + header_segment_width + 10.0;
        let final_header_width = signal_right - final_header_x;
        if final_header_width > 24.0 {
            paint_lcars_right_cap_bar(
                self,
                layers,
                final_header_x,
                content_top,
                final_header_width,
                header_height,
                LCARS_BYTE_BLUE.with_alpha(210),
            )?;
        }
        self.filled_rectangle(
            layers,
            0,
            rect(content_left, top + 36.0, signal_width, 2.0),
            lcars.amber,
        )?;
        self.filled_rectangle(
            layers,
            0,
            rect(content_left, top + 42.0, signal_width * 0.28, 2.0),
            lcars.dim_violet,
        )?;
        paint_lcars_right_cap_bar(
            self,
            layers,
            action_left,
            content_top,
            action_width,
            header_height,
            LCARS_BYTE_BLUE.with_alpha(220),
        )?;
        self.filled_rectangle(
            layers,
            0,
            rect(action_left, top + 36.0, action_width, 2.0),
            lcars.cyan,
        )?;
        self.filled_rectangle(
            layers,
            0,
            rect(action_left, top + 42.0, action_width * 0.36, 2.0),
            lcars.peach,
        )?;

        let marker_gap = 18.0;
        let text_left = content_left + marker_gap;
        let signal_cols = ((signal_width - marker_gap - 16.0) / cell_width).max(8.0) as usize;
        let action_cols = ((action_width - 24.0) / cell_width).max(8.0) as usize;
        let max_cols = signal_cols.max(action_cols);
        let lines = panel_lines(&document, max_cols);
        let footer_rule_y = bottom - 20.0;
        let footer_text_y = bottom - 18.0;
        let footer_guard_y = footer_rule_y - 4.0;

        self.paint_owt_panel_text(
            layers,
            text_left,
            content_top + 2.0,
            signal_cols,
            &lines.title,
            RgbColor::new_8bpc(0, 0, 0),
            true,
        )?;
        self.paint_owt_panel_text(
            layers,
            content_left + 6.0,
            top + 48.0,
            signal_cols,
            &lines.scope,
            RgbColor::new_8bpc(255, 204, 102),
            false,
        )?;
        self.paint_owt_panel_text(
            layers,
            action_left + 12.0,
            content_top + 2.0,
            action_cols,
            "COMMAND",
            RgbColor::new_8bpc(0, 0, 0),
            true,
        )?;

        let mut signal_y = top + 48.0 + (cell_height * 1.08);
        for (index, line) in lines
            .items
            .iter()
            .filter(|line| line.action_id.is_none())
            .take(4)
            .enumerate()
        {
            if signal_y + cell_height > footer_guard_y {
                break;
            }
            let color = lcars_signal_text_color(line.kind, index);
            let bar = lcars.signal_fill(index);
            paint_lcars_signal_marker(
                self,
                layers,
                content_left,
                signal_y,
                line.kind,
                signal_width,
                cell_height,
                bar,
            )?;
            if line.kind == PanelLineKind::Progress {
                paint_progress_rail(
                    self,
                    layers,
                    content_left + signal_width - 124.0,
                    signal_y + 5.0,
                    112.0,
                    cell_height * 0.45,
                    line.progress,
                    lcars.dim_blue,
                    lcars.amber,
                )?;
            }
            self.paint_owt_panel_text(
                layers,
                text_left,
                signal_y,
                signal_cols,
                &line.text,
                color,
                false,
            )?;
            signal_y += cell_height * 1.16;
        }

        let mut action_y = top + 48.0;
        let mut action_count = 0usize;
        let button_height = (cell_height * 1.02).clamp(17.0, 21.0);
        for (index, line) in lines
            .items
            .iter()
            .filter(|line| line.action_id.is_some())
            .take(4)
            .enumerate()
        {
            if action_y + button_height > footer_guard_y {
                break;
            }
            let active = last_action_id.as_deref() == line.action_id.as_deref();
            paint_lcars_action_button(
                self,
                layers,
                action_left,
                action_y,
                action_width,
                button_height,
                action_cols,
                &line.text,
                lcars_action_text_color(index),
                lcars.action_fill(index),
                lcars_action_fill_byte(index),
                active,
                lcars,
            )?;
            if let Some(action_id) = &line.action_id {
                self.ui_items.push(UIItem {
                    x: action_left.max(0.0) as usize,
                    y: action_y.max(0.0) as usize,
                    width: action_width.ceil().max(1.0) as usize,
                    height: button_height.ceil().max(1.0) as usize,
                    item_type: UIItemType::OwtLcarsAction(action_id.clone()),
                });
            }
            action_count += 1;
            action_y += button_height + 5.0;
        }

        if action_count == 0 && action_y + button_height <= footer_guard_y {
            self.filled_rectangle(
                layers,
                0,
                rect(action_left, action_y, action_width, button_height),
                lcars.red,
            )?;
            self.paint_owt_panel_text(
                layers,
                action_left + 12.0,
                action_y + 4.0,
                action_cols,
                "STATE ONLY",
                RgbColor::new_8bpc(255, 102, 102),
                true,
            )?;
        }

        let footer_text = if let Some(action_id) = last_action_id.as_deref() {
            format!("NATIVE RUNTIME / ACK {action_id}")
        } else if lines.has_actions {
            "NATIVE RUNTIME / HITBOX + KEY DISPATCH".to_string()
        } else {
            "NATIVE RUNTIME / DISPLAY ONLY".to_string()
        };
        self.filled_rectangle(
            layers,
            0,
            rect(left + 68.0, footer_rule_y, panel_width - 84.0, 3.0),
            lcars.dim_blue,
        )?;
        self.paint_owt_panel_text(
            layers,
            left + 76.0,
            footer_text_y,
            ((panel_width - 100.0) / cell_width).max(8.0) as usize,
            &footer_text,
            RgbColor::new_8bpc(153, 204, 255),
            false,
        )?;

        Ok(())
    }

    fn paint_owt_lcars_structural_top_panel(
        &mut self,
        layers: &mut TripleLayerQuadAllocator,
        document: &InterfaceDocument,
        layout: LcarsPanelLayout,
    ) -> anyhow::Result<()> {
        let cell_width = self.render_metrics.cell_size.width as f32;
        let cell_height = self.render_metrics.cell_size.height as f32;
        let border = self.get_os_border();
        let tab_bar_height = if self.show_tab_bar && !self.config.tab_bar_at_bottom {
            self.tab_bar_pixel_height()
                .context("compute tab bar height for OWT structural LCARS panel")?
        } else {
            0.0
        };
        let bottom_bar_height = if self.show_tab_bar && self.config.tab_bar_at_bottom {
            self.tab_bar_pixel_height()
                .context("compute bottom tab bar height for OWT structural LCARS panel")?
        } else {
            0.0
        };

        let margin = LCARS_PANEL_MARGIN;
        let top = border.top.get() as f32 + tab_bar_height + margin;
        let available_height = self.dimensions.pixel_height as f32 - top - margin;
        let Some(panel_height) = self.owt_lcars_panel_height_for(available_height, true) else {
            return Ok(());
        };

        let panel_width = (self.dimensions.pixel_width as f32 - (margin * 2.0)).max(520.0);
        let left = margin;
        let right = left + panel_width;
        let bottom = top + panel_height;
        let window_bottom = self.dimensions.pixel_height as f32
            - border.bottom.get() as f32
            - bottom_bar_height
            - margin;

        let lcars = LcarsPalette::new();
        let panel_bg = lcars.panel_background(layout == LcarsPanelLayout::Overlay);
        let last_action_id =
            crate::owt_native::last_dispatched_action_snapshot().map(|action| action.action_id);
        let lines = panel_lines(
            document,
            ((panel_width - 120.0) / cell_width).max(8.0) as usize,
        );

        self.filled_rectangle(
            layers,
            0,
            rect(left, top, panel_width, panel_height),
            panel_bg,
        )?;

        let rail_left = left + 10.0;
        let rail_width = 88.0;
        let content_left = left + LCARS_LEFT_RAIL_RESERVED;
        let content_right = right - 12.0;
        let content_width = (content_right - content_left).max(320.0);
        let header_y = top + 10.0;
        let header_h = 38.0;
        let rail_bottom = window_bottom.max(bottom);
        paint_lcars_side_rail_chrome(
            self,
            layers,
            rail_left,
            top + 10.0,
            rail_width,
            (rail_bottom - top - 18.0).max(panel_height),
            lcars,
        )?;
        paint_lcars_primary_elbow(
            self,
            layers,
            rail_left,
            header_y,
            (content_left - rail_left + (content_width * 0.28)).clamp(240.0, 620.0),
            (panel_height * 0.58).clamp(148.0, 220.0),
            rail_width,
            header_h,
            lcars,
        )?;
        paint_lcars_bar_run(
            self,
            layers,
            content_left,
            header_y,
            content_width,
            header_h,
        )?;

        self.filled_rectangle(
            layers,
            0,
            rect(content_left, header_y + header_h + 7.0, content_width, 3.0),
            lcars.amber,
        )?;
        self.filled_rectangle(
            layers,
            0,
            rect(
                content_left,
                header_y + header_h + 15.0,
                content_width * 0.27,
                3.0,
            ),
            lcars.dim_violet,
        )?;

        let content_bay_y = bottom - 52.0;
        let signal_top = top + 90.0;
        let signal_limit = content_bay_y - 8.0;
        let has_table = document_has_kind(document, |kind| matches!(kind, UiNodeKind::Table));
        let has_data_cascade =
            document_has_kind(document, |kind| matches!(kind, UiNodeKind::DataCascade));
        let action_lines = lines
            .items
            .iter()
            .filter(|line| line.action_id.is_some())
            .collect::<Vec<_>>();
        let layout_plan = plan_lcars_structural_console(
            content_left,
            content_right,
            signal_top,
            signal_limit,
            cell_width,
            cell_height,
            has_table,
            has_data_cascade,
            action_lines.len(),
        );
        let table_data = has_table
            .then(|| {
                lcars_table_data(
                    document,
                    layout_plan.table_max_columns(cell_width),
                    layout_plan.table_max_rows(cell_height),
                )
            })
            .flatten();
        let table_detail_ready =
            table_data.is_some() && layout_plan.detail != LcarsDetailPlacement::None;
        let signal_cols = (layout_plan.signal_width / cell_width).max(8.0) as usize;
        let signal_visual_limit =
            structural_signal_visual_limit(&layout_plan, signal_limit, cell_height);
        let action_cols = if layout_plan.command_visible {
            ((layout_plan.command_width - 26.0) / cell_width).max(8.0) as usize
        } else {
            ((content_width * 0.52) / cell_width).max(8.0) as usize
        };
        let button_height = (cell_height * 2.15).clamp(36.0, 46.0);
        let button_gap = 12.0;
        let two_columns = layout_plan.two_action_columns;
        let button_width = if two_columns {
            ((layout_plan.command_width - button_gap) * 0.5).max(132.0)
        } else {
            layout_plan.command_width
        };
        let action_columns = if two_columns { 2 } else { 1 };
        let action_rows = if layout_plan.command_visible {
            let visible_actions = action_lines.len().min(layout_plan.action_slots);
            (visible_actions + action_columns - 1) / action_columns
        } else {
            0
        };

        self.paint_owt_panel_text(
            layers,
            content_left + 12.0,
            header_y + 6.0,
            ((content_width * 0.46) / cell_width).max(8.0) as usize,
            &lines.title,
            RgbColor::new_8bpc(0, 0, 0),
            true,
        )?;
        self.paint_owt_panel_text(
            layers,
            content_left + 12.0,
            top + 64.0,
            signal_cols,
            &lines.scope,
            RgbColor::new_8bpc(255, 204, 102),
            false,
        )?;

        paint_lcars_structural_console_chrome(
            self,
            layers,
            content_left,
            content_right,
            signal_top,
            content_bay_y,
            &layout_plan,
            action_rows,
            button_height,
            button_gap,
            lcars,
        )?;
        paint_lcars_content_bay_frame(
            self,
            layers,
            content_left,
            content_bay_y,
            content_width,
            38.0,
            lcars,
        )?;
        self.paint_owt_panel_text(
            layers,
            content_left + 18.0,
            content_bay_y + 12.0,
            ((content_width * 0.52) / cell_width).max(8.0) as usize,
            "CONTENT BAY / TERMINAL BELOW",
            RgbColor::new_8bpc(153, 204, 255),
            false,
        )?;

        let mut signal_y = signal_top;
        let signal_lines = lines
            .items
            .iter()
            .filter(|line| line.action_id.is_none())
            .filter(|line| !matches!(line.kind, PanelLineKind::Frame | PanelLineKind::CommandGrid))
            .filter(|line| !table_detail_ready || line.kind != PanelLineKind::Table)
            .collect::<Vec<_>>();
        let mut shown_signals = 0usize;
        let mut used_signal_rows = 0usize;
        for (index, line) in signal_lines.iter().enumerate() {
            let remaining_rows = layout_plan.signal_rows.saturating_sub(used_signal_rows);
            if remaining_rows == 0 || signal_y + cell_height > signal_visual_limit {
                break;
            }
            let text_cols = structural_signal_text_cols(
                line.kind,
                layout_plan.signal_width,
                cell_width,
                signal_cols,
            );
            let max_text_lines = remaining_rows.min(LCARS_SIGNAL_TEXT_MAX_LINES);
            let wrapped_text = wrap_text_lines(&line.text, text_cols, max_text_lines);
            let text_rows = wrapped_text.len().max(1);
            if signal_y + (cell_height * text_rows as f32) > signal_visual_limit {
                break;
            }
            let backing_extra = if structural_signal_text_needs_backing(line.kind) {
                16.0
            } else {
                10.0
            };
            let backing_width = ((text_cols as f32 * cell_width) + backing_extra)
                .min((layout_plan.signal_width - 20.0).max(cell_width * 8.0));
            paint_lcars_signal_label_bar(
                self,
                layers,
                content_left,
                signal_y,
                line.kind,
                layout_plan.signal_width,
                backing_width,
                text_rows,
                cell_height,
                lcars.signal_fill(index),
                lcars,
            )?;
            if line.kind == PanelLineKind::Progress {
                paint_progress_rail(
                    self,
                    layers,
                    content_left + layout_plan.signal_width - 120.0,
                    signal_y + 5.0,
                    108.0,
                    cell_height * 0.42,
                    line.progress,
                    lcars.dim_blue,
                    lcars.amber,
                )?;
            }
            for (text_index, text) in wrapped_text.iter().enumerate() {
                self.paint_owt_panel_text(
                    layers,
                    content_left + 18.0,
                    signal_y + (text_index as f32 * cell_height),
                    text_cols,
                    text,
                    lcars_signal_text_color(line.kind, index),
                    false,
                )?;
            }
            signal_y += cell_height * (text_rows as f32 + 0.18);
            used_signal_rows += text_rows;
            shown_signals += 1;
        }
        let hidden_signals = signal_lines.len().saturating_sub(shown_signals);
        if hidden_signals > 0 && signal_y + cell_height <= signal_visual_limit {
            let overflow_label = format!("+{hidden_signals} SIGNALS");
            let overflow_cols = signal_cols.saturating_sub(2);
            let overflow_y = (signal_y + (cell_height * 0.25))
                .min(signal_visual_limit - cell_height)
                .max(signal_y);
            self.filled_rectangle(
                layers,
                1,
                rect(
                    content_left,
                    overflow_y - 5.0,
                    layout_plan.signal_width,
                    cell_height + 10.0,
                ),
                lcars.black,
            )?;
            self.filled_rectangle(
                layers,
                1,
                rect(
                    content_left + 12.0,
                    overflow_y + 3.0,
                    10.0,
                    cell_height * 0.72,
                ),
                lcars.peach,
            )?;
            self.paint_owt_panel_text(
                layers,
                content_left + 28.0,
                overflow_y,
                overflow_cols,
                &overflow_label,
                RgbColor::new_8bpc(255, 153, 102),
                true,
            )?;
        }

        if let Some(table) = table_data.as_ref().filter(|_| table_detail_ready) {
            paint_lcars_table_bay(
                self,
                layers,
                layout_plan.detail_left,
                layout_plan.detail_top,
                layout_plan.detail_width,
                layout_plan.detail_height,
                table,
                lcars,
            )?;
        } else if layout_plan.detail != LcarsDetailPlacement::None && has_data_cascade {
            paint_lcars_data_cascade(
                self,
                layers,
                layout_plan.detail_left,
                layout_plan.detail_top,
                layout_plan.detail_width,
                layout_plan.detail_height,
                document,
                lcars,
            )?;
        }

        if layout_plan.command_visible {
            self.paint_owt_panel_text(
                layers,
                layout_plan.command_left + 12.0,
                top + 64.0,
                action_cols,
                "COMMAND BANK",
                RgbColor::new_8bpc(255, 153, 102),
                true,
            )?;
        }
        let mut action_count = 0usize;
        for (index, line) in action_lines
            .iter()
            .copied()
            .take(layout_plan.action_slots)
            .enumerate()
        {
            let col = if two_columns { index % 2 } else { 0 };
            let row = if two_columns { index / 2 } else { index };
            let x = layout_plan.command_left + (col as f32 * (button_width + button_gap));
            let y = top + 92.0 + (row as f32 * (button_height + button_gap));
            if y + button_height > signal_limit {
                break;
            }
            let active = last_action_id.as_deref() == line.action_id.as_deref();
            paint_lcars_action_button(
                self,
                layers,
                x,
                y,
                button_width,
                button_height,
                ((button_width - 22.0) / cell_width).max(6.0) as usize,
                &line.text,
                lcars_action_text_color(index),
                lcars.action_fill(index),
                lcars_action_fill_byte(index),
                active,
                lcars,
            )?;
            if let Some(action_id) = &line.action_id {
                self.ui_items.push(UIItem {
                    x: x.max(0.0) as usize,
                    y: y.max(0.0) as usize,
                    width: button_width.ceil().max(1.0) as usize,
                    height: button_height.ceil().max(1.0) as usize,
                    item_type: UIItemType::OwtLcarsAction(action_id.clone()),
                });
            }
            action_count += 1;
        }
        let hidden_actions = action_lines.len().saturating_sub(action_count);
        if layout_plan.command_visible && hidden_actions > 0 {
            self.paint_owt_panel_text(
                layers,
                layout_plan.command_left + 12.0,
                (signal_limit - cell_height).max(top + 92.0),
                action_cols,
                &format!("+{hidden_actions} ACTIONS"),
                RgbColor::new_8bpc(255, 204, 102),
                true,
            )?;
        }

        let footer_text = if let Some(action_id) = last_action_id.as_deref() {
            format!("NATIVE RUNTIME / ACK {action_id}")
        } else if action_count > 0 {
            "NATIVE RUNTIME / FRAME + HITBOX + ATOMIC BUILD".to_string()
        } else {
            "NATIVE RUNTIME / STRUCTURAL LCARS FRAME".to_string()
        };
        let footer_left = if layout_plan.command_visible {
            layout_plan.command_left + 12.0
        } else {
            content_left + 18.0
        };
        self.paint_owt_panel_text(
            layers,
            footer_left,
            content_bay_y + 8.0,
            action_cols,
            &footer_text,
            RgbColor::new_8bpc(153, 204, 255),
            false,
        )?;

        Ok(())
    }

    fn paint_owt_lcars_side_panel(
        &mut self,
        layers: &mut TripleLayerQuadAllocator,
        document: &InterfaceDocument,
    ) -> anyhow::Result<()> {
        let cell_width = self.render_metrics.cell_size.width as f32;
        let cell_height = self.render_metrics.cell_size.height as f32;
        let border = self.get_os_border();
        let top_bar_height = if self.show_tab_bar && !self.config.tab_bar_at_bottom {
            self.tab_bar_pixel_height()
                .context("compute tab bar height for OWT LCARS side panel")?
        } else {
            0.0
        };
        let bottom_bar_height = if self.show_tab_bar && self.config.tab_bar_at_bottom {
            self.tab_bar_pixel_height()
                .context("compute bottom tab bar height for OWT LCARS side panel")?
        } else {
            0.0
        };

        let margin = LCARS_PANEL_MARGIN;
        let available_width = self.dimensions.pixel_width as f32 - (margin * 2.0);
        let Some(panel_width) = self.owt_lcars_side_panel_width(available_width) else {
            return Ok(());
        };
        let left = self.dimensions.pixel_width as f32 - margin - panel_width;
        let top = border.top.get() as f32 + top_bar_height + margin;
        let bottom = self.dimensions.pixel_height as f32
            - border.bottom.get() as f32
            - bottom_bar_height
            - margin;
        let panel_height = (bottom - top).max(0.0);
        if panel_height < cell_height * 9.0 {
            return Ok(());
        }

        let lcars = LcarsPalette::new();
        let panel_bg = lcars.black;
        let last_action_id =
            crate::owt_native::last_dispatched_action_snapshot().map(|action| action.action_id);

        self.filled_rectangle(
            layers,
            0,
            rect(left, top, panel_width, panel_height),
            panel_bg,
        )?;
        self.filled_rectangle(
            layers,
            0,
            rect(left + 8.0, top + 10.0, 34.0, 96.0),
            lcars.orange,
        )?;
        self.filled_rectangle(
            layers,
            0,
            rect(left + 8.0, top + 10.0, 34.0, 28.0),
            lcars.peach,
        )?;
        self.filled_rectangle(
            layers,
            0,
            rect(left + 8.0, bottom - 70.0, 34.0, 60.0),
            lcars.violet,
        )?;
        self.filled_rectangle(
            layers,
            0,
            rect(left + 52.0, top + 10.0, panel_width - 68.0, 30.0),
            lcars.dim_blue,
        )?;
        self.filled_rectangle(
            layers,
            0,
            rect(left + 52.0, top + 48.0, panel_width - 68.0, 3.0),
            lcars.cyan,
        )?;
        self.filled_rectangle(
            layers,
            0,
            rect(left + 52.0, bottom - 28.0, panel_width - 68.0, 3.0),
            lcars.dim_violet,
        )?;

        let text_left = left + 60.0;
        let text_width = (panel_width - 76.0).max(cell_width * 8.0);
        let text_cols = (text_width / cell_width).max(8.0) as usize;
        let lines = panel_lines(document, text_cols);

        self.paint_owt_panel_text(
            layers,
            text_left,
            top + 15.0,
            text_cols,
            &lines.title,
            RgbColor::new_8bpc(0, 0, 0),
            true,
        )?;
        self.paint_owt_panel_text(
            layers,
            text_left,
            top + 58.0,
            text_cols,
            &lines.scope,
            RgbColor::new_8bpc(255, 204, 102),
            false,
        )?;

        let mut signal_y = top + 58.0 + (cell_height * 1.35);
        let action_start_floor = bottom - 156.0;
        for (index, line) in lines
            .items
            .iter()
            .filter(|line| line.action_id.is_none())
            .take(7)
            .enumerate()
        {
            if signal_y + cell_height > action_start_floor {
                break;
            }
            let color = lcars_signal_text_color(line.kind, index);
            let bar = lcars.signal_fill(index);
            paint_lcars_signal_marker(
                self,
                layers,
                text_left,
                signal_y,
                line.kind,
                text_width,
                cell_height,
                bar,
            )?;
            if line.kind == PanelLineKind::Progress {
                paint_progress_rail(
                    self,
                    layers,
                    text_left + text_width - 104.0,
                    signal_y + 5.0,
                    92.0,
                    cell_height * 0.45,
                    line.progress,
                    lcars.dim_blue,
                    lcars.amber,
                )?;
            }
            self.paint_owt_panel_text(
                layers,
                text_left + 18.0,
                signal_y,
                text_cols.saturating_sub(2),
                &line.text,
                color,
                false,
            )?;
            signal_y += cell_height * 1.16;
        }

        let mut action_y = signal_y.max(action_start_floor);
        let button_height = (cell_height * 1.55).max(24.0);
        let button_width = text_width;
        let mut action_count = 0usize;
        for (index, line) in lines
            .items
            .iter()
            .filter(|line| line.action_id.is_some())
            .take(4)
            .enumerate()
        {
            if action_y + button_height > bottom - 34.0 {
                break;
            }
            let active = last_action_id.as_deref() == line.action_id.as_deref();
            paint_lcars_action_button(
                self,
                layers,
                text_left,
                action_y,
                button_width,
                button_height,
                text_cols.saturating_sub(1),
                &line.text,
                lcars_action_text_color(index),
                lcars.action_fill(index),
                lcars_action_fill_byte(index),
                active,
                lcars,
            )?;
            if let Some(action_id) = &line.action_id {
                self.ui_items.push(UIItem {
                    x: text_left.max(0.0) as usize,
                    y: action_y.max(0.0) as usize,
                    width: button_width.ceil().max(1.0) as usize,
                    height: button_height.ceil().max(1.0) as usize,
                    item_type: UIItemType::OwtLcarsAction(action_id.clone()),
                });
            }
            action_count += 1;
            action_y += button_height + 8.0;
        }

        let footer_text = if let Some(action_id) = last_action_id.as_deref() {
            format!("RIGHT RAIL / ACK {action_id}")
        } else if action_count > 0 {
            "RIGHT RAIL / HITBOX + KEY DISPATCH".to_string()
        } else {
            "RIGHT RAIL / DISPLAY ONLY".to_string()
        };
        self.paint_owt_panel_text(
            layers,
            text_left,
            bottom - 22.0,
            text_cols,
            &footer_text,
            RgbColor::new_8bpc(153, 204, 255),
            false,
        )?;

        Ok(())
    }

    fn paint_owt_lcars_bottom_panel(
        &mut self,
        layers: &mut TripleLayerQuadAllocator,
        document: &InterfaceDocument,
    ) -> anyhow::Result<()> {
        let cell_width = self.render_metrics.cell_size.width as f32;
        let cell_height = self.render_metrics.cell_size.height as f32;
        let border = self.get_os_border();
        let bottom_bar_height = if self.show_tab_bar && self.config.tab_bar_at_bottom {
            self.tab_bar_pixel_height()
                .context("compute bottom tab bar height for OWT LCARS bottom panel")?
        } else {
            0.0
        };

        let margin = LCARS_PANEL_MARGIN;
        let available_height = self.dimensions.pixel_height as f32 - (margin * 2.0);
        let Some(panel_height) = self.owt_lcars_bottom_panel_height(available_height) else {
            return Ok(());
        };
        let left = margin;
        let width = (self.dimensions.pixel_width as f32 - (margin * 2.0)).max(360.0);
        let bottom = self.dimensions.pixel_height as f32
            - border.bottom.get() as f32
            - bottom_bar_height
            - margin;
        let top = bottom - panel_height;

        let lcars = LcarsPalette::new();
        let panel_bg = lcars.black;
        let last_action_id =
            crate::owt_native::last_dispatched_action_snapshot().map(|action| action.action_id);

        self.filled_rectangle(layers, 0, rect(left, top, width, panel_height), panel_bg)?;
        self.filled_rectangle(
            layers,
            0,
            rect(left + 8.0, top + 8.0, 44.0, panel_height - 16.0),
            lcars.orange,
        )?;
        self.filled_rectangle(
            layers,
            0,
            rect(left + 8.0, top + 8.0, 44.0, 28.0),
            lcars.peach,
        )?;
        self.filled_rectangle(
            layers,
            0,
            rect(left + 8.0, bottom - 38.0, 44.0, 30.0),
            lcars.violet,
        )?;

        let content_left = left + 66.0;
        let action_width = (width * 0.24).clamp(240.0, 360.0);
        let action_left = (left + width - action_width - 12.0).max(content_left + 280.0);
        let signal_width = (action_left - content_left - 18.0).max(180.0);
        let signal_cols = ((signal_width - 20.0) / cell_width).max(8.0) as usize;
        let action_cols = ((action_width - 20.0) / cell_width).max(8.0) as usize;
        let lines = panel_lines(document, signal_cols.max(action_cols));

        self.filled_rectangle(
            layers,
            0,
            rect(content_left, top + 8.0, signal_width, 30.0),
            lcars.orange,
        )?;
        self.filled_rectangle(
            layers,
            0,
            rect(content_left, top + 46.0, signal_width, 3.0),
            lcars.amber,
        )?;
        self.filled_rectangle(
            layers,
            0,
            rect(action_left, top + 8.0, action_width, 30.0),
            lcars.dim_blue,
        )?;
        self.filled_rectangle(
            layers,
            0,
            rect(action_left, top + 46.0, action_width, 3.0),
            lcars.cyan,
        )?;

        self.paint_owt_panel_text(
            layers,
            content_left + 12.0,
            top + 13.0,
            signal_cols,
            &lines.title,
            RgbColor::new_8bpc(0, 0, 0),
            true,
        )?;
        self.paint_owt_panel_text(
            layers,
            content_left + 12.0,
            top + 56.0,
            signal_cols,
            &lines.scope,
            RgbColor::new_8bpc(255, 204, 102),
            false,
        )?;
        self.paint_owt_panel_text(
            layers,
            action_left + 10.0,
            top + 13.0,
            action_cols,
            "LCARS ACTIONS",
            RgbColor::new_8bpc(0, 0, 0),
            true,
        )?;

        let mut y = top + 56.0 + (cell_height * 1.15);
        for (index, line) in lines
            .items
            .iter()
            .filter(|line| line.action_id.is_none())
            .take(3)
            .enumerate()
        {
            if y + cell_height > bottom - 12.0 {
                break;
            }
            let color = lcars_signal_text_color(line.kind, index);
            let bar = lcars.signal_fill(index);
            paint_lcars_signal_marker(
                self,
                layers,
                content_left + 12.0,
                y,
                line.kind,
                signal_width,
                cell_height,
                bar,
            )?;
            if line.kind == PanelLineKind::Progress {
                paint_progress_rail(
                    self,
                    layers,
                    content_left + signal_width - 116.0,
                    y + 5.0,
                    104.0,
                    cell_height * 0.42,
                    line.progress,
                    lcars.dim_blue,
                    lcars.amber,
                )?;
            }
            self.paint_owt_panel_text(
                layers,
                content_left + 28.0,
                y,
                signal_cols.saturating_sub(2),
                &line.text,
                color,
                false,
            )?;
            y += cell_height * 1.12;
        }

        let button_height = (cell_height * 1.55).max(24.0);
        let mut action_y = top + 56.0;
        for (index, line) in lines
            .items
            .iter()
            .filter(|line| line.action_id.is_some())
            .take(3)
            .enumerate()
        {
            if action_y + button_height > bottom - 10.0 {
                break;
            }
            let active = last_action_id.as_deref() == line.action_id.as_deref();
            paint_lcars_action_button(
                self,
                layers,
                action_left,
                action_y,
                action_width,
                button_height,
                action_cols,
                &line.text,
                lcars_action_text_color(index),
                lcars.action_fill(index),
                lcars_action_fill_byte(index),
                active,
                lcars,
            )?;
            if let Some(action_id) = &line.action_id {
                self.ui_items.push(UIItem {
                    x: action_left.max(0.0) as usize,
                    y: action_y.max(0.0) as usize,
                    width: action_width.ceil().max(1.0) as usize,
                    height: button_height.ceil().max(1.0) as usize,
                    item_type: UIItemType::OwtLcarsAction(action_id.clone()),
                });
            }
            action_y += button_height + 8.0;
        }

        Ok(())
    }

    fn owt_lcars_panel_height_for(&self, available_height: f32, structural: bool) -> Option<f32> {
        let cell_height = self.render_metrics.cell_size.height as f32;
        if available_height < cell_height * 7.0 {
            return None;
        }
        if structural {
            return Some(
                (cell_height * LCARS_STRUCTURAL_PANEL_ROW_HEIGHT)
                    .max(LCARS_STRUCTURAL_PANEL_MIN_HEIGHT)
                    .min(available_height)
                    .min(LCARS_STRUCTURAL_PANEL_MAX_HEIGHT),
            );
        }
        Some(
            (cell_height * LCARS_PANEL_ROW_HEIGHT)
                .max(LCARS_PANEL_MIN_HEIGHT)
                .min(available_height)
                .min(LCARS_PANEL_MAX_HEIGHT),
        )
    }

    fn owt_lcars_side_panel_width(&self, available_width: f32) -> Option<f32> {
        let cell_width = self.render_metrics.cell_size.width as f32;
        if available_width < LCARS_MIN_TERMINAL_REMAINDER + (cell_width * 12.0) {
            return None;
        }
        let max_without_starving_terminal =
            (available_width - LCARS_MIN_TERMINAL_REMAINDER).max(cell_width * 12.0);
        Some(
            (available_width * 0.24)
                .max(LCARS_SIDE_PANEL_MIN_WIDTH)
                .min(LCARS_SIDE_PANEL_MAX_WIDTH)
                .min(max_without_starving_terminal),
        )
    }

    fn owt_lcars_bottom_panel_height(&self, available_height: f32) -> Option<f32> {
        let cell_height = self.render_metrics.cell_size.height as f32;
        if available_height < cell_height * 10.0 {
            return None;
        }
        Some(
            (cell_height * LCARS_BOTTOM_PANEL_ROW_HEIGHT)
                .max(LCARS_BOTTOM_PANEL_MIN_HEIGHT)
                .min(available_height)
                .min(LCARS_BOTTOM_PANEL_MAX_HEIGHT),
        )
    }

    fn paint_owt_panel_text(
        &mut self,
        layers: &mut TripleLayerQuadAllocator,
        x: f32,
        y: f32,
        max_cols: usize,
        text: &str,
        fg: RgbColor,
        bold: bool,
    ) -> anyhow::Result<()> {
        if max_cols == 0 {
            return Ok(());
        }

        let mut attrs = CellAttributes::blank();
        attrs.set_foreground(ColorSpec::TrueColor(fg.to_tuple_rgba()));
        if bold {
            attrs.set_intensity(Intensity::Bold);
        }
        let line = Line::from_text(&fit_text(text, max_cols), &attrs, 0, None);
        let palette = self.palette().clone();
        let gl_state = self.render_state.as_ref().unwrap();
        let white_space = gl_state.util_sprites.white_space.texture_coords();
        let filled_box = gl_state.util_sprites.filled_box.texture_coords();
        let dims = RenderableDimensions {
            cols: max_cols,
            physical_top: 0,
            scrollback_rows: 0,
            scrollback_top: 0,
            viewport_rows: 1,
            dpi: self.terminal_size.dpi,
            pixel_height: self.render_metrics.cell_size.height as usize,
            pixel_width: (max_cols as f32 * self.render_metrics.cell_size.width as f32) as usize,
            reverse_video: false,
        };
        let cursor = StableCursorPosition::default();

        self.render_screen_line(
            RenderScreenLineParams {
                top_pixel_y: y,
                left_pixel_x: x,
                pixel_width: max_cols as f32 * self.render_metrics.cell_size.width as f32,
                stable_line_idx: None,
                line: &line,
                selection: 0..0,
                cursor: &cursor,
                palette: &palette,
                dims: &dims,
                config: &self.config,
                pane: None,
                white_space,
                filled_box,
                cursor_border_color: LinearRgba::TRANSPARENT,
                foreground: fg.to_linear_tuple_rgba(),
                is_active: true,
                selection_fg: LinearRgba::TRANSPARENT,
                selection_bg: LinearRgba::TRANSPARENT,
                cursor_fg: LinearRgba::TRANSPARENT,
                cursor_bg: LinearRgba::TRANSPARENT,
                cursor_is_default_color: true,
                window_is_transparent: false,
                default_bg: LinearRgba::TRANSPARENT,
                font: None,
                style: None,
                use_pixel_positioning: self.config.experimental_pixel_positioning,
                render_metrics: self.render_metrics,
                shape_key: None,
                password_input: false,
            },
            layers,
        )?;

        Ok(())
    }
}

struct PanelLines {
    title: String,
    scope: String,
    items: Vec<PanelLine>,
    has_actions: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PanelLineKind {
    Text,
    Status,
    Frame,
    Section,
    Metric,
    Badge,
    Progress,
    Bar,
    BarRun,
    Elbow,
    SideRail,
    ContentBay,
    CommandGrid,
    DataCascade,
    List,
    Table,
    Image,
    Button,
}

struct PanelLine {
    text: String,
    action_id: Option<String>,
    kind: PanelLineKind,
    hotkey: Option<usize>,
    progress: Option<f32>,
}

impl PanelLine {
    fn semantic(kind: PanelLineKind, text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            action_id: None,
            kind,
            hotkey: None,
            progress: None,
        }
    }

    fn action(text: impl Into<String>, action_id: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            action_id: Some(action_id.into()),
            kind: PanelLineKind::Button,
            hotkey: None,
            progress: None,
        }
    }

    fn with_progress(mut self, progress: Option<f32>) -> Self {
        self.progress = progress;
        self
    }
}

struct LcarsTableData {
    title: String,
    columns: Vec<String>,
    rows: Vec<LcarsTableRow>,
    overflow_columns: usize,
    overflow_rows: usize,
}

struct LcarsTableRow {
    cells: Vec<String>,
    severity: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum LcarsDetailPlacement {
    None,
    Inline,
    Stacked,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum LcarsStructuralBreakpoint {
    Compact,
    Regular,
    Wide,
}

#[derive(Clone, Copy, Debug)]
struct LcarsStructuralPlan {
    command_left: f32,
    command_width: f32,
    command_visible: bool,
    signal_width: f32,
    signal_rows: usize,
    detail: LcarsDetailPlacement,
    detail_left: f32,
    detail_top: f32,
    detail_width: f32,
    detail_height: f32,
    action_slots: usize,
    two_action_columns: bool,
}

impl LcarsStructuralPlan {
    fn table_max_columns(self, cell_width: f32) -> usize {
        let cols = (self.detail_width / cell_width).floor() as usize;
        if self.detail == LcarsDetailPlacement::None {
            3
        } else if cols >= 68 {
            6
        } else if cols >= 52 {
            5
        } else if cols >= 38 {
            4
        } else {
            3
        }
    }

    fn table_max_rows(self, cell_height: f32) -> usize {
        if self.detail == LcarsDetailPlacement::None {
            return 3;
        }
        let title_and_header = cell_height * 3.4;
        let row_height = (cell_height * 1.08).max(14.0);
        ((self.detail_height - title_and_header) / row_height)
            .floor()
            .max(2.0) as usize
    }
}

fn lcars_structural_breakpoint(content_width: f32, cell_width: f32) -> LcarsStructuralBreakpoint {
    let cell_columns = content_width / cell_width.max(1.0);
    if content_width < 840.0 || cell_columns < 88.0 {
        LcarsStructuralBreakpoint::Compact
    } else if content_width < 1280.0 || cell_columns < 132.0 {
        LcarsStructuralBreakpoint::Regular
    } else {
        LcarsStructuralBreakpoint::Wide
    }
}

fn structural_signal_visual_limit(
    plan: &LcarsStructuralPlan,
    signal_limit: f32,
    cell_height: f32,
) -> f32 {
    if matches!(plan.detail, LcarsDetailPlacement::Stacked) {
        (plan.detail_top - LCARS_PANEL_GAP.max(cell_height * 0.35)).min(signal_limit)
    } else {
        signal_limit
    }
}

fn structural_signal_text_cols(
    kind: PanelLineKind,
    signal_width: f32,
    cell_width: f32,
    signal_cols: usize,
) -> usize {
    let chrome_width = match kind {
        PanelLineKind::Progress => 132.0,
        PanelLineKind::Bar
        | PanelLineKind::BarRun
        | PanelLineKind::Frame
        | PanelLineKind::ContentBay
        | PanelLineKind::CommandGrid
        | PanelLineKind::DataCascade
        | PanelLineKind::Section => (cell_width * 4.0).min(signal_width * 0.12),
        _ => 0.0,
    };
    let chrome_cols = (chrome_width / cell_width.max(1.0)).ceil().max(0.0) as usize;
    let max_cols = signal_cols.saturating_sub(2).max(1);
    signal_cols
        .saturating_sub(chrome_cols)
        .saturating_sub(2)
        .max(8)
        .min(max_cols)
}

fn structural_signal_text_needs_backing(kind: PanelLineKind) -> bool {
    matches!(
        kind,
        PanelLineKind::Bar
            | PanelLineKind::BarRun
            | PanelLineKind::Frame
            | PanelLineKind::ContentBay
            | PanelLineKind::CommandGrid
            | PanelLineKind::DataCascade
            | PanelLineKind::Section
            | PanelLineKind::Progress
    )
}

fn plan_lcars_structural_console(
    content_left: f32,
    content_right: f32,
    signal_top: f32,
    signal_limit: f32,
    cell_width: f32,
    cell_height: f32,
    has_table: bool,
    has_data_cascade: bool,
    action_count: usize,
) -> LcarsStructuralPlan {
    let content_width = (content_right - content_left).max(cell_width * 34.0);
    let breakpoint = lcars_structural_breakpoint(content_width, cell_width);
    let command_visible = action_count > 0;
    let command_width = if command_visible {
        let (min_cells, min_px, max_px, width_ratio, max_ratio): (f32, f32, f32, f32, f32) =
            match breakpoint {
                LcarsStructuralBreakpoint::Compact => (20.0, 210.0, 270.0, 0.30, 0.34),
                LcarsStructuralBreakpoint::Regular => (24.0, 250.0, 390.0, 0.29, 0.36),
                LcarsStructuralBreakpoint::Wide => (34.0, 360.0, 620.0, 0.31, 0.38),
            };
        let min_command_width = (cell_width * min_cells).max(min_px);
        let command_ceiling = max_px
            .min(content_width * max_ratio)
            .max(min_command_width.min(content_width * max_ratio));
        let desired_command_width = clamp_ordered(
            content_width * width_ratio,
            min_command_width,
            command_ceiling,
        );
        desired_command_width
            .min((content_width - (cell_width * 18.0)).max(min_command_width.min(content_width)))
            .max(min_command_width.min(command_ceiling))
    } else {
        0.0
    };
    let command_left = content_right - command_width;
    let pre_command_width = if command_visible {
        (command_left - content_left - 18.0).max(cell_width * 16.0)
    } else {
        (content_right - content_left).max(cell_width * 16.0)
    };
    let wants_detail = has_table || has_data_cascade;
    let min_signal_width = match breakpoint {
        LcarsStructuralBreakpoint::Compact => cell_width * 18.0,
        LcarsStructuralBreakpoint::Regular => cell_width * 22.0,
        LcarsStructuralBreakpoint::Wide => cell_width * 26.0,
    };
    let min_detail_width = match (breakpoint, has_table) {
        (LcarsStructuralBreakpoint::Compact, true) => (cell_width * 32.0).max(280.0),
        (LcarsStructuralBreakpoint::Compact, false) => (cell_width * 22.0).max(210.0),
        (_, true) => (cell_width * 38.0).max(320.0),
        (_, false) => (cell_width * 28.0).max(240.0),
    };
    let vertical_span = (signal_limit - signal_top).max(cell_height * 4.0);
    let available_signal_rows = (vertical_span / (cell_height * 1.18)).floor().max(1.0) as usize;
    let inline_capacity = pre_command_width - min_signal_width - LCARS_PANEL_GAP;

    let (detail, detail_left, detail_top, detail_width, detail_height, signal_width, signal_rows) =
        if wants_detail && inline_capacity >= min_detail_width {
            let max_detail_px = match breakpoint {
                LcarsStructuralBreakpoint::Compact => 360.0,
                LcarsStructuralBreakpoint::Regular => 680.0,
                LcarsStructuralBreakpoint::Wide => 900.0,
            };
            let detail_ratio = match (breakpoint, has_table) {
                (LcarsStructuralBreakpoint::Compact, true) => 0.56,
                (LcarsStructuralBreakpoint::Compact, false) => 0.46,
                (LcarsStructuralBreakpoint::Regular, true) => 0.54,
                (LcarsStructuralBreakpoint::Regular, false) => 0.46,
                (LcarsStructuralBreakpoint::Wide, true) => 0.60,
                (LcarsStructuralBreakpoint::Wide, false) => 0.54,
            };
            let max_detail = (inline_capacity * 0.92).min(max_detail_px);
            let detail_width = clamp_ordered(
                pre_command_width * detail_ratio,
                min_detail_width,
                max_detail,
            );
            let detail_left = command_left - detail_width - LCARS_PANEL_GAP;
            let signal_width = (detail_left - content_left - 14.0).max(min_signal_width);
            let max_signal_rows = match breakpoint {
                LcarsStructuralBreakpoint::Compact => 4,
                LcarsStructuralBreakpoint::Regular => 5,
                LcarsStructuralBreakpoint::Wide => 6,
            };
            (
                LcarsDetailPlacement::Inline,
                detail_left,
                signal_top,
                detail_width,
                vertical_span,
                signal_width,
                available_signal_rows.min(max_signal_rows),
            )
        } else if wants_detail && vertical_span >= cell_height * 7.0 {
            let signal_rows = available_signal_rows.min(match breakpoint {
                LcarsStructuralBreakpoint::Compact => 3,
                LcarsStructuralBreakpoint::Regular | LcarsStructuralBreakpoint::Wide => 2,
            });
            let detail_top =
                signal_top + (signal_rows as f32 * cell_height * 1.18) + LCARS_PANEL_GAP;
            let detail_height = (signal_limit - detail_top).max(cell_height * 4.0);
            (
                LcarsDetailPlacement::Stacked,
                content_left,
                detail_top,
                pre_command_width,
                detail_height,
                pre_command_width,
                signal_rows,
            )
        } else {
            let max_signal_rows = match breakpoint {
                LcarsStructuralBreakpoint::Compact => 4,
                LcarsStructuralBreakpoint::Regular => 5,
                LcarsStructuralBreakpoint::Wide => 6,
            };
            (
                LcarsDetailPlacement::None,
                content_left,
                signal_top,
                0.0,
                0.0,
                pre_command_width,
                available_signal_rows.min(max_signal_rows),
            )
        };

    let button_height = (cell_height * 2.15).clamp(36.0, 46.0);
    let button_gap = 12.0;
    let action_rows = ((signal_limit - (signal_top + 2.0)) / (button_height + button_gap))
        .floor()
        .max(0.0) as usize;
    let two_action_columns = command_visible
        && !matches!(breakpoint, LcarsStructuralBreakpoint::Compact)
        && action_count > 2
        && command_width >= cell_width * 42.0;
    let action_slots = if command_visible {
        action_rows
            .saturating_mul(if two_action_columns { 2 } else { 1 })
            .min(LCARS_KEY_ACTION_LIMIT)
            .max(1)
    } else {
        0
    };

    LcarsStructuralPlan {
        command_left,
        command_width,
        command_visible,
        signal_width,
        signal_rows,
        detail,
        detail_left,
        detail_top,
        detail_width,
        detail_height,
        action_slots,
        two_action_columns,
    }
}

fn panel_lines(document: &InterfaceDocument, max_cols: usize) -> PanelLines {
    let mut items = collect_panel_items(document);
    if let Some(action) = crate::owt_native::last_dispatched_action_snapshot() {
        items.insert(
            0,
            PanelLine::semantic(PanelLineKind::Status, format!("ACK {}", action.action_id)),
        );
    }
    if items.is_empty() {
        items.push(PanelLine::semantic(
            PanelLineKind::Status,
            "INTERFACE STATE APPLIED",
        ));
    }
    assign_lcars_hotkeys(&mut items);
    let has_actions = items.iter().any(|line| line.action_id.is_some());

    PanelLines {
        title: fit_text(&document.title.to_uppercase(), max_cols),
        scope: fit_text(
            &format!(
                "SCOPE {:?} | {}",
                document.scope.kind,
                compact_scope_id(&document.scope.id)
            ),
            max_cols,
        ),
        items,
        has_actions,
    }
}

fn collect_panel_items(document: &InterfaceDocument) -> Vec<PanelLine> {
    let mut items = Vec::new();
    for node in &document.nodes {
        collect_node_lines(node, &mut items);
    }
    for action in &document.actions {
        if !items
            .iter()
            .any(|line| line.action_id.as_deref() == Some(action.id.as_str()))
        {
            items.push(PanelLine::action(action.label.clone(), action.id.clone()));
        }
    }
    items
}

fn assign_lcars_hotkeys(items: &mut [PanelLine]) {
    let mut slot = 1usize;
    for line in items.iter_mut().filter(|line| line.action_id.is_some()) {
        if slot > LCARS_KEY_ACTION_LIMIT {
            break;
        }
        line.hotkey = Some(slot);
        line.text = format!("[{slot}] {}", line.text);
        slot += 1;
    }
}

fn lcars_keyboard_action_slots(document: &InterfaceDocument) -> Vec<String> {
    collect_panel_items(document)
        .into_iter()
        .filter_map(|line| line.action_id)
        .take(LCARS_KEY_ACTION_LIMIT)
        .collect()
}

fn lcars_panel_layout(document: &InterfaceDocument) -> LcarsPanelLayout {
    for node in &document.nodes {
        if let Some(layout) = node_property_value(node, "layout")
            .or_else(|| node_property_value(node, "mode"))
            .or_else(|| node_property_value(node, "profile"))
            .and_then(parse_lcars_panel_layout)
        {
            return layout;
        }
    }
    LcarsPanelLayout::Top
}

fn has_structural_lcars_layout(document: &InterfaceDocument) -> bool {
    for node in &document.nodes {
        if let Some(profile) = node_property_value(node, "profile")
            .or_else(|| node_property_value(node, "layout_profile"))
            .or_else(|| node_property_value(node, "information_shape"))
            .or_else(|| node_property_value(node, "data_shape"))
            .or_else(|| node_property_value(node, "shape"))
        {
            let normalized = profile.trim().to_ascii_lowercase().replace(['-', ' '], "_");
            if matches!(
                normalized.as_str(),
                "structural"
                    | "structural_console"
                    | "console_frame"
                    | "frame"
                    | "framed"
                    | "content_bay"
                    | "table"
                    | "table_bay"
                    | "matrix"
                    | "operational_matrix"
                    | "fleet_matrix"
                    | "incident_summary"
                    | "queue_triage"
                    | "project_status"
                    | "artifact_browser"
                    | "lcars_v24"
            ) {
                return true;
            }
        }
    }

    document_has_kind(document, |kind| {
        matches!(
            kind,
            UiNodeKind::Frame
                | UiNodeKind::SideRail
                | UiNodeKind::ContentBay
                | UiNodeKind::BarRun
                | UiNodeKind::CommandGrid
                | UiNodeKind::DataCascade
                | UiNodeKind::Table
        )
    })
}

fn parse_lcars_panel_layout(value: &str) -> Option<LcarsPanelLayout> {
    let normalized = value.trim().to_ascii_lowercase().replace(['-', ' '], "_");
    match normalized.as_str() {
        "docked" | "docked_top" | "top" | "top_band" | "compact_top" | "full_dashboard" => {
            Some(LcarsPanelLayout::Top)
        }
        "right" | "right_rail" | "rail_right" | "side" | "side_rail" | "docked_right"
        | "right_browser" | "browser_right" => Some(LcarsPanelLayout::Right),
        "bottom" | "bottom_strip" | "status_strip" | "docked_bottom" | "alert_strip" => {
            Some(LcarsPanelLayout::Bottom)
        }
        "overlay" | "hud" | "heads_up" | "heads_up_display" => Some(LcarsPanelLayout::Overlay),
        _ => None,
    }
}

fn lcars_shortcut_index(key: &KeyCode, physical_key: Option<&PhysKeyCode>) -> Option<usize> {
    match physical_key {
        Some(PhysKeyCode::K1) => return Some(0),
        Some(PhysKeyCode::K2) => return Some(1),
        Some(PhysKeyCode::K3) => return Some(2),
        Some(PhysKeyCode::K4) => return Some(3),
        Some(PhysKeyCode::K5) => return Some(4),
        Some(PhysKeyCode::K6) => return Some(5),
        Some(PhysKeyCode::K7) => return Some(6),
        Some(PhysKeyCode::K8) => return Some(7),
        Some(PhysKeyCode::K9) => return Some(8),
        Some(PhysKeyCode::Keypad1) => return Some(0),
        Some(PhysKeyCode::Keypad2) => return Some(1),
        Some(PhysKeyCode::Keypad3) => return Some(2),
        Some(PhysKeyCode::Keypad4) => return Some(3),
        Some(PhysKeyCode::Keypad5) => return Some(4),
        Some(PhysKeyCode::Keypad6) => return Some(5),
        Some(PhysKeyCode::Keypad7) => return Some(6),
        Some(PhysKeyCode::Keypad8) => return Some(7),
        Some(PhysKeyCode::Keypad9) => return Some(8),
        _ => {}
    }

    match key {
        KeyCode::Char('1' | '!') | KeyCode::Numpad(1) => Some(0),
        KeyCode::Char('2' | '@' | '"') | KeyCode::Numpad(2) => Some(1),
        KeyCode::Char('3' | '#') | KeyCode::Numpad(3) => Some(2),
        KeyCode::Char('4' | '$') | KeyCode::Numpad(4) => Some(3),
        KeyCode::Char('5' | '%') | KeyCode::Numpad(5) => Some(4),
        KeyCode::Char('6' | '^') | KeyCode::Numpad(6) => Some(5),
        KeyCode::Char('7' | '&' | '/') | KeyCode::Numpad(7) => Some(6),
        KeyCode::Char('8' | '*') | KeyCode::Numpad(8) => Some(7),
        KeyCode::Char('9' | '(' | ')') | KeyCode::Numpad(9) => Some(8),
        _ => None,
    }
}

fn node_property_value<'a>(node: &'a UiNode, key: &str) -> Option<&'a str> {
    if let Some(value) = node.properties.get(key).map(String::as_str) {
        return Some(value);
    }
    for child in &node.children {
        if let Some(value) = node_property_value(child, key) {
            return Some(value);
        }
    }
    None
}

fn document_has_kind<F>(document: &InterfaceDocument, mut predicate: F) -> bool
where
    F: FnMut(&UiNodeKind) -> bool,
{
    document
        .nodes
        .iter()
        .any(|node| node_has_kind(node, &mut predicate))
}

fn node_has_kind<F>(node: &UiNode, predicate: &mut F) -> bool
where
    F: FnMut(&UiNodeKind) -> bool,
{
    predicate(&node.kind)
        || node
            .children
            .iter()
            .any(|child| node_has_kind(child, predicate))
}

fn collect_node_lines(node: &UiNode, items: &mut Vec<PanelLine>) {
    if items.len() >= LCARS_MAX_PANEL_LINES {
        return;
    }

    if let Some(line) = node_line(node) {
        items.push(line);
    }

    for child in &node.children {
        collect_node_lines(child, items);
        if items.len() >= LCARS_MAX_PANEL_LINES {
            break;
        }
    }
}

fn node_line(node: &UiNode) -> Option<PanelLine> {
    match &node.kind {
        UiNodeKind::Panel | UiNodeKind::Spacer => None,
        UiNodeKind::Frame => node_summary_text(node, "FRAME")
            .map(|text| PanelLine::semantic(PanelLineKind::Frame, text.to_uppercase())),
        UiNodeKind::SideRail => node_summary_text(node, "SIDE RAIL")
            .map(|text| PanelLine::semantic(PanelLineKind::SideRail, text.to_uppercase())),
        UiNodeKind::ContentBay => node_summary_text(node, "CONTENT BAY")
            .map(|text| PanelLine::semantic(PanelLineKind::ContentBay, text.to_uppercase())),
        UiNodeKind::Region | UiNodeKind::Group => node_summary_text(node, "SECTION")
            .map(|text| PanelLine::semantic(PanelLineKind::Section, text.to_uppercase())),
        UiNodeKind::Metric => Some(PanelLine::semantic(
            PanelLineKind::Metric,
            format!(
                "{}: {}",
                node.label.as_deref().unwrap_or("METRIC"),
                node_value_text(node, &["value", "state", "status"], "ONLINE")
            ),
        )),
        UiNodeKind::Button => {
            let text = node.label.as_deref().unwrap_or("ACTION").to_string();
            match &node.action_id {
                Some(action_id) => Some(PanelLine::action(text, action_id.clone())),
                None => Some(PanelLine::semantic(PanelLineKind::Button, text)),
            }
        }
        UiNodeKind::Badge => Some(PanelLine::semantic(
            PanelLineKind::Badge,
            compact_label_value(
                node.label.as_deref().unwrap_or("BADGE"),
                &node_value_text(node, &["value", "state", "status"], ""),
            ),
        )),
        UiNodeKind::Progress => {
            let progress = node_progress_value(node);
            let value = progress
                .map(|value| format!("{:.0}%", value * 100.0))
                .unwrap_or_else(|| node_value_text(node, &["value", "progress", "percent"], ""));
            Some(
                PanelLine::semantic(
                    PanelLineKind::Progress,
                    compact_label_value(node.label.as_deref().unwrap_or("PROGRESS"), &value),
                )
                .with_progress(progress),
            )
        }
        UiNodeKind::List => node_summary_text(node, "LIST")
            .map(|text| PanelLine::semantic(PanelLineKind::List, text)),
        UiNodeKind::Table => node_summary_text(node, "TABLE")
            .map(|text| PanelLine::semantic(PanelLineKind::Table, text)),
        UiNodeKind::Text => node_summary_text(node, "TEXT")
            .map(|text| PanelLine::semantic(PanelLineKind::Text, text)),
        UiNodeKind::Bar => {
            node_summary_text(node, "BAR").map(|text| PanelLine::semantic(PanelLineKind::Bar, text))
        }
        UiNodeKind::BarRun => node_summary_text(node, "BAR RUN")
            .map(|text| PanelLine::semantic(PanelLineKind::BarRun, text.to_uppercase())),
        UiNodeKind::Elbow => node_summary_text(node, "ELBOW")
            .map(|text| PanelLine::semantic(PanelLineKind::Elbow, text)),
        UiNodeKind::CommandGrid => node_summary_text(node, "COMMAND GRID")
            .map(|text| PanelLine::semantic(PanelLineKind::CommandGrid, text.to_uppercase())),
        UiNodeKind::DataCascade => node_summary_text(node, "DATA CASCADE")
            .map(|text| PanelLine::semantic(PanelLineKind::DataCascade, text.to_uppercase())),
        UiNodeKind::Image => node_summary_text(node, "IMAGE")
            .map(|text| PanelLine::semantic(PanelLineKind::Image, text)),
    }
}

fn node_summary_text(node: &UiNode, fallback: &str) -> Option<String> {
    node.text
        .clone()
        .or_else(|| node.label.clone())
        .or_else(|| node.properties.get("title").cloned())
        .or_else(|| node.properties.get("value").cloned())
        .filter(|value| !value.trim().is_empty())
        .or_else(|| Some(fallback.to_string()))
}

fn node_value_text(node: &UiNode, keys: &[&str], fallback: &str) -> String {
    if let Some(text) = node
        .text
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    {
        return text.to_string();
    }
    for key in keys {
        if let Some(value) = node
            .properties
            .get(*key)
            .filter(|value| !value.trim().is_empty())
        {
            return value.to_string();
        }
    }
    fallback.to_string()
}

fn compact_label_value(label: &str, value: &str) -> String {
    if value.trim().is_empty() {
        label.to_string()
    } else {
        format!("{label} {value}")
    }
}

fn node_progress_value(node: &UiNode) -> Option<f32> {
    for key in ["value", "progress", "percent", "ratio"] {
        if let Some(value) = node.properties.get(key).or(node.text.as_ref()) {
            if let Some(parsed) = parse_progress_value(value) {
                return Some(parsed);
            }
        }
    }
    None
}

fn lcars_table_data(
    document: &InterfaceDocument,
    max_columns: usize,
    max_rows: usize,
) -> Option<LcarsTableData> {
    let node = first_node_matching(document, |node| node.kind == UiNodeKind::Table)?;
    let (columns, overflow_columns) = parse_table_columns(node, max_columns);
    let mut rows = parse_table_rows(node, columns.len().max(1));
    let overflow_rows = rows.len().saturating_sub(max_rows);
    rows.truncate(max_rows);
    if rows.is_empty() && columns.is_empty() {
        return None;
    }

    Some(LcarsTableData {
        title: node
            .label
            .clone()
            .or_else(|| node.properties.get("title").cloned())
            .unwrap_or_else(|| "TABLE BAY".to_string()),
        columns,
        rows,
        overflow_columns,
        overflow_rows,
    })
}

fn parse_table_columns(node: &UiNode, max_columns: usize) -> (Vec<String>, usize) {
    let source = node
        .properties
        .get("columns")
        .or_else(|| node.properties.get("headers"));
    let source_columns = source
        .map(|value| split_table_cells(value))
        .unwrap_or_default()
        .into_iter()
        .filter(|cell| !cell.trim().is_empty())
        .collect::<Vec<_>>();
    let max_columns = max_columns.max(1);
    let overflow_columns = source_columns.len().saturating_sub(max_columns);
    let mut columns = source_columns
        .into_iter()
        .take(max_columns)
        .collect::<Vec<_>>();
    if columns.is_empty() {
        columns = vec!["ITEM".to_string(), "VALUE".to_string(), "STATE".to_string()];
        return (columns, 0);
    }
    (columns, overflow_columns)
}

fn parse_table_rows(node: &UiNode, expected_columns: usize) -> Vec<LcarsTableRow> {
    let mut rows = Vec::new();
    if let Some(source) = node
        .properties
        .get("rows")
        .or(node.text.as_ref())
        .filter(|value| !value.trim().is_empty())
    {
        for line in split_table_rows(source) {
            let cells = normalize_table_cells(split_table_cells(&line), expected_columns);
            if !cells.iter().any(|cell| !cell.trim().is_empty()) {
                continue;
            }
            rows.push(LcarsTableRow {
                severity: table_row_severity(&cells),
                cells,
            });
        }
    }

    for child in &node.children {
        let cells = child
            .properties
            .get("cells")
            .map(|value| split_table_cells(value))
            .unwrap_or_else(|| {
                vec![
                    child.label.clone().unwrap_or_else(|| child.id.clone()),
                    node_value_text(child, &["value", "state", "status"], ""),
                ]
            });
        let cells = normalize_table_cells(cells, expected_columns);
        if !cells.iter().any(|cell| !cell.trim().is_empty()) {
            continue;
        }
        rows.push(LcarsTableRow {
            severity: child
                .properties
                .get("severity")
                .cloned()
                .or_else(|| table_row_severity(&cells)),
            cells,
        });
    }
    rows
}

fn split_table_rows(source: &str) -> Vec<String> {
    let normalized = source.replace("\r\n", "\n").replace('\r', "\n");
    if normalized.contains('\n') {
        normalized
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .map(str::to_string)
            .collect()
    } else {
        normalized
            .split(';')
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .map(str::to_string)
            .collect()
    }
}

fn split_table_cells(source: &str) -> Vec<String> {
    let delimiter = if source.contains('|') {
        '|'
    } else if source.contains('\t') {
        '\t'
    } else {
        ','
    };
    source
        .split(delimiter)
        .map(str::trim)
        .map(str::to_string)
        .collect()
}

fn normalize_table_cells(mut cells: Vec<String>, expected_columns: usize) -> Vec<String> {
    let expected_columns = expected_columns.max(1);
    cells.truncate(expected_columns);
    while cells.len() < expected_columns {
        cells.push(String::new());
    }
    cells
}

fn table_row_severity(cells: &[String]) -> Option<String> {
    let joined = cells.join(" ").to_ascii_lowercase();
    if joined.contains("error")
        || joined.contains("failed")
        || joined.contains("blocked")
        || joined.contains("unreachable")
    {
        Some("error".to_string())
    } else if joined.contains("warning")
        || joined.contains("stale")
        || joined.contains("legacy")
        || joined.contains("oldest")
        || joined.contains("watch")
    {
        Some("warning".to_string())
    } else if joined.contains("ok")
        || joined.contains("ready")
        || joined.contains("online")
        || joined.contains("current")
    {
        Some("success".to_string())
    } else {
        None
    }
}

fn first_node_matching<F>(document: &InterfaceDocument, mut predicate: F) -> Option<&UiNode>
where
    F: FnMut(&UiNode) -> bool,
{
    document
        .nodes
        .iter()
        .find_map(|node| first_node_matching_in(node, &mut predicate))
}

fn first_node_matching_in<'a, F>(node: &'a UiNode, predicate: &mut F) -> Option<&'a UiNode>
where
    F: FnMut(&UiNode) -> bool,
{
    if predicate(node) {
        return Some(node);
    }
    node.children
        .iter()
        .find_map(|child| first_node_matching_in(child, predicate))
}

fn parse_progress_value(value: &str) -> Option<f32> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }
    let percent = trimmed.ends_with('%');
    let number = trimmed.trim_end_matches('%').trim().parse::<f32>().ok()?;
    let normalized = if percent || number > 1.0 {
        number / 100.0
    } else {
        number
    };
    Some(normalized.clamp(0.0, 1.0))
}

fn lcars_signal_text_color(kind: PanelLineKind, index: usize) -> RgbColor {
    match kind {
        PanelLineKind::Status => RgbColor::new_8bpc(255, 102, 102),
        PanelLineKind::Frame => RgbColor::new_8bpc(255, 153, 102),
        PanelLineKind::Section => RgbColor::new_8bpc(255, 204, 102),
        PanelLineKind::Metric => RgbColor::new_8bpc(153, 204, 255),
        PanelLineKind::Badge => RgbColor::new_8bpc(255, 153, 102),
        PanelLineKind::Progress => RgbColor::new_8bpc(153, 204, 255),
        PanelLineKind::Bar | PanelLineKind::BarRun => RgbColor::new_8bpc(255, 204, 102),
        PanelLineKind::Elbow => RgbColor::new_8bpc(204, 153, 255),
        PanelLineKind::SideRail | PanelLineKind::ContentBay => RgbColor::new_8bpc(153, 204, 255),
        PanelLineKind::CommandGrid => RgbColor::new_8bpc(255, 153, 102),
        PanelLineKind::DataCascade => RgbColor::new_8bpc(204, 153, 255),
        PanelLineKind::List | PanelLineKind::Table => RgbColor::new_8bpc(153, 204, 255),
        PanelLineKind::Image => RgbColor::new_8bpc(102, 153, 204),
        PanelLineKind::Button | PanelLineKind::Text => match index {
            0 => RgbColor::new_8bpc(153, 204, 255),
            1 => RgbColor::new_8bpc(255, 204, 102),
            2 => RgbColor::new_8bpc(204, 153, 255),
            _ => RgbColor::new_8bpc(255, 153, 102),
        },
    }
}

fn lcars_action_text_color(index: usize) -> RgbColor {
    match index % 4 {
        0 => RgbColor::new_8bpc(255, 153, 102),
        1 => RgbColor::new_8bpc(204, 153, 255),
        2 => RgbColor::new_8bpc(153, 204, 255),
        _ => RgbColor::new_8bpc(255, 204, 102),
    }
}

fn lcars_action_fill_byte(index: usize) -> LcarsByteColor {
    match index % 4 {
        0 => LCARS_BYTE_PEACH,
        1 => LCARS_BYTE_VIOLET,
        2 => LCARS_BYTE_BLUE,
        _ => LCARS_BYTE_AMBER,
    }
}

fn lcars_action_accent_byte(fill_byte: LcarsByteColor, active: bool) -> LcarsByteColor {
    if active {
        fill_byte.with_alpha(214)
    } else {
        fill_byte
    }
}

fn lcars_signal_marker_width(kind: PanelLineKind, available_width: f32) -> f32 {
    match kind {
        PanelLineKind::Frame => available_width.clamp(72.0, 220.0),
        PanelLineKind::Section => available_width.clamp(42.0, 132.0),
        PanelLineKind::Bar | PanelLineKind::BarRun => available_width.clamp(64.0, 196.0),
        PanelLineKind::Elbow | PanelLineKind::SideRail => 34.0,
        PanelLineKind::ContentBay => available_width.clamp(96.0, 240.0),
        PanelLineKind::CommandGrid => available_width.clamp(56.0, 150.0),
        PanelLineKind::DataCascade => available_width.clamp(48.0, 138.0),
        PanelLineKind::Badge | PanelLineKind::Status => 14.0,
        _ => 10.0,
    }
}

fn paint_lcars_signal_marker(
    window: &mut crate::TermWindow,
    layers: &mut TripleLayerQuadAllocator,
    x: f32,
    y: f32,
    kind: PanelLineKind,
    available_width: f32,
    cell_height: f32,
    fill: LinearRgba,
) -> anyhow::Result<()> {
    match kind {
        PanelLineKind::Elbow | PanelLineKind::SideRail => {
            window.filled_rectangle(layers, 0, rect(x, y + 2.0, 10.0, cell_height * 0.86), fill)?;
            window.filled_rectangle(layers, 0, rect(x, y + 2.0, 34.0, 4.0), fill)?;
        }
        PanelLineKind::Bar
        | PanelLineKind::BarRun
        | PanelLineKind::Frame
        | PanelLineKind::ContentBay
        | PanelLineKind::CommandGrid
        | PanelLineKind::DataCascade
        | PanelLineKind::Section => {
            let rule_width = lcars_signal_marker_width(kind, available_width - 16.0);
            let rule_x = (x + available_width - rule_width).max(x + 20.0);
            window.filled_rectangle(layers, 0, rect(x, y + 3.0, 10.0, cell_height * 0.72), fill)?;
            window.filled_rectangle(
                layers,
                0,
                rect(rule_x, y + cell_height - 3.0, rule_width, 2.0),
                fill,
            )?;
        }
        _ => {
            window.filled_rectangle(layers, 0, rect(x, y + 3.0, 10.0, cell_height * 0.72), fill)?;
        }
    }
    Ok(())
}

fn paint_lcars_signal_label_bar(
    window: &mut crate::TermWindow,
    layers: &mut TripleLayerQuadAllocator,
    x: f32,
    y: f32,
    kind: PanelLineKind,
    available_width: f32,
    backing_width: f32,
    text_rows: usize,
    cell_height: f32,
    fill: LinearRgba,
    palette: LcarsPalette,
) -> anyhow::Result<()> {
    let rows = text_rows.max(1) as f32;
    let label_height = cell_height * rows;
    let tab_height = (label_height + 1.0).max(cell_height * 0.72);
    let text_x = x + 14.0;
    let backing_width = backing_width.min((available_width - 24.0).max(1.0));
    let text_right = (text_x + backing_width + 10.0).min(x + available_width);

    window.filled_rectangle(
        layers,
        0,
        rect(text_x, y - 1.0, backing_width + 8.0, label_height + 4.0),
        palette.black,
    )?;
    window.filled_rectangle(
        layers,
        0,
        rect(x, y + 3.0, 10.0, tab_height.min(label_height + 4.0)),
        fill,
    )?;

    if structural_signal_text_needs_backing(kind) {
        let header_width = (backing_width * 0.86).min(available_width * 0.36).max(24.0);
        window.filled_rectangle(layers, 0, rect(text_x, y - 5.0, header_width, 3.0), fill)?;
        let right_rail_x = (text_right + 10.0)
            .max(x + available_width * 0.58)
            .min(x + available_width - 24.0);
        let right_rail_width = (x + available_width - right_rail_x - 8.0).max(0.0);
        if right_rail_width >= 18.0 {
            window.filled_rectangle(
                layers,
                0,
                rect(right_rail_x, y + label_height - 3.0, right_rail_width, 2.0),
                fill,
            )?;
        }
    } else if matches!(kind, PanelLineKind::Status | PanelLineKind::Badge) {
        window.filled_rectangle(
            layers,
            0,
            rect(
                text_x + backing_width + 4.0,
                y + 4.0,
                12.0,
                cell_height * 0.55,
            ),
            fill,
        )?;
    }

    Ok(())
}

fn paint_lcars_side_rail_chrome(
    window: &mut crate::TermWindow,
    layers: &mut TripleLayerQuadAllocator,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    palette: LcarsPalette,
) -> anyhow::Result<()> {
    window.filled_rectangle(
        layers,
        0,
        rect(x - 6.0, y - 2.0, width + 12.0, height + 4.0),
        palette.black,
    )?;
    paint_lcars_generated_bitmap(window, layers, x, y, width, height, |raster| {
        let gutter = 8.0;
        let body_x = gutter;
        let body_w = (width - gutter).max(1.0);
        raster.fill_rect(body_x, 0.0, body_w, height, LCARS_BYTE_ORANGE);
        raster.fill_rect(body_x, 0.0, body_w, 26.0, LCARS_BYTE_PEACH);
        raster.fill_rect(body_x, 32.0, body_w, 6.0, LCARS_BYTE_BLACK);
        raster.fill_rect(body_x, height * 0.46, body_w, 6.0, LCARS_BYTE_BLACK);
        raster.fill_rect(body_x, height - 34.0, body_w, 34.0, LCARS_BYTE_VIOLET);
        raster.fill_rect(0.0, 0.0, gutter, height, LCARS_BYTE_BLACK);
        raster.fill_rect(0.0, height * 0.5 - 14.0, gutter, 28.0, LCARS_BYTE_VIOLET);
    })
}

fn paint_lcars_primary_elbow(
    window: &mut crate::TermWindow,
    layers: &mut TripleLayerQuadAllocator,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    rail_width: f32,
    header_height: f32,
    _palette: LcarsPalette,
) -> anyhow::Result<()> {
    paint_lcars_generated_bitmap(
        window,
        layers,
        x,
        y,
        width.max(1.0),
        height.max(header_height).max(1.0),
        |raster| {
            let header_h = header_height.clamp(12.0, height.max(header_height));
            let rail_w = rail_width.clamp(24.0, width.max(24.0));
            let radius = header_h * 0.5;
            let throat_x = rail_w + 18.0;
            let throat_y = header_h + 12.0;
            let throat_w = (width - throat_x).max(1.0);
            let throat_h = (height - throat_y).max(1.0);

            raster.fill_rounded_rect(0.0, 0.0, width, header_h, radius, LCARS_BYTE_ORANGE);
            raster.fill_rect(radius, 0.0, width - radius, header_h, LCARS_BYTE_ORANGE);
            raster.fill_rect(0.0, 0.0, rail_w, height, LCARS_BYTE_ORANGE);
            raster.fill_rect(0.0, 0.0, rail_w, 30.0, LCARS_BYTE_PEACH);
            raster.fill_rect(0.0, header_h + 6.0, rail_w, 7.0, LCARS_BYTE_BLACK);
            raster.fill_rect(
                0.0,
                (height * 0.48).max(header_h + 20.0),
                rail_w,
                7.0,
                LCARS_BYTE_BLACK,
            );
            raster.fill_rect(0.0, height - 46.0, rail_w, 46.0, LCARS_BYTE_VIOLET);

            raster.fill_rounded_rect(
                throat_x,
                throat_y,
                throat_w,
                throat_h,
                (header_h * 0.78).clamp(18.0, 34.0),
                LCARS_BYTE_BLACK,
            );
            raster.fill_rect(
                throat_x + 26.0,
                throat_y,
                throat_w,
                throat_h,
                LCARS_BYTE_BLACK,
            );
            raster.fill_rect(
                rail_w + 22.0,
                height - 18.0,
                (width - rail_w - 58.0).max(1.0),
                4.0,
                LCARS_BYTE_PEACH,
            );
            raster.fill_rect(
                rail_w + 38.0,
                height - 10.0,
                (width * 0.22).max(32.0),
                3.0,
                LCARS_BYTE_BLUE,
            );
        },
    )
}

fn paint_lcars_content_bay_frame(
    window: &mut crate::TermWindow,
    layers: &mut TripleLayerQuadAllocator,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    _palette: LcarsPalette,
) -> anyhow::Result<()> {
    paint_lcars_generated_bitmap(window, layers, x, y, width, height, |raster| {
        let cap_w = 12.0;
        raster.fill_rect(0.0, 0.0, width, 5.0, LCARS_BYTE_BLUE);
        raster.fill_rect(0.0, 0.0, cap_w, height, LCARS_BYTE_BLUE);
        raster.fill_rect(width - cap_w, 0.0, cap_w, height, LCARS_BYTE_BLUE);
        raster.fill_rect(
            0.0,
            10.0,
            width * 0.42,
            3.0,
            LCARS_BYTE_SCREEN.with_alpha(210),
        );
        raster.fill_rect(width * 0.58, 10.0, width * 0.24, 3.0, LCARS_BYTE_VIOLET);
        raster.fill_rect(
            0.0,
            height - 6.0,
            width,
            2.0,
            LCARS_BYTE_BLUE.with_alpha(190),
        );
        raster.fill_rect(
            0.0,
            5.0,
            width,
            height - 11.0,
            LCARS_BYTE_BLACK.with_alpha(110),
        );
    })
}

fn paint_lcars_bar_run(
    window: &mut crate::TermWindow,
    layers: &mut TripleLayerQuadAllocator,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
) -> anyhow::Result<()> {
    paint_lcars_generated_bitmap(window, layers, x, y, width, height, |raster| {
        let gap = 10.0;
        let mut cursor = 0.0;
        let fixed_segments = [
            (0.46, LCARS_BYTE_ORANGE),
            (0.075, LCARS_BYTE_VIOLET),
            (0.18, LCARS_BYTE_BLUE.with_alpha(230)),
        ];
        for (fraction, fill) in fixed_segments {
            let segment_width = (width * fraction).max(20.0).min((width - cursor).max(0.0));
            if segment_width <= 0.0 {
                return;
            }
            raster.fill_rect(cursor, 0.0, segment_width, height, fill);
            cursor += segment_width + gap;
        }
        let remaining = width - cursor;
        if remaining > height {
            let radius = height * 0.5;
            raster.fill_rect(
                cursor,
                0.0,
                remaining - radius,
                height,
                LCARS_BYTE_BLUE.with_alpha(215),
            );
            raster.fill_rounded_rect(
                cursor + remaining - height,
                0.0,
                height,
                height,
                radius,
                LCARS_BYTE_BLUE.with_alpha(215),
            );
        } else if remaining > 0.0 {
            raster.fill_rect(
                cursor,
                0.0,
                remaining,
                height,
                LCARS_BYTE_BLUE.with_alpha(215),
            );
        }
    })
}

fn paint_lcars_structural_console_chrome(
    window: &mut crate::TermWindow,
    layers: &mut TripleLayerQuadAllocator,
    content_left: f32,
    content_right: f32,
    signal_top: f32,
    content_bay_y: f32,
    plan: &LcarsStructuralPlan,
    action_rows: usize,
    button_height: f32,
    button_gap: f32,
    palette: LcarsPalette,
) -> anyhow::Result<()> {
    let field_width = (content_right - content_left).max(1.0);
    let upper_rule_y = signal_top - 12.0;
    let lower_rule_y = content_bay_y - 17.0;
    window.filled_rectangle(
        layers,
        0,
        rect(content_left, upper_rule_y, field_width * 0.32, 3.0),
        palette.dim_violet,
    )?;
    window.filled_rectangle(
        layers,
        0,
        rect(
            content_left + field_width * 0.28,
            signal_top + 54.0,
            field_width * 0.22,
            3.0,
        ),
        palette.peach,
    )?;
    window.filled_rectangle(
        layers,
        0,
        rect(
            content_left + field_width * 0.42,
            signal_top + 78.0,
            field_width * 0.16,
            2.0,
        ),
        palette.cyan,
    )?;
    window.filled_rectangle(
        layers,
        0,
        rect(content_left, lower_rule_y, field_width * 0.39, 3.0),
        palette.peach,
    )?;
    window.filled_rectangle(
        layers,
        0,
        rect(
            content_left + field_width * 0.54,
            lower_rule_y + 11.0,
            field_width * 0.24,
            3.0,
        ),
        palette.dim_violet,
    )?;

    if plan.detail != LcarsDetailPlacement::None {
        let detail_rule_y = (plan.detail_top - 7.0).max(signal_top - 8.0);
        window.filled_rectangle(
            layers,
            0,
            rect(plan.detail_left, detail_rule_y, plan.detail_width, 2.0),
            palette.dim_violet,
        )?;
        window.filled_rectangle(
            layers,
            0,
            rect(
                plan.detail_left + plan.detail_width * 0.34,
                detail_rule_y + 20.0,
                plan.detail_width * 0.28,
                3.0,
            ),
            palette.amber,
        )?;
    }

    if !plan.command_visible || plan.command_width <= 0.0 {
        return Ok(());
    }

    let bank_x = plan.command_left - 16.0;
    let bank_y = signal_top + 2.0;
    let bank_width = (plan.command_width + 18.0).max(1.0);
    let rows = action_rows.max(1);
    let bank_height =
        (2.0 + (rows as f32 * button_height) + (rows.saturating_sub(1) as f32 * button_gap) + 2.0)
            .min((content_bay_y - bank_y - 14.0).max(button_height + 4.0));

    paint_lcars_generated_bitmap(
        window,
        layers,
        bank_x,
        bank_y,
        bank_width,
        bank_height,
        |raster| {
            let spine_w = 12.0;
            raster.fill_rect(0.0, 0.0, spine_w, bank_height, LCARS_BYTE_BLACK);
            raster.fill_rect(0.0, 0.0, 8.0, bank_height, LCARS_BYTE_ORANGE);
            raster.fill_rect(0.0, 0.0, 8.0, 30.0, LCARS_BYTE_PEACH);
            raster.fill_rect(0.0, bank_height - 26.0, 8.0, 26.0, LCARS_BYTE_VIOLET);
            raster.fill_rect(spine_w, 0.0, bank_width * 0.38, 3.0, LCARS_BYTE_PEACH);
            raster.fill_rect(
                spine_w + bank_width * 0.48,
                bank_height - 3.0,
                bank_width * 0.34,
                3.0,
                LCARS_BYTE_AMBER,
            );
            for row in 0..rows {
                let row_y = 2.0 + (row as f32 * (button_height + button_gap));
                let rail_y = row_y + (button_height * 0.5) - 1.0;
                let rail_fill = match row % 4 {
                    0 => LCARS_BYTE_PEACH,
                    1 => LCARS_BYTE_VIOLET,
                    2 => LCARS_BYTE_BLUE,
                    _ => LCARS_BYTE_AMBER,
                };
                raster.fill_rect(spine_w, rail_y, 24.0, 3.0, rail_fill);
                if row > 0 {
                    let gutter_y = row_y - (button_gap * 0.5);
                    raster.fill_rect(
                        spine_w,
                        gutter_y,
                        bank_width * 0.28,
                        2.0,
                        LCARS_BYTE_BLACK.with_alpha(230),
                    );
                }
            }
        },
    )
}

fn paint_lcars_data_cascade(
    window: &mut crate::TermWindow,
    layers: &mut TripleLayerQuadAllocator,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    document: &InterfaceDocument,
    palette: LcarsPalette,
) -> anyhow::Result<()> {
    let cell_width = window.render_metrics.cell_size.width as f32;
    let cell_height = window.render_metrics.cell_size.height as f32;
    if width < cell_width * 12.0 || height < cell_height * 2.0 {
        return Ok(());
    }

    let inner_x = x + 14.0;
    let inner_y = y + 11.0;
    let inner_width = (width - 24.0).max(cell_width * 10.0);
    let inner_height = (height - 18.0).max(cell_height * 2.0);
    let columns = if inner_width >= 330.0 { 3 } else { 2 };
    let column_width = inner_width / columns as f32;
    let rows = ((inner_height / (cell_height * 0.96)).floor() as usize).clamp(2, 7);
    let seed = document.id.bytes().fold(0u32, |acc, byte| {
        acc.wrapping_mul(33).wrapping_add(byte as u32)
    });
    let colors = [
        RgbColor::new_8bpc(255, 156, 0),
        RgbColor::new_8bpc(153, 204, 255),
        RgbColor::new_8bpc(204, 153, 255),
    ];

    window.filled_rectangle(
        layers,
        0,
        rect(x - 6.0, y - 6.0, width + 12.0, height + 12.0),
        palette.black,
    )?;
    window.filled_rectangle(layers, 0, rect(x, y, width * 0.44, 4.0), palette.dim_violet)?;
    window.filled_rectangle(
        layers,
        0,
        rect(x + width * 0.54, y + 1.0, width * 0.24, 3.0),
        palette.dim_violet,
    )?;
    window.filled_rectangle(
        layers,
        0,
        rect(x, y + 8.0, width * 0.30, 3.0),
        palette.amber,
    )?;
    window.filled_rectangle(
        layers,
        0,
        rect(x, y, 6.0, (height * 0.28).max(18.0)),
        palette.dim_blue,
    )?;
    window.filled_rectangle(
        layers,
        0,
        rect(
            x,
            y + height - (height * 0.24).max(16.0),
            6.0,
            (height * 0.24).max(16.0),
        ),
        palette.peach,
    )?;
    window.filled_rectangle(
        layers,
        0,
        rect(x, y + height - 3.0, width * 0.22, 2.0),
        palette.peach,
    )?;
    window.filled_rectangle(
        layers,
        0,
        rect(x + width * 0.68, y + height - 8.0, width * 0.24, 2.0),
        palette.cyan,
    )?;
    for column in 0..columns {
        let column_x = inner_x + (column as f32 * column_width);
        let max_cols = ((column_width - 6.0) / cell_width).max(5.0) as usize;
        if column > 0 {
            window.filled_rectangle(
                layers,
                0,
                rect(column_x - 8.0, inner_y + 5.0, 2.0, inner_height * 0.78),
                with_alpha(palette.dim_violet, 0.64),
            )?;
        }
        let lane_width = (column_width - 18.0).max(cell_width * 6.0);
        window.filled_rectangle(
            layers,
            0,
            rect(column_x - 2.0, inner_y - 3.0, lane_width, 1.0),
            with_alpha(palette.amber, 0.42),
        )?;
        for row in 0..rows {
            let row_y = inner_y + (row as f32 * cell_height * 0.96);
            if row > 0 {
                window.filled_rectangle(
                    layers,
                    0,
                    rect(column_x + 2.0, row_y - 2.0, lane_width * 0.68, 1.0),
                    with_alpha(palette.dim_blue, 0.16),
                )?;
            }
            let value = seed
                .wrapping_add((column as u32 + 1) * 0x2511)
                .wrapping_mul((row as u32 + 3) * 17)
                % 998_877;
            let text = if row % 3 == 0 {
                format!("{:02} {:05}", row + column + 1, value % 100_000)
            } else {
                format!("{value:06}")
            };
            window.paint_owt_panel_text(
                layers,
                column_x,
                row_y,
                max_cols,
                &text,
                colors[(column + row) % colors.len()],
                false,
            )?;
        }
    }
    Ok(())
}

fn paint_lcars_table_bay(
    window: &mut crate::TermWindow,
    layers: &mut TripleLayerQuadAllocator,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    table: &LcarsTableData,
    palette: LcarsPalette,
) -> anyhow::Result<()> {
    let cell_width = window.render_metrics.cell_size.width as f32;
    let cell_height = window.render_metrics.cell_size.height as f32;
    if width < cell_width * 18.0 || height < cell_height * 4.0 {
        return Ok(());
    }

    window.filled_rectangle(layers, 0, rect(x, y - 4.0, width, 2.0), palette.dim_violet)?;
    window.filled_rectangle(
        layers,
        0,
        rect(x, y, width, height),
        with_alpha(palette.black, 0.96),
    )?;
    window.filled_rectangle(layers, 0, rect(x, y, width, 4.0), palette.amber)?;
    window.filled_rectangle(
        layers,
        0,
        rect(x, y + height - 3.0, width, 2.0),
        palette.dim_blue,
    )?;
    window.filled_rectangle(layers, 0, rect(x, y, 5.0, height), palette.dim_blue)?;
    window.filled_rectangle(
        layers,
        0,
        rect(x + width - 5.0, y, 5.0, height),
        palette.dim_blue,
    )?;

    let title_cols = ((width - 20.0) / cell_width).max(6.0) as usize;
    window.paint_owt_panel_text(
        layers,
        x + 10.0,
        y + 7.0,
        title_cols,
        &table.title.to_uppercase(),
        RgbColor::new_8bpc(255, 204, 102),
        true,
    )?;

    let columns = table.columns.len().max(1);
    let table_left = x + 12.0;
    let table_width = (width - 24.0).max(cell_width * columns as f32);
    let header_y = y + 28.0;
    let row_y = header_y + (cell_height * 1.28);
    let row_height = (cell_height * 1.08).max(14.0);
    let available_rows = ((height - (row_y - y) - 12.0) / row_height)
        .floor()
        .max(0.0) as usize;
    let visible_rows = table.rows.len().min(available_rows);

    for (index, column) in table.columns.iter().enumerate() {
        let (cell_x, cell_cols) =
            table_cell_geometry(table_left, table_width, columns, index, cell_width);
        let fill = match index % 4 {
            0 => palette.peach,
            1 => palette.violet,
            2 => palette.dim_blue,
            _ => palette.amber,
        };
        window.filled_rectangle(
            layers,
            0,
            rect(
                cell_x,
                header_y - 3.0,
                cell_cols as f32 * cell_width - 4.0,
                row_height,
            ),
            fill,
        )?;
        window.paint_owt_panel_text(
            layers,
            cell_x + 4.0,
            header_y,
            cell_cols.saturating_sub(1),
            &fit_text_ellipsis(&column.to_uppercase(), cell_cols.saturating_sub(1)),
            RgbColor::new_8bpc(0, 0, 0),
            true,
        )?;
    }

    for (row_index, row) in table.rows.iter().take(visible_rows).enumerate() {
        let y = row_y + (row_index as f32 * row_height);
        let lane = table_severity_fill(row.severity.as_deref(), row_index, palette);
        window.filled_rectangle(
            layers,
            0,
            rect(table_left, y + 2.0, 8.0, row_height - 4.0),
            lane,
        )?;
        if row_index % 2 == 1 {
            window.filled_rectangle(
                layers,
                0,
                rect(
                    table_left + 12.0,
                    y + row_height - 2.0,
                    table_width - 12.0,
                    1.0,
                ),
                palette.dim_violet,
            )?;
        }
        for (cell_index, cell) in row.cells.iter().enumerate().take(columns) {
            let (cell_x, cell_cols) =
                table_cell_geometry(table_left, table_width, columns, cell_index, cell_width);
            window.paint_owt_panel_text(
                layers,
                cell_x + 4.0,
                y,
                cell_cols.saturating_sub(1),
                &fit_text_ellipsis(cell, cell_cols.saturating_sub(1)),
                table_severity_text(row.severity.as_deref(), cell_index),
                false,
            )?;
        }
    }

    let hidden_rows = table.overflow_rows + table.rows.len().saturating_sub(visible_rows);
    let overflow_text = match (hidden_rows, table.overflow_columns) {
        (0, 0) => None,
        (rows, 0) => Some(format!("+{rows} ROWS")),
        (0, cols) => Some(format!("+{cols} COLS")),
        (rows, cols) => Some(format!("+{rows}R +{cols}C")),
    };
    if let Some(overflow_text) = overflow_text {
        window.paint_owt_panel_text(
            layers,
            x + width - 108.0,
            y + height - cell_height - 4.0,
            14,
            &overflow_text,
            RgbColor::new_8bpc(255, 153, 102),
            true,
        )?;
    }

    Ok(())
}

fn table_cell_geometry(
    x: f32,
    width: f32,
    columns: usize,
    index: usize,
    cell_width: f32,
) -> (f32, usize) {
    let columns = columns.max(1);
    let weights = table_column_weights(columns);
    let total_weight: f32 = weights.iter().sum::<f32>().max(1.0);
    let prior_weight: f32 = weights.iter().take(index.min(columns)).sum();
    let cell_weight = weights
        .get(index.min(columns.saturating_sub(1)))
        .copied()
        .unwrap_or(1.0);
    let cell_x = x + (width * prior_weight / total_weight);
    let cell_width_px = width * cell_weight / total_weight;
    let cell_cols = (cell_width_px / cell_width).floor().max(1.0) as usize;
    (cell_x, cell_cols)
}

fn table_column_weights(columns: usize) -> Vec<f32> {
    match columns {
        0 | 1 => vec![1.0],
        2 => vec![1.2, 1.0],
        3 => vec![1.25, 1.0, 0.9],
        _ => {
            let mut weights = Vec::with_capacity(columns);
            weights.push(1.35);
            weights.push(0.85);
            for index in 2..columns {
                if index + 1 == columns {
                    weights.push(0.9);
                } else {
                    weights.push(1.0);
                }
            }
            weights
        }
    }
}

fn table_severity_fill(severity: Option<&str>, index: usize, palette: LcarsPalette) -> LinearRgba {
    match severity.map(|value| value.to_ascii_lowercase()) {
        Some(value) if matches!(value.as_str(), "error" | "blocked" | "critical") => palette.red,
        Some(value) if matches!(value.as_str(), "warning" | "stale" | "legacy") => palette.amber,
        Some(value) if matches!(value.as_str(), "success" | "ok" | "ready" | "current") => {
            palette.cyan
        }
        _ => palette.signal_fill(index),
    }
}

fn table_severity_text(severity: Option<&str>, cell_index: usize) -> RgbColor {
    match severity.map(|value| value.to_ascii_lowercase()) {
        Some(value) if matches!(value.as_str(), "error" | "blocked" | "critical") => {
            RgbColor::new_8bpc(255, 102, 102)
        }
        Some(value) if matches!(value.as_str(), "warning" | "stale" | "legacy") => {
            RgbColor::new_8bpc(255, 204, 102)
        }
        Some(value) if matches!(value.as_str(), "success" | "ok" | "ready" | "current") => {
            RgbColor::new_8bpc(153, 204, 255)
        }
        _ if cell_index == 0 => RgbColor::new_8bpc(255, 156, 0),
        _ => RgbColor::new_8bpc(153, 204, 255),
    }
}

fn paint_lcars_action_button(
    window: &mut crate::TermWindow,
    layers: &mut TripleLayerQuadAllocator,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    text_cols: usize,
    text: &str,
    text_fg: RgbColor,
    fill: LinearRgba,
    fill_byte: LcarsByteColor,
    active: bool,
    palette: LcarsPalette,
) -> anyhow::Result<()> {
    let accent_byte = lcars_action_accent_byte(fill_byte, active);
    let body_fill = if active { with_alpha(fill, 0.72) } else { fill };
    let cap_width = (height * 0.58).clamp(14.0, 26.0).min(width * 0.22);
    let cap_x = x + width - cap_width;
    let body_width = (width - cap_width * 0.42).max(1.0);
    let label_left = x + 34.0;
    let label_right_margin = cap_width + 18.0;
    let label_strip_width = (width - 34.0 - label_right_margin).max(32.0);

    window.filled_rectangle(layers, 0, rect(x, y, body_width, height), body_fill)?;
    paint_lcars_right_cap_bar(window, layers, cap_x, y, cap_width, height, accent_byte)?;

    window.filled_rectangle(
        layers,
        0,
        rect(x, y, width - cap_width * 0.4, 4.0),
        palette.black,
    )?;
    window.filled_rectangle(
        layers,
        0,
        rect(x, y + height - 4.0, width - cap_width * 0.4, 4.0),
        palette.black,
    )?;
    if active {
        window.filled_rectangle(layers, 0, rect(x, y, 24.0, height), palette.black)?;
        window.filled_rectangle(
            layers,
            0,
            rect(x + 7.0, y + 7.0, 5.0, (height - 14.0).max(4.0)),
            palette.cyan,
        )?;
        window.filled_rectangle(
            layers,
            0,
            rect(x + 15.0, y + 7.0, 4.0, (height - 14.0).max(4.0)),
            palette.black,
        )?;
    } else {
        window.filled_rectangle(layers, 0, rect(x, y, 24.0, height), palette.black)?;
    }
    window.filled_rectangle(
        layers,
        0,
        rect(label_left, y + height - 9.0, label_strip_width, 3.0),
        palette.black,
    )?;
    if active {
        window.filled_rectangle(
            layers,
            0,
            rect(label_left, y + 7.0, label_strip_width, 2.0),
            palette.black,
        )?;
        window.filled_rectangle(
            layers,
            0,
            rect(cap_x - 8.0, y + 7.0, 5.0, (height - 14.0).max(3.0)),
            palette.black,
        )?;
    }

    let cell_width = window.render_metrics.cell_size.width as f32;
    let cell_height = window.render_metrics.cell_size.height as f32;
    let pressed_offset = if active { 2.0 } else { 0.0 };
    let label_strip_x = label_left;
    let label_strip_height = (cell_height + 6.0).min(height - 8.0).max(cell_height);
    let label_strip_y = y + ((height - label_strip_height) * 0.5).max(0.0) + pressed_offset;
    window.filled_rectangle(
        layers,
        0,
        rect(
            label_strip_x,
            label_strip_y,
            label_strip_width,
            label_strip_height,
        ),
        palette.black,
    )?;

    let label_cols = (label_strip_width / cell_width).floor().max(1.0) as usize;
    window.paint_owt_panel_text(
        layers,
        label_strip_x + 6.0,
        label_strip_y + ((label_strip_height - cell_height) * 0.5).max(0.0),
        label_cols.min(text_cols.max(1)),
        text,
        if active {
            RgbColor::new_8bpc(255, 240, 176)
        } else {
            text_fg
        },
        true,
    )?;
    Ok(())
}

fn paint_lcars_right_cap_bar(
    window: &mut crate::TermWindow,
    layers: &mut TripleLayerQuadAllocator,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    fill: LcarsByteColor,
) -> anyhow::Result<()> {
    paint_lcars_generated_bitmap(window, layers, x, y, width, height, |raster| {
        if width <= height {
            raster.fill_rounded_rect(0.0, 0.0, width, height, height * 0.5, fill);
            return;
        }
        let radius = height * 0.5;
        raster.fill_rect(0.0, 0.0, width - radius, height, fill);
        raster.fill_rounded_rect(width - height, 0.0, height, height, radius, fill);
    })
}

fn paint_progress_rail(
    window: &mut crate::TermWindow,
    layers: &mut TripleLayerQuadAllocator,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    progress: Option<f32>,
    track: LinearRgba,
    fill: LinearRgba,
) -> anyhow::Result<()> {
    let width = width.max(0.0);
    let height = height.max(1.0);
    if width <= 0.0 {
        return Ok(());
    }

    window.filled_rectangle(layers, 0, rect(x, y, width, height), track)?;
    window.filled_rectangle(
        layers,
        0,
        rect(
            x,
            y,
            width * progress.unwrap_or(0.0).clamp(0.0, 1.0),
            height,
        ),
        fill,
    )?;
    Ok(())
}

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
}

fn paint_lcars_generated_bitmap<F>(
    window: &mut crate::TermWindow,
    layers: &mut TripleLayerQuadAllocator,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    draw: F,
) -> anyhow::Result<()>
where
    F: FnOnce(&mut LcarsRaster),
{
    if width <= 0.0 || height <= 0.0 {
        return Ok(());
    }
    let cell_width = window.render_metrics.cell_size.width as f32;
    let cell_height = window.render_metrics.cell_size.height as f32;
    let cols = (width / cell_width).ceil().max(1.0) as usize;
    let rows = (height / cell_height).ceil().max(1.0) as usize;
    let canvas_width = (cols as f32 * cell_width).ceil().max(1.0) as u32;
    let canvas_height = (rows as f32 * cell_height).ceil().max(1.0) as u32;
    let mut raster = LcarsRaster::new(canvas_width, canvas_height);
    draw(&mut raster);
    paint_lcars_rgba_cells(window, layers, x, y, cols, rows, raster)
}

fn paint_lcars_rgba_cells(
    window: &mut crate::TermWindow,
    layers: &mut TripleLayerQuadAllocator,
    x: f32,
    y: f32,
    cols: usize,
    rows: usize,
    raster: LcarsRaster,
) -> anyhow::Result<()> {
    let image_data = Arc::new(ImageData::with_data(ImageDataType::new_single_frame(
        raster.width,
        raster.height,
        raster.pixels,
    )));
    let cell_width = window.render_metrics.cell_size.width as f32;
    let cell_height = window.render_metrics.cell_size.height as f32;
    let palette = window.palette().clone();
    let gl_state = window.render_state.as_ref().unwrap();
    let white_space = gl_state.util_sprites.white_space.texture_coords();
    let filled_box = gl_state.util_sprites.filled_box.texture_coords();
    let attrs = CellAttributes::blank();
    let line = Line::from_text("", &attrs, 0, None);
    let cursor = StableCursorPosition::default();
    let dims = RenderableDimensions {
        cols,
        physical_top: 0,
        scrollback_rows: 0,
        scrollback_top: 0,
        viewport_rows: 1,
        dpi: window.terminal_size.dpi,
        pixel_height: cell_height.ceil() as usize,
        pixel_width: (cols as f32 * cell_width).ceil() as usize,
        reverse_video: false,
    };

    for row in 0..rows {
        let top = (row as f32 * cell_height) / raster.height as f32;
        let bottom = (((row + 1) as f32 * cell_height) / raster.height as f32).min(1.0);
        let params = RenderScreenLineParams {
            top_pixel_y: y + (row as f32 * cell_height),
            left_pixel_x: x,
            pixel_width: cols as f32 * cell_width,
            stable_line_idx: None,
            line: &line,
            selection: 0..0,
            cursor: &cursor,
            palette: &palette,
            dims: &dims,
            config: &window.config,
            pane: None,
            white_space,
            filled_box,
            cursor_border_color: LinearRgba::TRANSPARENT,
            foreground: color(255, 255, 255),
            is_active: true,
            selection_fg: LinearRgba::TRANSPARENT,
            selection_bg: LinearRgba::TRANSPARENT,
            cursor_fg: LinearRgba::TRANSPARENT,
            cursor_bg: LinearRgba::TRANSPARENT,
            cursor_is_default_color: true,
            window_is_transparent: false,
            default_bg: LinearRgba::TRANSPARENT,
            font: None,
            style: None,
            use_pixel_positioning: window.config.experimental_pixel_positioning,
            render_metrics: window.render_metrics,
            shape_key: None,
            password_input: false,
        };

        for col in 0..cols {
            let left = (col as f32 * cell_width) / raster.width as f32;
            let right = (((col + 1) as f32 * cell_width) / raster.width as f32).min(1.0);
            let image = ImageCell::new(
                TextureCoordinate::new_f32(left, top),
                TextureCoordinate::new_f32(right, bottom),
                Arc::clone(&image_data),
            );
            window.populate_image_quad(
                &image,
                gl_state,
                layers,
                0,
                col,
                &params,
                None,
                color(255, 255, 255),
            )?;
        }
    }

    Ok(())
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

fn compact_scope_id(value: &str) -> String {
    const MAX_LEN: usize = 42;
    if value.chars().count() <= MAX_LEN {
        return value.to_string();
    }
    let tail = value
        .chars()
        .rev()
        .take(MAX_LEN.saturating_sub(3))
        .collect::<Vec<_>>();
    format!("...{}", tail.into_iter().rev().collect::<String>())
}

fn fit_text_ellipsis(text: &str, max_cols: usize) -> String {
    let normalized = text.replace(['\r', '\n'], " ");
    if max_cols == 0 {
        return String::new();
    }
    if normalized.chars().count() <= max_cols {
        return normalized;
    }
    if max_cols <= 3 {
        return normalized.chars().take(max_cols).collect();
    }
    let head = normalized
        .chars()
        .take(max_cols.saturating_sub(3))
        .collect::<String>();
    format!("{head}...")
}

fn wrap_text_lines(text: &str, max_cols: usize, max_lines: usize) -> Vec<String> {
    let max_cols = max_cols.max(1);
    let max_lines = max_lines.max(1);
    let normalized = text.replace(['\r', '\n'], " ");
    let mut lines = Vec::new();
    let mut current = String::new();
    let mut overflow = false;

    for word in normalized.split_whitespace() {
        let word_len = word.chars().count();
        let current_len = current.chars().count();
        let separator = usize::from(!current.is_empty());
        if current_len + separator + word_len <= max_cols {
            if !current.is_empty() {
                current.push(' ');
            }
            current.push_str(word);
            continue;
        }

        if !current.is_empty() {
            lines.push(current);
            current = String::new();
            if lines.len() >= max_lines {
                overflow = true;
                break;
            }
        }

        if word_len <= max_cols {
            current.push_str(word);
        } else {
            let mut remaining = word;
            while !remaining.is_empty() {
                if lines.len() + usize::from(!current.is_empty()) >= max_lines {
                    overflow = true;
                    break;
                }
                let chunk = remaining.chars().take(max_cols).collect::<String>();
                let consumed = chunk.len();
                if current.is_empty() {
                    current = chunk;
                } else {
                    lines.push(current);
                    current = chunk;
                }
                remaining = &remaining[consumed..];
                if current.chars().count() >= max_cols {
                    lines.push(current);
                    current = String::new();
                }
            }
            if overflow {
                break;
            }
        }
    }

    if !current.is_empty() && lines.len() < max_lines {
        lines.push(current);
    } else if !current.is_empty() {
        overflow = true;
    }

    if lines.is_empty() {
        lines.push(String::new());
    }
    if overflow {
        if let Some(last) = lines.last_mut() {
            *last = mark_text_overflow(last, max_cols);
        }
    }
    lines
}

fn mark_text_overflow(text: &str, max_cols: usize) -> String {
    if max_cols == 0 {
        return String::new();
    }
    if max_cols <= 3 {
        return ".".repeat(max_cols);
    }
    let head = text
        .chars()
        .take(max_cols.saturating_sub(3))
        .collect::<String>();
    format!("{head}...")
}

fn fit_text(text: &str, max_cols: usize) -> String {
    let mut output = String::new();
    for ch in text.replace(['\r', '\n'], " ").chars() {
        if output.chars().count() >= max_cols {
            break;
        }
        output.push(ch);
    }
    output
}

fn color(red: u8, green: u8, blue: u8) -> LinearRgba {
    RgbColor::new_8bpc(red, green, blue).to_linear_tuple_rgba()
}

fn with_alpha(color: LinearRgba, alpha: f32) -> LinearRgba {
    let (red, green, blue, _) = color.tuple();
    LinearRgba::with_components(red, green, blue, alpha)
}

fn rect(x: f32, y: f32, width: f32, height: f32) -> RectF {
    RectF::new(
        euclid::Point2D::<f32, PixelUnit>::new(x, y),
        euclid::Size2D::<f32, PixelUnit>::new(width.max(0.0), height.max(0.0)),
    )
}

#[cfg(test)]
mod tests {
    use super::{
        lcars_action_accent_byte, lcars_action_fill_byte, lcars_structural_breakpoint,
        plan_lcars_structural_console, structural_signal_text_cols,
        structural_signal_text_needs_backing, structural_signal_visual_limit, wrap_text_lines,
        LcarsDetailPlacement, LcarsStructuralBreakpoint, PanelLineKind,
    };

    #[test]
    fn wraps_signal_text_on_word_boundaries() {
        assert_eq!(
            wrap_text_lines("fleet kernel cohort requires review", 18, 2),
            vec!["fleet kernel".to_string(), "cohort requires...".to_string()]
        );
    }

    #[test]
    fn marks_overflow_when_signal_text_exceeds_available_lines() {
        assert_eq!(
            wrap_text_lines("fleet kernel cohort requires review", 12, 2),
            vec!["fleet kernel".to_string(), "cohort...".to_string()]
        );
    }

    #[test]
    fn wraps_long_unbroken_signal_text() {
        assert_eq!(
            wrap_text_lines("abcdefghijk", 4, 3),
            vec!["abcd".to_string(), "efgh".to_string(), "ijk".to_string()]
        );
    }

    #[test]
    fn hides_command_grid_when_no_actions_are_declared() {
        let no_actions =
            plan_lcars_structural_console(120.0, 1500.0, 90.0, 320.0, 10.0, 20.0, true, false, 0);
        let with_actions =
            plan_lcars_structural_console(120.0, 1500.0, 90.0, 320.0, 10.0, 20.0, true, false, 4);

        assert!(!no_actions.command_visible);
        assert_eq!(no_actions.action_slots, 0);
        assert_eq!(no_actions.command_width, 0.0);
        assert!(with_actions.command_visible);
        assert!(with_actions.action_slots > 0);
        assert!(no_actions.detail_width >= with_actions.detail_width);
    }

    #[test]
    fn structural_breakpoints_keep_compact_command_banks_small() {
        let compact =
            plan_lcars_structural_console(120.0, 820.0, 90.0, 320.0, 10.0, 20.0, false, true, 4);
        let wide =
            plan_lcars_structural_console(120.0, 1800.0, 90.0, 320.0, 10.0, 20.0, false, true, 4);

        assert_eq!(
            lcars_structural_breakpoint(700.0, 10.0),
            LcarsStructuralBreakpoint::Compact
        );
        assert_eq!(compact.detail, LcarsDetailPlacement::Inline);
        assert!(compact.command_visible);
        assert!(!compact.two_action_columns);
        assert!(compact.command_width <= 270.0);
        assert_eq!(
            lcars_structural_breakpoint(1680.0, 10.0),
            LcarsStructuralBreakpoint::Wide
        );
        assert!(wide.two_action_columns);
        assert!(wide.command_width > compact.command_width);
        assert!(wide.detail_width > compact.detail_width);
    }

    #[test]
    fn stacked_detail_reserves_signal_overflow_guard() {
        let stacked =
            plan_lcars_structural_console(120.0, 760.0, 90.0, 320.0, 10.0, 20.0, false, true, 4);

        assert_eq!(stacked.detail, LcarsDetailPlacement::Stacked);
        let visual_limit = structural_signal_visual_limit(&stacked, 320.0, 20.0);
        assert!(visual_limit < stacked.detail_top);
        assert!(visual_limit <= stacked.detail_top - 7.0);
    }

    #[test]
    fn structural_signal_text_preserves_label_bar_width() {
        let signal_cols = 52;

        let content_cols =
            structural_signal_text_cols(PanelLineKind::ContentBay, 520.0, 10.0, signal_cols);
        let metric_cols =
            structural_signal_text_cols(PanelLineKind::Metric, 520.0, 10.0, signal_cols);

        assert!(content_cols < metric_cols);
        assert!(content_cols >= metric_cols.saturating_sub(4));
        assert_eq!(metric_cols, signal_cols - 2);
    }

    #[test]
    fn active_action_ack_accent_is_distinct_from_default_button_fills() {
        for index in 0..4 {
            let default_fill = lcars_action_fill_byte(index);
            let active_fill = lcars_action_accent_byte(default_fill, true);
            assert_eq!(active_fill.red, default_fill.red);
            assert_eq!(active_fill.green, default_fill.green);
            assert_eq!(active_fill.blue, default_fill.blue);
            assert!(active_fill.alpha < default_fill.alpha);
            assert_ne!(lcars_action_accent_byte(default_fill, true), default_fill);
            assert_eq!(lcars_action_accent_byte(default_fill, false), default_fill);
        }
    }

    #[test]
    fn structural_signal_backing_tracks_chrome_marker_kinds() {
        assert!(structural_signal_text_needs_backing(
            PanelLineKind::ContentBay
        ));
        assert!(structural_signal_text_needs_backing(
            PanelLineKind::Progress
        ));
        assert!(!structural_signal_text_needs_backing(PanelLineKind::Metric));
    }
}
