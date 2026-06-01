use crate::overlay::owt_dataview::{validate_dataview_document, LcarsDataViewDocument};
use crate::quad::{HeapQuadAllocator, QuadTrait, TripleLayerQuadAllocator};
use crate::selection::SelectionRange;
use crate::termwindow::box_model::*;
use crate::termwindow::render::{
    same_hyperlink, CursorProperties, LineQuadCacheKey, LineQuadCacheValue, LineToEleShapeCacheKey,
    RenderScreenLineParams,
};
use crate::termwindow::{ScrollHit, UIItem, UIItemType};
use ::window::bitmaps::TextureRect;
use ::window::DeadKeyStatus;
use anyhow::Context;
use config::VisualBellTarget;
use mux::pane::{PaneId, WithPaneLines};
use mux::renderable::{RenderableDimensions, StableCursorPosition};
use mux::tab::PositionedPane;
use ordered_float::NotNan;
use std::ops::Range;
use std::time::Instant;
use termwiz::cell::Intensity;
use termwiz::color::{ColorSpec, RgbColor};
use wezterm_dynamic::Value;
use wezterm_term::color::{ColorAttribute, ColorPalette};
use wezterm_term::{CellAttributes, Line, OwtTranscriptEvent, StableRowIndex};
use window::color::LinearRgba;

const LCARS_MARKDOWN_LOOKBACK_ROWS: StableRowIndex = 512;

impl crate::TermWindow {
    fn paint_pane_box_model(&mut self, pos: &PositionedPane) -> anyhow::Result<()> {
        let computed = self.build_pane(pos)?;
        let mut ui_items = computed.ui_items();
        self.ui_items.append(&mut ui_items);
        let gl_state = self.render_state.as_ref().unwrap();
        self.render_element(&computed, gl_state, None)
    }

    pub fn paint_pane(
        &mut self,
        pos: &PositionedPane,
        layers: &mut TripleLayerQuadAllocator,
    ) -> anyhow::Result<()> {
        if self.config.use_box_model_render {
            return self.paint_pane_box_model(pos);
        }

        self.check_for_dirty_lines_and_invalidate_selection(&pos.pane);
        /*
        let zone = {
            let dims = pos.pane.get_dimensions();
            let position = self
                .get_viewport(pos.pane.pane_id())
                .unwrap_or(dims.physical_top);

            let zones = self.get_semantic_zones(&pos.pane);
            let idx = match zones.binary_search_by(|zone| zone.start_y.cmp(&position)) {
                Ok(idx) | Err(idx) => idx,
            };
            let idx = ((idx as isize) - 1).max(0) as usize;
            zones.get(idx).cloned()
        };
        */

        let global_cursor_fg = self.palette().cursor_fg;
        let global_cursor_bg = self.palette().cursor_bg;
        let config = self.config.clone();
        let palette = pos.pane.palette();

        let (padding_left, padding_top) = self.padding_left_top();

        let tab_bar_height = if self.show_tab_bar {
            self.tab_bar_pixel_height()
                .context("tab_bar_pixel_height")?
        } else {
            0.
        };
        let (top_bar_height, bottom_bar_height) = if self.config.tab_bar_at_bottom {
            (0.0, tab_bar_height)
        } else {
            (tab_bar_height, 0.0)
        };

        let border = self.get_os_border();
        let top_pixel_y = top_bar_height + padding_top + border.top.get() as f32;

        let cursor = pos.pane.get_cursor_position();
        if pos.is_active {
            self.prev_cursor.update(&cursor);
        }

        let pane_id = pos.pane.pane_id();
        let current_viewport = self.get_viewport(pane_id);
        let dims = pos.pane.get_dimensions();

        let gl_state = self.render_state.as_ref().unwrap();

        let cursor_border_color = palette.cursor_border.to_linear();
        let foreground = palette.foreground.to_linear();
        let white_space = gl_state.util_sprites.white_space.texture_coords();
        let filled_box = gl_state.util_sprites.filled_box.texture_coords();

        let window_is_transparent =
            !self.window_background.is_empty() || config.window_background_opacity != 1.0;

        let default_bg = palette
            .resolve_bg(ColorAttribute::Default)
            .to_linear()
            .mul_alpha(if window_is_transparent {
                0.
            } else {
                config.text_background_opacity
            });

        let cell_width = self.render_metrics.cell_size.width as f32;
        let cell_height = self.render_metrics.cell_size.height as f32;
        let background_rect = {
            // We want to fill out to the edges of the splits
            let (x, width_delta) = if pos.left == 0 {
                (
                    0.,
                    padding_left + border.left.get() as f32 + (cell_width / 2.0),
                )
            } else {
                (
                    padding_left + border.left.get() as f32 - (cell_width / 2.0)
                        + (pos.left as f32 * cell_width),
                    cell_width,
                )
            };

            let (y, height_delta) = if pos.top == 0 {
                (
                    (top_pixel_y - padding_top),
                    padding_top + (cell_height / 2.0),
                )
            } else {
                (
                    top_pixel_y + (pos.top as f32 * cell_height) - (cell_height / 2.0),
                    cell_height,
                )
            };
            euclid::rect(
                x,
                y,
                // Go all the way to the right edge if we're right-most
                if pos.left + pos.width >= self.terminal_size.cols as usize {
                    self.dimensions.pixel_width as f32 - x
                } else {
                    (pos.width as f32 * cell_width) + width_delta
                },
                // Go all the way to the bottom if we're bottom-most
                if pos.top + pos.height >= self.terminal_size.rows as usize {
                    self.dimensions.pixel_height as f32 - y
                } else {
                    (pos.height as f32 * cell_height) + height_delta as f32
                },
            )
        };

        if self.window_background.is_empty() {
            // Per-pane, palette-specified background

            let mut quad = self
                .filled_rectangle(
                    layers,
                    0,
                    background_rect,
                    palette
                        .background
                        .to_linear()
                        .mul_alpha(config.window_background_opacity),
                )
                .context("filled_rectangle")?;
            quad.set_hsv(if pos.is_active {
                None
            } else {
                Some(config.inactive_pane_hsb)
            });
        }

        {
            // If the bell is ringing, we draw another background layer over the
            // top of this in the configured bell color
            if let Some(intensity) = self.get_intensity_if_bell_target_ringing(
                &pos.pane,
                &config,
                VisualBellTarget::BackgroundColor,
            ) {
                // target background color
                let LinearRgba(r, g, b, _) = config
                    .resolved_palette
                    .visual_bell
                    .as_deref()
                    .unwrap_or(&palette.foreground)
                    .to_linear();

                let background = if window_is_transparent {
                    // for transparent windows, we fade in the target color
                    // by adjusting its alpha
                    LinearRgba::with_components(r, g, b, intensity)
                } else {
                    // otherwise We'll interpolate between the background color
                    // and the the target color
                    let (r1, g1, b1, a) = palette
                        .background
                        .to_linear()
                        .mul_alpha(config.window_background_opacity)
                        .tuple();
                    LinearRgba::with_components(
                        r1 + (r - r1) * intensity,
                        g1 + (g - g1) * intensity,
                        b1 + (b - b1) * intensity,
                        a,
                    )
                };
                log::trace!("bell color is {:?}", background);

                let mut quad = self
                    .filled_rectangle(layers, 0, background_rect, background)
                    .context("filled_rectangle")?;

                quad.set_hsv(if pos.is_active {
                    None
                } else {
                    Some(config.inactive_pane_hsb)
                });
            }
        }

        // TODO: we only have a single scrollbar in a single position.
        // We only update it for the active pane, but we should probably
        // do a per-pane scrollbar.  That will require more extensive
        // changes to ScrollHit, mouse positioning, PositionedPane
        // and tab size calculation.
        if pos.is_active && self.show_scroll_bar {
            let thumb_y_offset = top_bar_height as usize + border.top.get();

            let min_height = self.min_scroll_bar_height();

            let info = ScrollHit::thumb(
                &*pos.pane,
                current_viewport,
                self.dimensions.pixel_height.saturating_sub(
                    thumb_y_offset + border.bottom.get() + bottom_bar_height as usize,
                ),
                min_height as usize,
            );
            let abs_thumb_top = thumb_y_offset + info.top;
            let thumb_size = info.height;
            let color = palette.scrollbar_thumb.to_linear();

            // Adjust the scrollbar thumb position
            let config = &self.config;
            let padding = self.effective_right_padding(&config) as f32;

            let thumb_x = self.dimensions.pixel_width - padding as usize - border.right.get();

            // Register the scroll bar location
            self.ui_items.push(UIItem {
                x: thumb_x,
                width: padding as usize,
                y: thumb_y_offset,
                height: info.top,
                item_type: UIItemType::AboveScrollThumb,
            });
            self.ui_items.push(UIItem {
                x: thumb_x,
                width: padding as usize,
                y: abs_thumb_top,
                height: thumb_size,
                item_type: UIItemType::ScrollThumb,
            });
            self.ui_items.push(UIItem {
                x: thumb_x,
                width: padding as usize,
                y: abs_thumb_top + thumb_size,
                height: self
                    .dimensions
                    .pixel_height
                    .saturating_sub(abs_thumb_top + thumb_size),
                item_type: UIItemType::BelowScrollThumb,
            });

            self.filled_rectangle(
                layers,
                2,
                euclid::rect(
                    thumb_x as f32,
                    abs_thumb_top as f32,
                    padding,
                    thumb_size as f32,
                ),
                color,
            )
            .context("filled_rectangle")?;
        }

        let (selrange, rectangular) = {
            let sel = self.selection(pos.pane.pane_id());
            (sel.range.clone(), sel.rectangular)
        };

        let start = Instant::now();
        let selection_fg = palette.selection_fg.to_linear();
        let selection_bg = palette.selection_bg.to_linear();
        let cursor_fg = palette.cursor_fg.to_linear();
        let cursor_bg = palette.cursor_bg.to_linear();
        let cursor_is_default_color =
            palette.cursor_fg == global_cursor_fg && palette.cursor_bg == global_cursor_bg;

        let stable_range = match current_viewport {
            Some(top) => top..top + dims.viewport_rows as StableRowIndex,
            None => dims.physical_top..dims.physical_top + dims.viewport_rows as StableRowIndex,
        };

        let left_pixel_x = padding_left
            + border.left.get() as f32
            + (pos.left as f32 * self.render_metrics.cell_size.width as f32);

        {
            pos.pane
                .apply_hyperlinks(stable_range.clone(), &self.config.hyperlink_rules);

            struct LineRender<'a, 'b> {
                term_window: &'a mut crate::TermWindow,
                selrange: Option<SelectionRange>,
                rectangular: bool,
                dims: RenderableDimensions,
                top_pixel_y: f32,
                left_pixel_x: f32,
                pos: &'a PositionedPane,
                pane_id: PaneId,
                cursor: &'a StableCursorPosition,
                palette: &'a ColorPalette,
                default_bg: LinearRgba,
                cursor_border_color: LinearRgba,
                selection_fg: LinearRgba,
                selection_bg: LinearRgba,
                cursor_fg: LinearRgba,
                cursor_bg: LinearRgba,
                foreground: LinearRgba,
                cursor_is_default_color: bool,
                white_space: TextureRect,
                filled_box: TextureRect,
                window_is_transparent: bool,
                layers: &'a mut TripleLayerQuadAllocator<'b>,
                error: Option<anyhow::Error>,
            }

            let mut render = LineRender {
                term_window: self,
                selrange,
                rectangular,
                dims,
                top_pixel_y,
                left_pixel_x,
                pos,
                pane_id,
                cursor: &cursor,
                palette: &palette,
                cursor_border_color,
                selection_fg,
                selection_bg,
                cursor_fg,
                default_bg,
                cursor_bg,
                foreground,
                cursor_is_default_color,
                white_space,
                filled_box,
                window_is_transparent,
                layers,
                error: None,
            };

            impl<'a, 'b> LineRender<'a, 'b> {
                fn render_line(
                    &mut self,
                    stable_top: StableRowIndex,
                    line_idx: usize,
                    line: &&mut Line,
                ) -> anyhow::Result<()> {
                    let stable_row = stable_top + line_idx as StableRowIndex;
                    let selrange = self
                        .selrange
                        .map_or(0..0, |sel| sel.cols_for_row(stable_row, self.rectangular));
                    // Constrain to the pane width!
                    let selrange = selrange.start..selrange.end.min(self.dims.cols);

                    let (cursor, composing, password_input) = if self.cursor.y == stable_row {
                        (
                            Some(CursorProperties {
                                position: StableCursorPosition {
                                    y: 0,
                                    ..*self.cursor
                                },
                                dead_key_or_leader: self.term_window.dead_key_status
                                    != DeadKeyStatus::None
                                    || self.term_window.leader_is_active(),
                                cursor_fg: self.cursor_fg,
                                cursor_bg: self.cursor_bg,
                                cursor_border_color: self.cursor_border_color,
                                cursor_is_default_color: self.cursor_is_default_color,
                            }),
                            match (self.pos.is_active, &self.term_window.dead_key_status) {
                                (true, DeadKeyStatus::Composing(composing)) => {
                                    Some(composing.to_string())
                                }
                                _ => None,
                            },
                            if self.term_window.config.detect_password_input {
                                match self.pos.pane.get_metadata() {
                                    Value::Object(obj) => {
                                        match obj.get(&Value::String("password_input".to_string()))
                                        {
                                            Some(Value::Bool(b)) => *b,
                                            _ => false,
                                        }
                                    }
                                    _ => false,
                                }
                            } else {
                                false
                            },
                        )
                    } else {
                        (None, None, false)
                    };

                    let shape_hash = self.term_window.shape_hash_for_line(line);

                    let quad_key = LineQuadCacheKey {
                        pane_id: self.pane_id,
                        password_input,
                        pane_is_active: self.pos.is_active,
                        config_generation: self.term_window.config.generation(),
                        shape_generation: self.term_window.shape_generation,
                        quad_generation: self.term_window.quad_generation,
                        composing: composing.clone(),
                        selection: selrange.clone(),
                        cursor,
                        shape_hash,
                        top_pixel_y: NotNan::new(self.top_pixel_y).unwrap()
                            + (line_idx + self.pos.top) as f32
                                * self.term_window.render_metrics.cell_size.height as f32,
                        left_pixel_x: NotNan::new(self.left_pixel_x).unwrap(),
                        phys_line_idx: line_idx,
                        reverse_video: self.dims.reverse_video,
                    };

                    if let Some(cached_quad) =
                        self.term_window.line_quad_cache.borrow_mut().get(&quad_key)
                    {
                        let expired = cached_quad
                            .expires
                            .map(|i| Instant::now() >= i)
                            .unwrap_or(false);
                        let hover_changed = if cached_quad.invalidate_on_hover_change {
                            !same_hyperlink(
                                cached_quad.current_highlight.as_ref(),
                                self.term_window.current_highlight.as_ref(),
                            )
                        } else {
                            false
                        };
                        if !expired && !hover_changed {
                            cached_quad
                                .layers
                                .apply_to(self.layers)
                                .context("cached_quad.layers.apply_to")?;
                            self.term_window.update_next_frame_time(cached_quad.expires);
                            return Ok(());
                        }
                    }

                    let mut buf = HeapQuadAllocator::default();
                    let next_due = self.term_window.has_animation.borrow_mut().take();

                    let shape_key = LineToEleShapeCacheKey {
                        shape_hash,
                        shape_generation: quad_key.shape_generation,
                        composing: if self.cursor.y == stable_row && self.pos.is_active {
                            if let DeadKeyStatus::Composing(composing) =
                                &self.term_window.dead_key_status
                            {
                                Some((self.cursor.x, composing.to_string()))
                            } else {
                                None
                            }
                        } else {
                            None
                        },
                    };

                    let render_result = self
                        .term_window
                        .render_screen_line(
                            RenderScreenLineParams {
                                top_pixel_y: *quad_key.top_pixel_y,
                                left_pixel_x: self.left_pixel_x,
                                pixel_width: self.dims.cols as f32
                                    * self.term_window.render_metrics.cell_size.width as f32,
                                stable_line_idx: Some(stable_row),
                                line: &line,
                                selection: selrange.clone(),
                                cursor: &self.cursor,
                                palette: &self.palette,
                                dims: &self.dims,
                                config: &self.term_window.config,
                                cursor_border_color: self.cursor_border_color,
                                foreground: self.foreground,
                                is_active: self.pos.is_active,
                                pane: Some(&self.pos.pane),
                                selection_fg: self.selection_fg,
                                selection_bg: self.selection_bg,
                                cursor_fg: self.cursor_fg,
                                cursor_bg: self.cursor_bg,
                                cursor_is_default_color: self.cursor_is_default_color,
                                white_space: self.white_space,
                                filled_box: self.filled_box,
                                window_is_transparent: self.window_is_transparent,
                                default_bg: self.default_bg,
                                font: None,
                                style: None,
                                use_pixel_positioning: self
                                    .term_window
                                    .config
                                    .experimental_pixel_positioning,
                                render_metrics: self.term_window.render_metrics,
                                shape_key: Some(shape_key),
                                password_input,
                            },
                            &mut TripleLayerQuadAllocator::Heap(&mut buf),
                        )
                        .context("render_screen_line")?;

                    let expires = self.term_window.has_animation.borrow().as_ref().cloned();
                    self.term_window.update_next_frame_time(next_due);

                    buf.apply_to(self.layers)
                        .context("HeapQuadAllocator::apply_to")?;

                    let quad_value = LineQuadCacheValue {
                        layers: buf,
                        expires,
                        line: (*line).clone(),
                        invalidate_on_hover_change: render_result.invalidate_on_hover_change,
                        current_highlight: if render_result.invalidate_on_hover_change {
                            self.term_window.current_highlight.clone()
                        } else {
                            None
                        },
                    };

                    self.term_window
                        .line_quad_cache
                        .borrow_mut()
                        .put(quad_key, quad_value);

                    Ok(())
                }
            }

            impl<'a, 'b> WithPaneLines for LineRender<'a, 'b> {
                fn with_lines_mut(&mut self, stable_top: StableRowIndex, lines: &mut [&mut Line]) {
                    for (line_idx, line) in lines.iter().enumerate() {
                        if let Err(err) = self.render_line(stable_top, line_idx, line) {
                            self.error.replace(err);
                            return;
                        }
                    }
                }
            }

            pos.pane.with_lines_mut(stable_range.clone(), &mut render);
            if let Some(error) = render.error.take() {
                return Err(error).context("error while calling with_lines_mut");
            }
        }

        let markdown_scan_start = stable_range
            .start
            .saturating_sub(LCARS_MARKDOWN_LOOKBACK_ROWS);
        let markdown_scan_lines =
            collect_lcars_markdown_lines(pos, markdown_scan_start..stable_range.end);
        self.paint_lcars_markdown_annotations(
            pos,
            layers,
            stable_range.clone(),
            left_pixel_x,
            top_pixel_y,
            dims.cols,
            palette
                .background
                .to_linear()
                .mul_alpha(config.window_background_opacity),
            &markdown_scan_lines,
        )?;
        self.paint_owt_transcript_markers(pos, layers, stable_range, left_pixel_x, top_pixel_y)?;

        /*
        if let Some(zone) = zone {
            // TODO: render a thingy to jump to prior prompt
        }
        */
        metrics::histogram!("paint_pane.lines", start.elapsed());
        log::trace!("lines elapsed {:?}", start.elapsed());

        Ok(())
    }

    fn paint_owt_transcript_markers(
        &mut self,
        pos: &PositionedPane,
        layers: &mut TripleLayerQuadAllocator,
        stable_range: Range<StableRowIndex>,
        left_pixel_x: f32,
        top_pixel_y: f32,
    ) -> anyhow::Result<()> {
        let events = pos.pane.copy_owt_transcript_events();
        if events.is_empty() {
            return Ok(());
        }

        let markers = events.iter().filter_map(|event| {
            OwtTranscriptMarkerVisual::for_event(event).map(|visual| (event.row, visual))
        });
        self.paint_owt_marker_chrome(
            pos,
            layers,
            stable_range,
            left_pixel_x,
            top_pixel_y,
            markers,
        )
    }

    fn paint_lcars_markdown_annotations(
        &mut self,
        pos: &PositionedPane,
        layers: &mut TripleLayerQuadAllocator,
        stable_range: Range<StableRowIndex>,
        left_pixel_x: f32,
        top_pixel_y: f32,
        pane_cols: usize,
        background: LinearRgba,
        scan_lines: &[LcarsMarkdownLine],
    ) -> anyhow::Result<()> {
        let scan = LcarsMarkdownScanner::scan(scan_lines, stable_range.clone());
        self.paint_lcars_markdown_suppressed_rows(
            pos,
            layers,
            stable_range.clone(),
            left_pixel_x,
            top_pixel_y,
            pane_cols,
            background,
            scan.suppressed_rows.iter().copied(),
        )?;
        self.paint_lcars_markdown_dataview_blocks(
            pos,
            layers,
            stable_range.clone(),
            left_pixel_x,
            top_pixel_y,
            pane_cols,
            &scan.dataview_blocks,
        )?;

        let markers = scan.markers.into_iter().map(|marker| {
            (
                marker.stable_row,
                OwtTranscriptMarkerVisual::for_lcars_markdown(marker.kind),
            )
        });
        self.paint_owt_marker_chrome(
            pos,
            layers,
            stable_range,
            left_pixel_x,
            top_pixel_y,
            markers,
        )
    }

    fn paint_lcars_markdown_suppressed_rows(
        &mut self,
        pos: &PositionedPane,
        layers: &mut TripleLayerQuadAllocator,
        stable_range: Range<StableRowIndex>,
        left_pixel_x: f32,
        top_pixel_y: f32,
        pane_cols: usize,
        background: LinearRgba,
        rows: impl IntoIterator<Item = StableRowIndex>,
    ) -> anyhow::Result<()> {
        let cell_width = self.render_metrics.cell_size.width as f32;
        let cell_height = self.render_metrics.cell_size.height as f32;
        let width = pane_cols as f32 * cell_width;

        for stable_row in rows {
            if stable_row < stable_range.start || stable_row >= stable_range.end {
                continue;
            }

            let row_index = (stable_row - stable_range.start) as usize;
            let y = top_pixel_y + (row_index + pos.top) as f32 * cell_height;
            self.filled_rectangle(
                layers,
                2,
                euclid::rect(left_pixel_x, y, width, cell_height),
                background,
            )?;
        }

        Ok(())
    }

    fn paint_lcars_markdown_dataview_blocks(
        &mut self,
        pos: &PositionedPane,
        layers: &mut TripleLayerQuadAllocator,
        stable_range: Range<StableRowIndex>,
        left_pixel_x: f32,
        top_pixel_y: f32,
        pane_cols: usize,
        blocks: &[LcarsMarkdownDataViewBlock],
    ) -> anyhow::Result<()> {
        let cell_width = self.render_metrics.cell_size.width as f32;
        let cell_height = self.render_metrics.cell_size.height as f32;

        for block in blocks {
            if block.start_row < stable_range.start || block.start_row >= stable_range.end {
                continue;
            }

            let block_rows = (block.end_row - block.start_row + 1).max(1) as usize;
            let Some(layout) = lcars_markdown_dataview_layout(block, pane_cols, block_rows) else {
                continue;
            };

            let row_index = (block.start_row - stable_range.start) as usize;
            let x = left_pixel_x;
            let y = top_pixel_y + (row_index + pos.top) as f32 * cell_height;
            let height = layout.visible_rows as f32 * cell_height;
            let bay_width = layout.bay_cols as f32 * cell_width;
            let rail_width = layout.rail_cols as f32 * cell_width;
            let accent = owt_marker_color(255, 156, 0, 0.88);
            let secondary = owt_marker_color(102, 153, 204, 0.78);
            let violet = owt_marker_color(204, 153, 255, 0.68);
            let black = owt_marker_color(0, 0, 0, 0.96);

            self.filled_rectangle(layers, 2, euclid::rect(x, y, bay_width, height), black)?;
            self.filled_rectangle(layers, 2, euclid::rect(x, y, rail_width, height), accent)?;
            self.filled_rectangle(
                layers,
                2,
                euclid::rect(x + rail_width + 2.0, y, bay_width - rail_width - 2.0, 4.0),
                accent,
            )?;
            self.filled_rectangle(
                layers,
                2,
                euclid::rect(
                    x + rail_width + 2.0,
                    y + cell_height * 1.82,
                    bay_width - rail_width - 2.0,
                    3.0,
                ),
                secondary,
            )?;
            self.filled_rectangle(
                layers,
                2,
                euclid::rect(
                    x + rail_width + 2.0,
                    y + height - 4.0,
                    bay_width * 0.48,
                    4.0,
                ),
                violet,
            )?;

            let text_x = x + (layout.rail_cols + 1) as f32 * cell_width;
            let mut lines = block.dataview_lines(layout.text_cols, layout.visible_rows);
            for (offset, line) in lines.drain(..).enumerate() {
                let row_y = y + (offset as f32 * cell_height);
                self.paint_lcars_markdown_text(
                    layers,
                    text_x,
                    row_y,
                    layout.text_cols,
                    &line.text,
                    line.fg,
                    line.bold,
                )?;
            }
        }

        Ok(())
    }

    fn paint_lcars_markdown_text(
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
        let text = fit_lcars_markdown_text(text, max_cols);
        let line = Line::from_text(&text, &attrs, 0, None);
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
        let mut text_layers = HeapQuadAllocator::default();

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
            &mut TripleLayerQuadAllocator::Heap(&mut text_layers),
        )?;

        text_layers.apply_layer_to(1, 2, layers)?;

        Ok(())
    }

    fn paint_owt_marker_chrome(
        &mut self,
        pos: &PositionedPane,
        layers: &mut TripleLayerQuadAllocator,
        stable_range: Range<StableRowIndex>,
        left_pixel_x: f32,
        top_pixel_y: f32,
        markers: impl IntoIterator<Item = (StableRowIndex, OwtTranscriptMarkerVisual)>,
    ) -> anyhow::Result<()> {
        let cell_width = self.render_metrics.cell_size.width as f32;
        let cell_height = self.render_metrics.cell_size.height as f32;
        let gutter_width = (cell_width * 0.36).max(4.0);
        if left_pixel_x < gutter_width + 2.0 {
            return Ok(());
        }
        let gutter_x = left_pixel_x - gutter_width - 2.0;

        for (stable_row, visual) in markers {
            if stable_row < stable_range.start || stable_row >= stable_range.end {
                continue;
            }

            let row_index = (stable_row - stable_range.start) as usize;
            let y = top_pixel_y + (row_index + pos.top) as f32 * cell_height;
            let marker_y = y + (cell_height * 0.18);
            let marker_h = (cell_height * 0.64).max(4.0);

            self.filled_rectangle(
                layers,
                2,
                euclid::rect(gutter_x, marker_y, gutter_width, marker_h),
                visual.primary,
            )?;

            if visual.section_rule {
                self.filled_rectangle(
                    layers,
                    2,
                    euclid::rect(gutter_x, y + cell_height - 3.0, gutter_width, 2.0),
                    visual.secondary,
                )?;
                self.filled_rectangle(
                    layers,
                    2,
                    euclid::rect(gutter_x, y + 2.0, gutter_width, 3.0),
                    visual.secondary,
                )?;
            } else if visual.strong {
                self.filled_rectangle(
                    layers,
                    2,
                    euclid::rect(gutter_x, marker_y, gutter_width, 3.0),
                    visual.secondary,
                )?;
                self.filled_rectangle(
                    layers,
                    2,
                    euclid::rect(gutter_x, marker_y + marker_h - 3.0, gutter_width, 3.0),
                    visual.secondary,
                )?;
            }
        }

        Ok(())
    }

    pub fn build_pane(&mut self, pos: &PositionedPane) -> anyhow::Result<ComputedElement> {
        // First compute the bounds for the pane background

        let cell_width = self.render_metrics.cell_size.width as f32;
        let cell_height = self.render_metrics.cell_size.height as f32;
        let (padding_left, padding_top) = self.padding_left_top();
        let tab_bar_height = if self.show_tab_bar {
            self.tab_bar_pixel_height()?
        } else {
            0.
        };
        let (top_bar_height, _bottom_bar_height) = if self.config.tab_bar_at_bottom {
            (0.0, tab_bar_height)
        } else {
            (tab_bar_height, 0.0)
        };

        let border = self.get_os_border();
        let top_pixel_y = top_bar_height + padding_top + border.top.get() as f32;

        // We want to fill out to the edges of the splits
        let (x, width_delta) = if pos.left == 0 {
            (
                0.,
                padding_left + border.left.get() as f32 + (cell_width / 2.0),
            )
        } else {
            (
                padding_left + border.left.get() as f32 - (cell_width / 2.0)
                    + (pos.left as f32 * cell_width),
                cell_width,
            )
        };

        let (y, height_delta) = if pos.top == 0 {
            (
                (top_pixel_y - padding_top),
                padding_top + (cell_height / 2.0),
            )
        } else {
            (
                top_pixel_y + (pos.top as f32 * cell_height) - (cell_height / 2.0),
                cell_height,
            )
        };

        let background_rect = euclid::rect(
            x,
            y,
            // Go all the way to the right edge if we're right-most
            if pos.left + pos.width >= self.terminal_size.cols as usize {
                self.dimensions.pixel_width as f32 - x
            } else {
                (pos.width as f32 * cell_width) + width_delta
            },
            // Go all the way to the bottom if we're bottom-most
            if pos.top + pos.height >= self.terminal_size.rows as usize {
                self.dimensions.pixel_height as f32 - y
            } else {
                (pos.height as f32 * cell_height) + height_delta as f32
            },
        );

        // Bounds for the terminal cells
        let content_rect = euclid::rect(
            padding_left + border.left.get() as f32 - (cell_width / 2.0)
                + (pos.left as f32 * cell_width),
            top_pixel_y + (pos.top as f32 * cell_height) - (cell_height / 2.0),
            pos.width as f32 * cell_width,
            pos.height as f32 * cell_height,
        );

        let palette = pos.pane.palette();

        // TODO: visual bell background layer
        // TODO: scrollbar

        Ok(ComputedElement {
            item_type: None,
            zindex: 0,
            bounds: background_rect,
            border: PixelDimension::default(),
            border_rect: background_rect,
            border_corners: None,
            colors: ElementColors {
                border: BorderColor::default(),
                bg: if self.window_background.is_empty() {
                    palette
                        .background
                        .to_linear()
                        .mul_alpha(self.config.window_background_opacity)
                        .into()
                } else {
                    InheritableColor::Inherited
                },
                text: InheritableColor::Inherited,
            },
            hover_colors: None,
            padding: background_rect,
            content_rect,
            baseline: 1.0,
            content: ComputedElementContent::Children(vec![]),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct LcarsMarkdownLine {
    stable_row: StableRowIndex,
    text: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct LcarsMarkdownMarker {
    stable_row: StableRowIndex,
    kind: LcarsMarkdownKind,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
struct LcarsMarkdownScan {
    markers: Vec<LcarsMarkdownMarker>,
    suppressed_rows: Vec<StableRowIndex>,
    dataview_blocks: Vec<LcarsMarkdownDataViewBlock>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct LcarsMarkdownDataViewBlock {
    start_row: StableRowIndex,
    end_row: StableRowIndex,
    document: LcarsDataViewDocument,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct LcarsMarkdownDataViewCandidate {
    start_row: StableRowIndex,
    rows: Vec<StableRowIndex>,
    json_lines: Vec<String>,
}

#[derive(Debug, Clone)]
struct LcarsMarkdownRenderedLine {
    text: String,
    fg: RgbColor,
    bold: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct LcarsMarkdownDataViewLayout {
    bay_cols: usize,
    rail_cols: usize,
    text_cols: usize,
    visible_rows: usize,
}

impl LcarsMarkdownDataViewBlock {
    fn preferred_text_cols(&self, max_cols: usize) -> usize {
        let mut width = 0;
        for line in self.dataview_lines(max_cols, 4) {
            width = width.max(line.text.chars().count());
        }
        width.min(max_cols)
    }

    fn dataview_lines(
        &self,
        max_cols: usize,
        available_rows: usize,
    ) -> Vec<LcarsMarkdownRenderedLine> {
        if available_rows == 0 {
            return vec![];
        }

        let mut lines = Vec::new();
        lines.push(LcarsMarkdownRenderedLine {
            text: format!(
                "LCARS DATAVIEW  {}  ROWS {}  COLS {}",
                self.document.title,
                self.document.rows.len(),
                self.document.columns.len()
            ),
            fg: RgbColor::new_8bpc(255, 204, 102),
            bold: true,
        });
        if available_rows == 1 {
            return lines;
        }

        lines.push(LcarsMarkdownRenderedLine {
            text: format!("{}  / LOCAL / SAFE MARKDOWN", self.document.id),
            fg: RgbColor::new_8bpc(153, 204, 255),
            bold: false,
        });
        if available_rows == 2 {
            return lines;
        }

        let table_rows = available_rows
            .saturating_sub(3)
            .min(self.document.rows.len());
        let table_columns = self.document.columns.len().min(6);
        let widths = dataview_column_widths(&self.document, table_columns, max_cols);
        lines.push(LcarsMarkdownRenderedLine {
            text: format_dataview_cells(&self.document.columns[..table_columns], &widths),
            fg: RgbColor::new_8bpc(255, 153, 102),
            bold: true,
        });

        for row in self.document.rows.iter().take(table_rows) {
            let mut cells = Vec::new();
            for index in 0..table_columns {
                cells.push(row.get(index).map(String::as_str).unwrap_or(""));
            }
            lines.push(LcarsMarkdownRenderedLine {
                text: format_dataview_cells(&cells, &widths),
                fg: RgbColor::new_8bpc(220, 220, 220),
                bold: false,
            });
        }

        if self.document.rows.len() > table_rows && lines.len() < available_rows {
            lines.push(LcarsMarkdownRenderedLine {
                text: format!(
                    "+{} rows hidden in inline preview",
                    self.document.rows.len() - table_rows
                ),
                fg: RgbColor::new_8bpc(204, 153, 255),
                bold: false,
            });
        }

        lines
    }
}

fn lcars_markdown_dataview_layout(
    block: &LcarsMarkdownDataViewBlock,
    pane_cols: usize,
    block_rows: usize,
) -> Option<LcarsMarkdownDataViewLayout> {
    let visible_rows = block_rows.min(10);
    if visible_rows < 3 || pane_cols < 12 {
        return None;
    }

    let rail_cols = if pane_cols >= 24 { 2 } else { 1 };
    let gap_cols = 2;
    let max_text_cols = pane_cols.saturating_sub(rail_cols + gap_cols);
    if max_text_cols < 8 {
        return None;
    }

    let min_text_cols = 28.min(max_text_cols);
    let preferred_text_cols = block
        .preferred_text_cols(max_text_cols)
        .max(min_text_cols)
        .min(max_text_cols);
    let bay_cols = (rail_cols + gap_cols + preferred_text_cols).min(pane_cols);

    Some(LcarsMarkdownDataViewLayout {
        bay_cols,
        rail_cols,
        text_cols: preferred_text_cols,
        visible_rows,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LcarsMarkdownKind {
    Section { level: u8 },
    Task,
    Finding,
    Source,
    Assessment,
    Warning,
    Error,
    Code,
    Divider,
    Table,
}

struct LcarsMarkdownScanner;

impl LcarsMarkdownScanner {
    fn scan(
        lines: &[LcarsMarkdownLine],
        visible_range: Range<StableRowIndex>,
    ) -> LcarsMarkdownScan {
        let mut enabled = false;
        let mut in_code_block = false;
        let mut dataview_candidate: Option<LcarsMarkdownDataViewCandidate> = None;
        let mut pending_hint = None;
        let mut scan = LcarsMarkdownScan::default();

        for line in lines {
            let trimmed = line.text.trim();

            if is_lcars_markdown_sentinel(trimmed) {
                enabled = true;
                if visible_range.contains(&line.stable_row) {
                    scan.suppressed_rows.push(line.stable_row);
                }
                continue;
            }

            if !enabled {
                continue;
            }

            if let Some(candidate) = dataview_candidate.as_mut() {
                candidate.rows.push(line.stable_row);
                if markdown_code_fence_info(trimmed).is_some() {
                    let candidate = dataview_candidate.take().expect("candidate is set");
                    in_code_block = false;
                    scan.finish_dataview_candidate(candidate, &visible_range);
                } else {
                    candidate.json_lines.push(line.text.clone());
                }
                continue;
            }

            if let Some(hint) = parse_lcars_markdown_hint(trimmed) {
                if hint.is_some() && visible_range.contains(&line.stable_row) {
                    scan.suppressed_rows.push(line.stable_row);
                }
                pending_hint = hint;
                continue;
            }

            if trimmed.is_empty() {
                continue;
            }

            let mut kind = if let Some(fence_info) = markdown_code_fence_info(trimmed) {
                let opening = !in_code_block;
                if opening && is_lcars_dataview_fence_info(fence_info) {
                    dataview_candidate = Some(LcarsMarkdownDataViewCandidate {
                        start_row: line.stable_row,
                        rows: vec![line.stable_row],
                        json_lines: vec![],
                    });
                    pending_hint = None;
                    in_code_block = true;
                    continue;
                }
                in_code_block = !in_code_block;
                Some(LcarsMarkdownKind::Code)
            } else if in_code_block {
                Some(LcarsMarkdownKind::Code)
            } else {
                classify_lcars_markdown_line(trimmed)
            };

            if let Some(hint) = pending_hint.take() {
                kind = Some(hint);
            }

            if let Some(kind) = kind {
                if visible_range.contains(&line.stable_row) {
                    scan.markers.push(LcarsMarkdownMarker {
                        stable_row: line.stable_row,
                        kind,
                    });
                }
            }
        }

        if let Some(candidate) = dataview_candidate.take() {
            scan.mark_invalid_dataview_candidate(candidate, &visible_range);
        }

        scan
    }
}

impl LcarsMarkdownScan {
    fn finish_dataview_candidate(
        &mut self,
        candidate: LcarsMarkdownDataViewCandidate,
        visible_range: &Range<StableRowIndex>,
    ) {
        let payload = candidate.json_lines.join("\n");
        match validate_dataview_document(&payload) {
            Ok(document) => {
                let table_rows = self.take_adjacent_fallback_table_rows(candidate.start_row);
                let start_row = table_rows.first().copied().unwrap_or(candidate.start_row);
                for row in &table_rows {
                    if visible_range.contains(row) {
                        self.suppressed_rows.push(*row);
                    }
                }
                for row in &candidate.rows {
                    if visible_range.contains(row) {
                        self.suppressed_rows.push(*row);
                    }
                }
                let end_row = *candidate.rows.last().unwrap_or(&candidate.start_row);
                if end_row >= visible_range.start && start_row < visible_range.end {
                    self.dataview_blocks.push(LcarsMarkdownDataViewBlock {
                        start_row,
                        end_row,
                        document,
                    });
                }
            }
            Err(err) => {
                log::warn!(
                    "Invalid LCARSMarkdown lcars-dataview block at row {}: {err:#}",
                    candidate.start_row
                );
                self.mark_invalid_dataview_candidate(candidate, visible_range);
            }
        }
    }

    fn take_adjacent_fallback_table_rows(
        &mut self,
        dataview_start_row: StableRowIndex,
    ) -> Vec<StableRowIndex> {
        let Some(last_marker) = self.markers.last() else {
            return vec![];
        };
        if last_marker.kind != LcarsMarkdownKind::Table
            || dataview_start_row < last_marker.stable_row
            || dataview_start_row - last_marker.stable_row > 3
        {
            return vec![];
        }

        let mut table_rows = Vec::new();
        while self
            .markers
            .last()
            .is_some_and(|marker| marker.kind == LcarsMarkdownKind::Table)
        {
            let marker = self.markers.pop().expect("marker exists");
            table_rows.push(marker.stable_row);
        }
        table_rows.reverse();
        table_rows
    }

    fn mark_invalid_dataview_candidate(
        &mut self,
        candidate: LcarsMarkdownDataViewCandidate,
        visible_range: &Range<StableRowIndex>,
    ) {
        for row in candidate.rows {
            if visible_range.contains(&row) {
                self.markers.push(LcarsMarkdownMarker {
                    stable_row: row,
                    kind: if row == candidate.start_row {
                        LcarsMarkdownKind::Warning
                    } else {
                        LcarsMarkdownKind::Code
                    },
                });
            }
        }
    }
}

fn collect_lcars_markdown_lines(
    pos: &PositionedPane,
    stable_range: Range<StableRowIndex>,
) -> Vec<LcarsMarkdownLine> {
    struct Collector {
        lines: Vec<LcarsMarkdownLine>,
    }

    impl WithPaneLines for Collector {
        fn with_lines_mut(&mut self, stable_top: StableRowIndex, lines: &mut [&mut Line]) {
            for (line_idx, line) in lines.iter().enumerate() {
                let stable_row = stable_top + line_idx as StableRowIndex;
                self.lines.push(LcarsMarkdownLine {
                    stable_row,
                    text: line.as_str().into_owned(),
                });
            }
        }
    }

    let mut collector = Collector { lines: vec![] };
    pos.pane.with_lines_mut(stable_range, &mut collector);
    collector.lines
}

fn is_lcars_markdown_sentinel(trimmed: &str) -> bool {
    trimmed.starts_with("<!--") && trimmed.contains("owt:lcars-md")
}

fn parse_lcars_markdown_hint(trimmed: &str) -> Option<Option<LcarsMarkdownKind>> {
    let body = trimmed.strip_prefix("<!--")?.strip_suffix("-->")?.trim();
    let hint = body.strip_prefix("lcars:")?.trim();
    let mut parts = hint.split_whitespace();
    let name = parts.next()?.to_ascii_lowercase();
    let severity = hint
        .split_whitespace()
        .find_map(|part| part.strip_prefix("severity="))
        .map(|value| {
            value
                .trim_matches('"')
                .trim_matches('\'')
                .to_ascii_lowercase()
        });

    let kind = match name.as_str() {
        "section" => LcarsMarkdownKind::Section { level: 2 },
        "task" => LcarsMarkdownKind::Task,
        "finding" => match severity.as_deref() {
            Some("warn") | Some("warning") => LcarsMarkdownKind::Warning,
            Some("error") | Some("err") => LcarsMarkdownKind::Error,
            _ => LcarsMarkdownKind::Finding,
        },
        "source" => LcarsMarkdownKind::Source,
        "assessment" => LcarsMarkdownKind::Assessment,
        "warning" | "warn" => LcarsMarkdownKind::Warning,
        "error" | "err" => LcarsMarkdownKind::Error,
        "code" => LcarsMarkdownKind::Code,
        "table" => LcarsMarkdownKind::Table,
        "divider" | "rule" => LcarsMarkdownKind::Divider,
        _ => return Some(None),
    };

    Some(Some(kind))
}

fn classify_lcars_markdown_line(trimmed: &str) -> Option<LcarsMarkdownKind> {
    if let Some(level) = markdown_heading_level(trimmed) {
        return Some(LcarsMarkdownKind::Section { level });
    }

    if is_markdown_task_item(trimmed) {
        return Some(LcarsMarkdownKind::Task);
    }

    if trimmed.starts_with("> [!WARNING]") || trimmed.starts_with("> [!WARN]") {
        return Some(LcarsMarkdownKind::Warning);
    }

    if trimmed.starts_with("> [!ERROR]") || trimmed.starts_with("> [!ERR]") {
        return Some(LcarsMarkdownKind::Error);
    }

    if trimmed.starts_with('>') {
        return Some(LcarsMarkdownKind::Source);
    }

    if is_markdown_rule(trimmed) {
        return Some(LcarsMarkdownKind::Divider);
    }

    if is_markdown_table_row(trimmed) {
        return Some(LcarsMarkdownKind::Table);
    }

    None
}

fn markdown_heading_level(trimmed: &str) -> Option<u8> {
    let level = trimmed.chars().take_while(|ch| *ch == '#').count();
    if (1..=6).contains(&level) && trimmed.chars().nth(level).is_some_and(char::is_whitespace) {
        Some(level as u8)
    } else {
        None
    }
}

fn is_markdown_task_item(trimmed: &str) -> bool {
    let lower = trimmed.to_ascii_lowercase();
    ["- [ ]", "- [x]", "* [ ]", "* [x]", "+ [ ]", "+ [x]"]
        .iter()
        .any(|prefix| lower.starts_with(prefix))
}

fn markdown_code_fence_info(trimmed: &str) -> Option<&str> {
    trimmed
        .strip_prefix("```")
        .or_else(|| trimmed.strip_prefix("~~~"))
        .map(str::trim)
}

fn is_lcars_dataview_fence_info(fence_info: &str) -> bool {
    fence_info
        .split_whitespace()
        .next()
        .is_some_and(|language| language.eq_ignore_ascii_case("lcars-dataview"))
}

fn dataview_column_widths(
    document: &LcarsDataViewDocument,
    table_columns: usize,
    max_cols: usize,
) -> Vec<usize> {
    if table_columns == 0 {
        return vec![];
    }

    let gap_total = table_columns.saturating_sub(1) * 2;
    let usable = max_cols.saturating_sub(gap_total).max(table_columns);
    let per_column_cap = (usable / table_columns).clamp(4, 22);
    let mut widths = Vec::with_capacity(table_columns);

    for index in 0..table_columns {
        let mut width = document
            .columns
            .get(index)
            .map(|column| column.chars().count())
            .unwrap_or(4)
            .max(4);
        for row in &document.rows {
            if let Some(cell) = row.get(index) {
                width = width.max(cell.chars().count());
            }
        }
        widths.push(width.min(per_column_cap));
    }

    widths
}

fn format_dataview_cells<S: AsRef<str>>(cells: &[S], widths: &[usize]) -> String {
    let mut parts = Vec::with_capacity(widths.len());
    for (index, width) in widths.iter().copied().enumerate() {
        let value = cells
            .get(index)
            .map(AsRef::as_ref)
            .map(|text| fit_lcars_markdown_text(text, width))
            .unwrap_or_default();
        parts.push(format!("{value:<width$}"));
    }
    parts.join("  ")
}

fn fit_lcars_markdown_text(text: &str, max_cols: usize) -> String {
    if max_cols == 0 {
        return String::new();
    }

    let mut chars = text.chars();
    let mut fitted = String::new();
    for _ in 0..max_cols {
        let Some(ch) = chars.next() else {
            return text.to_string();
        };
        fitted.push(ch);
    }

    if chars.next().is_some() {
        if max_cols == 1 {
            "~".to_string()
        } else {
            fitted.pop();
            fitted.push('~');
            fitted
        }
    } else {
        text.to_string()
    }
}

fn is_markdown_rule(trimmed: &str) -> bool {
    let mut chars = trimmed.chars().filter(|ch| !ch.is_whitespace());
    let Some(first) = chars.next() else {
        return false;
    };
    if !matches!(first, '-' | '*' | '_') {
        return false;
    }
    let mut count = 1;
    for ch in chars {
        if ch != first {
            return false;
        }
        count += 1;
    }
    count >= 3
}

fn is_markdown_table_row(trimmed: &str) -> bool {
    trimmed.matches('|').count() >= 2
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct OwtTranscriptMarkerVisual {
    primary: LinearRgba,
    secondary: LinearRgba,
    section_rule: bool,
    strong: bool,
}

impl OwtTranscriptMarkerVisual {
    fn for_event(event: &OwtTranscriptEvent) -> Option<Self> {
        match event.event.as_str() {
            "owt.section" => Some(Self {
                primary: owt_marker_color(255, 156, 0, 0.95),
                secondary: owt_marker_color(255, 204, 102, 0.78),
                section_rule: true,
                strong: false,
            }),
            "owt.task" => Some(Self {
                primary: owt_marker_color(204, 153, 255, 0.86),
                secondary: owt_marker_color(255, 204, 102, 0.70),
                section_rule: false,
                strong: false,
            }),
            "owt.finding" | "owt.assessment" => Some(Self {
                primary: owt_marker_color(102, 153, 204, 0.88),
                secondary: owt_marker_color(204, 153, 255, 0.70),
                section_rule: false,
                strong: false,
            }),
            "owt.source" => Some(Self {
                primary: owt_marker_color(255, 204, 102, 0.78),
                secondary: owt_marker_color(102, 153, 204, 0.64),
                section_rule: false,
                strong: false,
            }),
            "owt.warning" | "owt.error" => Some(Self {
                primary: owt_marker_color(255, 102, 102, 0.95),
                secondary: owt_marker_color(255, 204, 102, 0.82),
                section_rule: false,
                strong: true,
            }),
            _ => None,
        }
    }

    fn for_lcars_markdown(kind: LcarsMarkdownKind) -> Self {
        match kind {
            LcarsMarkdownKind::Section { level } => Self {
                primary: if level <= 2 {
                    owt_marker_color(255, 156, 0, 0.88)
                } else {
                    owt_marker_color(255, 204, 102, 0.80)
                },
                secondary: owt_marker_color(255, 204, 102, 0.62),
                section_rule: level <= 2,
                strong: false,
            },
            LcarsMarkdownKind::Task => Self {
                primary: owt_marker_color(204, 153, 255, 0.76),
                secondary: owt_marker_color(255, 204, 102, 0.62),
                section_rule: false,
                strong: false,
            },
            LcarsMarkdownKind::Finding | LcarsMarkdownKind::Assessment => Self {
                primary: owt_marker_color(102, 153, 204, 0.76),
                secondary: owt_marker_color(204, 153, 255, 0.58),
                section_rule: false,
                strong: false,
            },
            LcarsMarkdownKind::Source => Self {
                primary: owt_marker_color(255, 204, 102, 0.68),
                secondary: owt_marker_color(102, 153, 204, 0.56),
                section_rule: false,
                strong: false,
            },
            LcarsMarkdownKind::Warning | LcarsMarkdownKind::Error => Self {
                primary: owt_marker_color(255, 102, 102, 0.88),
                secondary: owt_marker_color(255, 204, 102, 0.74),
                section_rule: false,
                strong: true,
            },
            LcarsMarkdownKind::Code => Self {
                primary: owt_marker_color(204, 153, 255, 0.64),
                secondary: owt_marker_color(102, 153, 204, 0.54),
                section_rule: false,
                strong: true,
            },
            LcarsMarkdownKind::Divider => Self {
                primary: owt_marker_color(255, 156, 0, 0.72),
                secondary: owt_marker_color(255, 204, 102, 0.62),
                section_rule: true,
                strong: false,
            },
            LcarsMarkdownKind::Table => Self {
                primary: owt_marker_color(102, 153, 204, 0.72),
                secondary: owt_marker_color(255, 204, 102, 0.56),
                section_rule: false,
                strong: true,
            },
        }
    }
}

fn owt_marker_color(red: u8, green: u8, blue: u8, alpha: f32) -> LinearRgba {
    let red = red as f32 / 255.0;
    let green = green as f32 / 255.0;
    let blue = blue as f32 / 255.0;
    LinearRgba::with_components(red, green, blue, alpha)
}

#[cfg(test)]
mod owt_marker_tests {
    use super::*;

    fn event(name: &str) -> OwtTranscriptEvent {
        OwtTranscriptEvent {
            row: 0,
            col: 0,
            seqno: 0,
            event: name.to_string(),
            payload_json: r#"{"version":1}"#.to_string(),
        }
    }

    fn lcars_line(stable_row: StableRowIndex, text: &str) -> LcarsMarkdownLine {
        LcarsMarkdownLine {
            stable_row,
            text: text.to_string(),
        }
    }

    fn lcars_markdown_kinds(
        lines: &[LcarsMarkdownLine],
    ) -> Vec<(StableRowIndex, LcarsMarkdownKind)> {
        LcarsMarkdownScanner::scan(lines, 0..100)
            .markers
            .into_iter()
            .map(|marker| (marker.stable_row, marker.kind))
            .collect()
    }

    fn lcars_markdown_suppressed_rows(lines: &[LcarsMarkdownLine]) -> Vec<StableRowIndex> {
        LcarsMarkdownScanner::scan(lines, 0..100).suppressed_rows
    }

    fn lcars_markdown_dataview_titles(lines: &[LcarsMarkdownLine]) -> Vec<String> {
        LcarsMarkdownScanner::scan(lines, 0..100)
            .dataview_blocks
            .into_iter()
            .map(|block| block.document.title)
            .collect()
    }

    fn lcars_markdown_dataview_ranges(
        lines: &[LcarsMarkdownLine],
    ) -> Vec<(StableRowIndex, StableRowIndex)> {
        LcarsMarkdownScanner::scan(lines, 0..100)
            .dataview_blocks
            .into_iter()
            .map(|block| (block.start_row, block.end_row))
            .collect()
    }

    fn lcars_markdown_dataview_layouts(
        lines: &[LcarsMarkdownLine],
        pane_cols: usize,
    ) -> Vec<LcarsMarkdownDataViewLayout> {
        LcarsMarkdownScanner::scan(lines, 0..100)
            .dataview_blocks
            .into_iter()
            .filter_map(|block| {
                let block_rows = (block.end_row - block.start_row + 1).max(1) as usize;
                lcars_markdown_dataview_layout(&block, pane_cols, block_rows)
            })
            .collect()
    }

    #[test]
    fn owt_marker_visuals_cover_initial_events() {
        for name in [
            "owt.section",
            "owt.task",
            "owt.finding",
            "owt.source",
            "owt.assessment",
            "owt.warning",
            "owt.error",
        ] {
            assert!(OwtTranscriptMarkerVisual::for_event(&event(name)).is_some());
        }
    }

    #[test]
    fn owt_marker_visuals_ignore_unknown_events() {
        assert!(OwtTranscriptMarkerVisual::for_event(&event("owt.future")).is_none());
        assert!(OwtTranscriptMarkerVisual::for_event(&event("not_owt")).is_none());
    }

    #[test]
    fn lcars_markdown_requires_sentinel() {
        let lines = [
            lcars_line(0, "# Report"),
            lcars_line(1, "- [x] done"),
            lcars_line(2, "> quote"),
        ];

        assert_eq!(
            LcarsMarkdownScanner::scan(&lines, 0..10),
            LcarsMarkdownScan::default()
        );
    }

    #[test]
    fn lcars_markdown_classifies_initial_shapes() {
        let lines = [
            lcars_line(0, "<!-- owt:lcars-md v=1 -->"),
            lcars_line(1, "# Report"),
            lcars_line(2, "- [x] done"),
            lcars_line(3, "> source"),
            lcars_line(4, "> [!WARNING] warning"),
            lcars_line(5, "---"),
            lcars_line(6, "| key | value |"),
        ];

        assert_eq!(
            lcars_markdown_kinds(&lines),
            vec![
                (1, LcarsMarkdownKind::Section { level: 1 }),
                (2, LcarsMarkdownKind::Task),
                (3, LcarsMarkdownKind::Source),
                (4, LcarsMarkdownKind::Warning),
                (5, LcarsMarkdownKind::Divider),
                (6, LcarsMarkdownKind::Table),
            ]
        );
    }

    #[test]
    fn lcars_markdown_hint_applies_to_next_block() {
        let lines = [
            lcars_line(0, "<!-- owt:lcars-md v=1 -->"),
            lcars_line(1, "<!-- lcars: finding severity=warn -->"),
            lcars_line(2, "plain finding text"),
            lcars_line(3, "<!-- lcars: assessment -->"),
            lcars_line(4, "assessment text"),
        ];

        assert_eq!(
            lcars_markdown_kinds(&lines),
            vec![
                (2, LcarsMarkdownKind::Warning),
                (4, LcarsMarkdownKind::Assessment),
            ]
        );
        assert_eq!(lcars_markdown_suppressed_rows(&lines), vec![0, 1, 3]);
    }

    #[test]
    fn lcars_markdown_ignores_unknown_hint() {
        let lines = [
            lcars_line(0, "<!-- owt:lcars-md v=1 -->"),
            lcars_line(1, "<!-- lcars: future-widget -->"),
            lcars_line(2, "plain text"),
            lcars_line(3, "## Known heading"),
        ];

        assert_eq!(
            lcars_markdown_kinds(&lines),
            vec![(3, LcarsMarkdownKind::Section { level: 2 })]
        );
        assert_eq!(lcars_markdown_suppressed_rows(&lines), vec![0]);
    }

    #[test]
    fn lcars_markdown_suppresses_sentinel_and_known_hints() {
        let lines = [
            lcars_line(0, "<!-- owt:lcars-md v=1 -->"),
            lcars_line(1, "<!-- lcars: source -->"),
            lcars_line(2, "> source text"),
            lcars_line(3, "<!-- lcars: future-widget -->"),
            lcars_line(4, "## Known heading"),
        ];

        assert_eq!(lcars_markdown_suppressed_rows(&lines), vec![0, 1]);
    }

    #[test]
    fn lcars_markdown_tracks_code_fences() {
        let lines = [
            lcars_line(0, "<!-- owt:lcars-md v=1 -->"),
            lcars_line(1, "```sh"),
            lcars_line(2, "echo bridge"),
            lcars_line(3, "```"),
        ];

        assert_eq!(
            lcars_markdown_kinds(&lines),
            vec![
                (1, LcarsMarkdownKind::Code),
                (2, LcarsMarkdownKind::Code),
                (3, LcarsMarkdownKind::Code),
            ]
        );
    }

    #[test]
    fn lcars_markdown_marks_dataview_fences_inline() {
        let lines = [
            lcars_line(0, "<!-- owt:lcars-md v=1 -->"),
            lcars_line(1, "```lcars-dataview"),
            lcars_line(
                2,
                r#"{"version":1,"kind":"owt.lcars.dataview","id":"fleet.demo","title":"Fleet Demo","columns":["Ship","Registry"],"rows":[["Enterprise","1701"]]}"#,
            ),
            lcars_line(3, "```"),
            lcars_line(4, "```sh"),
            lcars_line(5, "echo still-code"),
            lcars_line(6, "```"),
        ];

        assert_eq!(
            lcars_markdown_kinds(&lines),
            vec![
                (4, LcarsMarkdownKind::Code),
                (5, LcarsMarkdownKind::Code),
                (6, LcarsMarkdownKind::Code),
            ]
        );
        assert_eq!(lcars_markdown_suppressed_rows(&lines), vec![0, 1, 2, 3]);
        assert_eq!(lcars_markdown_dataview_titles(&lines), vec!["Fleet Demo"]);
    }

    #[test]
    fn lcars_markdown_dataview_replaces_adjacent_fallback_table() {
        let lines = [
            lcars_line(0, "<!-- owt:lcars-md v=1 -->"),
            lcars_line(1, "| Ship | Registry |"),
            lcars_line(2, "| Enterprise | 1701 |"),
            lcars_line(3, ""),
            lcars_line(4, "```lcars-dataview"),
            lcars_line(
                5,
                r#"{"version":1,"kind":"owt.lcars.dataview","id":"fleet.demo","title":"Fleet Demo","columns":["Ship","Registry"],"rows":[["Enterprise","1701"]]}"#,
            ),
            lcars_line(6, "```"),
        ];

        assert!(lcars_markdown_kinds(&lines).is_empty());
        assert_eq!(
            lcars_markdown_suppressed_rows(&lines),
            vec![0, 1, 2, 4, 5, 6]
        );
        assert_eq!(lcars_markdown_dataview_ranges(&lines), vec![(1, 6)]);
    }

    #[test]
    fn lcars_markdown_dataview_layout_is_content_aware() {
        let lines = [
            lcars_line(0, "<!-- owt:lcars-md v=1 -->"),
            lcars_line(1, "| Ship | Registry | Class | Crew | Status |"),
            lcars_line(2, "| Enterprise | 1701 | Constitution | 430 | active |"),
            lcars_line(3, ""),
            lcars_line(4, "```lcars-dataview"),
            lcars_line(
                5,
                r#"{"version":1,"kind":"owt.lcars.dataview","id":"fleet.demo.inline","title":"Fleet Demo Inline","columns":["Ship","Registry","Class","Crew","Status"],"rows":[["Enterprise","1701","Constitution","430","active"],["Voyager","74656","Intrepid","150","survey"],["Defiant","74205","Escort","50","tactical"]]}"#,
            ),
            lcars_line(6, "```"),
        ];

        let layouts = lcars_markdown_dataview_layouts(&lines, 180);
        assert_eq!(layouts.len(), 1);
        let layout = layouts[0];
        assert_eq!(layout.visible_rows, 6);
        assert_eq!(layout.bay_cols, layout.rail_cols + 2 + layout.text_cols);
        assert!(layout.bay_cols < 90);
        assert!(layout.text_cols >= 45);
    }

    #[test]
    fn lcars_markdown_dataview_layout_clamps_to_narrow_pane() {
        let lines = [
            lcars_line(0, "<!-- owt:lcars-md v=1 -->"),
            lcars_line(1, "```lcars-dataview"),
            lcars_line(
                2,
                r#"{"version":1,"kind":"owt.lcars.dataview","id":"fleet.demo","title":"Fleet Demo","columns":["Ship","Registry"],"rows":[["Enterprise","1701"]]}"#,
            ),
            lcars_line(3, "```"),
        ];

        let layouts = lcars_markdown_dataview_layouts(&lines, 24);
        assert_eq!(layouts.len(), 1);
        assert_eq!(layouts[0].bay_cols, 24);
        assert_eq!(layouts[0].text_cols, 20);
    }

    #[test]
    fn lcars_markdown_leaves_invalid_dataview_visible() {
        let lines = [
            lcars_line(0, "<!-- owt:lcars-md v=1 -->"),
            lcars_line(1, "```lcars-dataview"),
            lcars_line(2, r#"{"version":1}"#),
            lcars_line(3, "```"),
        ];

        assert_eq!(
            lcars_markdown_kinds(&lines),
            vec![
                (1, LcarsMarkdownKind::Warning),
                (2, LcarsMarkdownKind::Code),
                (3, LcarsMarkdownKind::Code),
            ]
        );
        assert_eq!(lcars_markdown_suppressed_rows(&lines), vec![0]);
        assert!(lcars_markdown_dataview_titles(&lines).is_empty());
    }

    #[test]
    fn lcars_markdown_visible_range_filters_lookback_lines() {
        let lines = [
            lcars_line(-2, "<!-- owt:lcars-md v=1 -->"),
            lcars_line(-1, "# Above viewport"),
            lcars_line(0, "## Visible"),
        ];

        let markers = LcarsMarkdownScanner::scan(&lines, 0..1).markers;
        assert_eq!(
            markers,
            vec![LcarsMarkdownMarker {
                stable_row: 0,
                kind: LcarsMarkdownKind::Section { level: 2 },
            }]
        );
    }
}
