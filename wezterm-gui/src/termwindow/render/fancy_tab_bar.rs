use crate::customglyph::*;
use crate::quad::{QuadTrait, TripleLayerQuadAllocator};
use crate::tabbar::{TabBarItem, TabEntry};
use crate::termwindow::box_model::*;
use crate::termwindow::render::corners::*;

use crate::termwindow::render::window_buttons::window_button_element;
use crate::termwindow::{UIItem, UIItemType};
use crate::utilsprites::RenderMetrics;
use config::{Dimension, DimensionContext, TabBarColors};
use termwiz::color::RgbColor;
use wezterm_term::color::{ColorAttribute, ColorPalette};
use window::color::LinearRgba;
use window::{IntegratedTitleButtonAlignment, IntegratedTitleButtonStyle};

const OWT_LCARS_WINDOW_CHROME_LEFT_RAIL_CELLS: f32 = 6.5;
const OWT_LCARS_WINDOW_CHROME_LEFT_RAIL_MIN: f32 = 64.0;
const OWT_LCARS_WINDOW_CHROME_LEFT_RAIL_MAX: f32 = 88.0;
const OWT_LCARS_WINDOW_CHROME_RIGHT_CONTROLS_CELLS: f32 = 15.5;
const OWT_LCARS_WINDOW_CHROME_RIGHT_CONTROLS_MIN: f32 = 148.0;
const OWT_LCARS_WINDOW_CHROME_RIGHT_CONTROLS_MAX: f32 = 224.0;
const OWT_LCARS_WINDOW_CHROME_RAIL_GAP: f32 = 8.0;
const OWT_LCARS_WINDOW_CHROME_BLACK_GAP: f32 = 9.0;
const OWT_LCARS_WINDOW_CHROME_MIN_WIDTH: f32 = 320.0;
const OWT_LCARS_WINDOW_CHROME_MIN_HEIGHT: f32 = 180.0;

#[derive(Clone, Copy, Debug)]
struct OwtLcarsChromeRailSegment {
    offset: f32,
    height: f32,
    width_factor: f32,
}

#[derive(Clone, Copy, Debug)]
struct OwtLcarsChromeLeftGeometry {
    reserved_width: f32,
    rail_width: f32,
    boundary_mask_width: f32,
    slab_width: f32,
}

fn owt_lcars_window_chrome_left_reserved_for_cell_width(cell_width: f32) -> f32 {
    (cell_width * OWT_LCARS_WINDOW_CHROME_LEFT_RAIL_CELLS)
        .max(OWT_LCARS_WINDOW_CHROME_LEFT_RAIL_MIN)
        .min(OWT_LCARS_WINDOW_CHROME_LEFT_RAIL_MAX)
}

fn owt_lcars_window_chrome_left_geometry(
    reserved_width: f32,
    cell_width: f32,
) -> OwtLcarsChromeLeftGeometry {
    let rail_width = (reserved_width - OWT_LCARS_WINDOW_CHROME_RAIL_GAP)
        .max(20.0)
        .min(reserved_width.max(1.0));
    let boundary_mask_width = (OWT_LCARS_WINDOW_CHROME_RAIL_GAP
        + (cell_width * 0.75).clamp(5.0, 10.0))
    .min(reserved_width.max(rail_width) * 0.24)
    .min((reserved_width - 8.0).max(0.0));
    let slab_width = (rail_width - boundary_mask_width)
        .min(rail_width * 0.64)
        .max((cell_width * 3.6).clamp(28.0, 48.0))
        .min((reserved_width - boundary_mask_width).max(cell_width * 2.5))
        .min(reserved_width.max(1.0));

    OwtLcarsChromeLeftGeometry {
        reserved_width,
        rail_width,
        boundary_mask_width,
        slab_width,
    }
}

fn owt_lcars_window_chrome_viewport_ready(width: f32, height: f32) -> bool {
    width >= OWT_LCARS_WINDOW_CHROME_MIN_WIDTH && height >= OWT_LCARS_WINDOW_CHROME_MIN_HEIGHT
}

fn owt_lcars_window_chrome_body_segment_plan(
    available_height: f32,
    gap: f32,
) -> Vec<OwtLcarsChromeRailSegment> {
    if available_height <= 4.0 {
        return Vec::new();
    }

    let weights = [0.18_f32, 0.20, 0.16, 0.18];
    let total_gap = gap * weights.len().saturating_sub(1) as f32;
    let usable_height =
        ((available_height - total_gap).max(0.0) * 0.68).min(available_height.max(0.0));
    let total_weight = weights.iter().sum::<f32>().max(1.0);
    let mut cursor = 0.0;

    weights
        .iter()
        .copied()
        .enumerate()
        .filter_map(|(index, weight)| {
            let remaining_weight = weights[index..].iter().sum::<f32>().max(weight);
            let remaining_height = (usable_height - cursor).max(0.0);
            let height = if index + 1 == weights.len() {
                remaining_height
            } else {
                (usable_height * weight / total_weight)
                    .max(8.0)
                    .min(remaining_height / remaining_weight * weight)
            };
            if height <= 0.0 {
                return None;
            }
            let segment = OwtLcarsChromeRailSegment {
                offset: cursor + index as f32 * gap,
                height,
                width_factor: [1.0, 0.72, 0.88, 0.58][index],
            };
            cursor += height;
            Some(segment)
        })
        .collect()
}

const X_BUTTON: &[Poly] = &[
    Poly {
        path: &[
            PolyCommand::MoveTo(BlockCoord::One, BlockCoord::Zero),
            PolyCommand::LineTo(BlockCoord::Zero, BlockCoord::One),
        ],
        intensity: BlockAlpha::Full,
        style: PolyStyle::Outline,
    },
    Poly {
        path: &[
            PolyCommand::MoveTo(BlockCoord::Zero, BlockCoord::Zero),
            PolyCommand::LineTo(BlockCoord::One, BlockCoord::One),
        ],
        intensity: BlockAlpha::Full,
        style: PolyStyle::Outline,
    },
];

const PLUS_BUTTON: &[Poly] = &[
    Poly {
        path: &[
            PolyCommand::MoveTo(BlockCoord::Frac(1, 2), BlockCoord::Zero),
            PolyCommand::LineTo(BlockCoord::Frac(1, 2), BlockCoord::One),
        ],
        intensity: BlockAlpha::Full,
        style: PolyStyle::Outline,
    },
    Poly {
        path: &[
            PolyCommand::MoveTo(BlockCoord::Zero, BlockCoord::Frac(1, 2)),
            PolyCommand::LineTo(BlockCoord::One, BlockCoord::Frac(1, 2)),
        ],
        intensity: BlockAlpha::Full,
        style: PolyStyle::Outline,
    },
];

const MINUS_BUTTON: &[Poly] = &[Poly {
    path: &[
        PolyCommand::MoveTo(BlockCoord::Frac(1, 10), BlockCoord::Frac(6, 10)),
        PolyCommand::LineTo(BlockCoord::Frac(9, 10), BlockCoord::Frac(6, 10)),
    ],
    intensity: BlockAlpha::Full,
    style: PolyStyle::Outline,
}];

const MAXIMIZE_BUTTON: &[Poly] = &[Poly {
    path: &[
        PolyCommand::MoveTo(BlockCoord::Frac(2, 10), BlockCoord::Frac(2, 10)),
        PolyCommand::LineTo(BlockCoord::Frac(8, 10), BlockCoord::Frac(2, 10)),
        PolyCommand::LineTo(BlockCoord::Frac(8, 10), BlockCoord::Frac(8, 10)),
        PolyCommand::LineTo(BlockCoord::Frac(2, 10), BlockCoord::Frac(8, 10)),
        PolyCommand::LineTo(BlockCoord::Frac(2, 10), BlockCoord::Frac(2, 10)),
    ],
    intensity: BlockAlpha::Full,
    style: PolyStyle::Outline,
}];

const RESTORE_BUTTON: &[Poly] = &[
    Poly {
        path: &[
            PolyCommand::MoveTo(BlockCoord::Frac(4, 10), BlockCoord::Frac(2, 10)),
            PolyCommand::LineTo(BlockCoord::Frac(8, 10), BlockCoord::Frac(2, 10)),
            PolyCommand::LineTo(BlockCoord::Frac(8, 10), BlockCoord::Frac(6, 10)),
        ],
        intensity: BlockAlpha::Full,
        style: PolyStyle::Outline,
    },
    Poly {
        path: &[
            PolyCommand::MoveTo(BlockCoord::Frac(2, 10), BlockCoord::Frac(4, 10)),
            PolyCommand::LineTo(BlockCoord::Frac(6, 10), BlockCoord::Frac(4, 10)),
            PolyCommand::LineTo(BlockCoord::Frac(6, 10), BlockCoord::Frac(8, 10)),
            PolyCommand::LineTo(BlockCoord::Frac(2, 10), BlockCoord::Frac(8, 10)),
            PolyCommand::LineTo(BlockCoord::Frac(2, 10), BlockCoord::Frac(4, 10)),
        ],
        intensity: BlockAlpha::Full,
        style: PolyStyle::Outline,
    },
];

impl crate::TermWindow {
    pub fn invalidate_fancy_tab_bar(&mut self) {
        self.fancy_tab_bar.take();
    }

    pub fn owt_lcars_window_chrome_left_reserved_pixels(&self) -> f32 {
        if !self.owt_lcars_window_chrome_active() {
            return 0.0;
        }

        owt_lcars_window_chrome_left_reserved_for_cell_width(
            self.render_metrics.cell_size.width as f32,
        )
    }

    fn owt_lcars_window_chrome_right_reserved_pixels(&self) -> f32 {
        if !self.owt_lcars_window_chrome_active()
            || !self
                .config
                .window_decorations
                .contains(window::WindowDecorations::INTEGRATED_BUTTONS)
        {
            return 0.0;
        }

        let requested = self.render_metrics.cell_size.width as f32
            * OWT_LCARS_WINDOW_CHROME_RIGHT_CONTROLS_CELLS;
        requested
            .max(OWT_LCARS_WINDOW_CHROME_RIGHT_CONTROLS_MIN)
            .min(OWT_LCARS_WINDOW_CHROME_RIGHT_CONTROLS_MAX)
    }

    fn owt_lcars_window_chrome_active(&self) -> bool {
        self.config.owt_lcars_window_chrome
            && self.config.use_fancy_tab_bar
            && self.show_tab_bar
            && !self.config.tab_bar_at_bottom
            && owt_lcars_window_chrome_viewport_ready(
                self.dimensions.pixel_width as f32,
                self.dimensions.pixel_height as f32,
            )
    }

    pub fn build_fancy_tab_bar(&self, palette: &ColorPalette) -> anyhow::Result<ComputedElement> {
        if self.owt_lcars_window_chrome_active() {
            return self.build_owt_lcars_window_tab_bar(palette);
        }

        let tab_bar_height = self.tab_bar_pixel_height()?;
        let font = self.fonts.title_font()?;
        let metrics = RenderMetrics::with_font_metrics(&font.metrics());
        let items = self.tab_bar.items();
        let colors = self
            .config
            .colors
            .as_ref()
            .and_then(|c| c.tab_bar.as_ref())
            .cloned()
            .unwrap_or_else(TabBarColors::default);

        let mut left_status = vec![];
        let mut left_eles = vec![];
        let mut right_eles = vec![];
        let bar_colors = ElementColors {
            border: BorderColor::default(),
            bg: if self.focused.is_some() {
                self.config.window_frame.active_titlebar_bg
            } else {
                self.config.window_frame.inactive_titlebar_bg
            }
            .to_linear()
            .into(),
            text: if self.focused.is_some() {
                self.config.window_frame.active_titlebar_fg
            } else {
                self.config.window_frame.inactive_titlebar_fg
            }
            .to_linear()
            .into(),
        };

        let item_to_elem = |item: &TabEntry| -> Element {
            let element = Element::with_line(&font, &item.title, palette);

            let bg_color = item
                .title
                .get_cell(0)
                .and_then(|c| match c.attrs().background() {
                    ColorAttribute::Default => None,
                    col => Some(palette.resolve_bg(col)),
                });
            let fg_color = item
                .title
                .get_cell(0)
                .and_then(|c| match c.attrs().foreground() {
                    ColorAttribute::Default => None,
                    col => Some(palette.resolve_fg(col)),
                });

            let new_tab = colors.new_tab();
            let new_tab_hover = colors.new_tab_hover();
            let active_tab = colors.active_tab();

            match item.item {
                TabBarItem::RightStatus | TabBarItem::LeftStatus | TabBarItem::None => element
                    .item_type(UIItemType::TabBar(TabBarItem::None))
                    .line_height(Some(1.75))
                    .margin(BoxDimension {
                        left: Dimension::Cells(0.),
                        right: Dimension::Cells(0.),
                        top: Dimension::Cells(0.0),
                        bottom: Dimension::Cells(0.),
                    })
                    .padding(BoxDimension {
                        left: Dimension::Cells(0.5),
                        right: Dimension::Cells(0.),
                        top: Dimension::Cells(0.),
                        bottom: Dimension::Cells(0.),
                    })
                    .border(BoxDimension::new(Dimension::Pixels(0.)))
                    .colors(bar_colors.clone()),
                TabBarItem::NewTabButton => Element::new(
                    &font,
                    ElementContent::Poly {
                        line_width: metrics.underline_height.max(2),
                        poly: SizedPoly {
                            poly: PLUS_BUTTON,
                            width: Dimension::Pixels(metrics.cell_size.height as f32 / 2.),
                            height: Dimension::Pixels(metrics.cell_size.height as f32 / 2.),
                        },
                    },
                )
                .vertical_align(VerticalAlign::Middle)
                .item_type(UIItemType::TabBar(item.item.clone()))
                .margin(BoxDimension {
                    left: Dimension::Cells(0.5),
                    right: Dimension::Cells(0.),
                    top: Dimension::Cells(0.2),
                    bottom: Dimension::Cells(0.),
                })
                .padding(BoxDimension {
                    left: Dimension::Cells(0.5),
                    right: Dimension::Cells(0.5),
                    top: Dimension::Cells(0.2),
                    bottom: Dimension::Cells(0.25),
                })
                .border(BoxDimension::new(Dimension::Pixels(1.)))
                .colors(ElementColors {
                    border: BorderColor::default(),
                    bg: new_tab.bg_color.to_linear().into(),
                    text: new_tab.fg_color.to_linear().into(),
                })
                .hover_colors(Some(ElementColors {
                    border: BorderColor::default(),
                    bg: new_tab_hover.bg_color.to_linear().into(),
                    text: new_tab_hover.fg_color.to_linear().into(),
                })),
                TabBarItem::Tab { active, .. } if active => element
                    .vertical_align(VerticalAlign::Bottom)
                    .item_type(UIItemType::TabBar(item.item.clone()))
                    .margin(BoxDimension {
                        left: Dimension::Cells(0.),
                        right: Dimension::Cells(0.),
                        top: Dimension::Cells(0.2),
                        bottom: Dimension::Cells(0.),
                    })
                    .padding(BoxDimension {
                        left: Dimension::Cells(0.5),
                        right: Dimension::Cells(0.5),
                        top: Dimension::Cells(0.2),
                        bottom: Dimension::Cells(0.25),
                    })
                    .border(BoxDimension::new(Dimension::Pixels(1.)))
                    .border_corners(Some(Corners {
                        top_left: SizedPoly {
                            width: Dimension::Cells(0.5),
                            height: Dimension::Cells(0.5),
                            poly: TOP_LEFT_ROUNDED_CORNER,
                        },
                        top_right: SizedPoly {
                            width: Dimension::Cells(0.5),
                            height: Dimension::Cells(0.5),
                            poly: TOP_RIGHT_ROUNDED_CORNER,
                        },
                        bottom_left: SizedPoly::none(),
                        bottom_right: SizedPoly::none(),
                    }))
                    .colors(ElementColors {
                        border: BorderColor::new(
                            bg_color
                                .unwrap_or_else(|| active_tab.bg_color.into())
                                .to_linear(),
                        ),
                        bg: bg_color
                            .unwrap_or_else(|| active_tab.bg_color.into())
                            .to_linear()
                            .into(),
                        text: fg_color
                            .unwrap_or_else(|| active_tab.fg_color.into())
                            .to_linear()
                            .into(),
                    }),
                TabBarItem::Tab { .. } => element
                    .vertical_align(VerticalAlign::Bottom)
                    .item_type(UIItemType::TabBar(item.item.clone()))
                    .margin(BoxDimension {
                        left: Dimension::Cells(0.),
                        right: Dimension::Cells(0.),
                        top: Dimension::Cells(0.2),
                        bottom: Dimension::Cells(0.),
                    })
                    .padding(BoxDimension {
                        left: Dimension::Cells(0.5),
                        right: Dimension::Cells(0.5),
                        top: Dimension::Cells(0.2),
                        bottom: Dimension::Cells(0.25),
                    })
                    .border(BoxDimension::new(Dimension::Pixels(1.)))
                    .border_corners(Some(Corners {
                        top_left: SizedPoly {
                            width: Dimension::Cells(0.5),
                            height: Dimension::Cells(0.5),
                            poly: TOP_LEFT_ROUNDED_CORNER,
                        },
                        top_right: SizedPoly {
                            width: Dimension::Cells(0.5),
                            height: Dimension::Cells(0.5),
                            poly: TOP_RIGHT_ROUNDED_CORNER,
                        },
                        bottom_left: SizedPoly {
                            width: Dimension::Cells(0.),
                            height: Dimension::Cells(0.33),
                            poly: &[],
                        },
                        bottom_right: SizedPoly {
                            width: Dimension::Cells(0.),
                            height: Dimension::Cells(0.33),
                            poly: &[],
                        },
                    }))
                    .colors({
                        let inactive_tab = colors.inactive_tab();
                        let bg = bg_color
                            .unwrap_or_else(|| inactive_tab.bg_color.into())
                            .to_linear();
                        let edge = colors.inactive_tab_edge().to_linear();
                        ElementColors {
                            border: BorderColor {
                                left: bg,
                                right: edge,
                                top: bg,
                                bottom: bg,
                            },
                            bg: bg.into(),
                            text: fg_color
                                .unwrap_or_else(|| inactive_tab.fg_color.into())
                                .to_linear()
                                .into(),
                        }
                    })
                    .hover_colors({
                        let inactive_tab_hover = colors.inactive_tab_hover();
                        Some(ElementColors {
                            border: BorderColor::new(
                                bg_color
                                    .unwrap_or_else(|| inactive_tab_hover.bg_color.into())
                                    .to_linear(),
                            ),
                            bg: bg_color
                                .unwrap_or_else(|| inactive_tab_hover.bg_color.into())
                                .to_linear()
                                .into(),
                            text: fg_color
                                .unwrap_or_else(|| inactive_tab_hover.fg_color.into())
                                .to_linear()
                                .into(),
                        })
                    }),
                TabBarItem::WindowButton(button) => window_button_element(
                    button,
                    self.window_state.contains(window::WindowState::MAXIMIZED),
                    &font,
                    &metrics,
                    &self.config,
                ),
            }
        };

        let num_tabs: f32 = items
            .iter()
            .map(|item| match item.item {
                TabBarItem::NewTabButton | TabBarItem::Tab { .. } => 1.,
                _ => 0.,
            })
            .sum();
        let max_tab_width = ((self.dimensions.pixel_width as f32 / num_tabs)
            - (1.5 * metrics.cell_size.width as f32))
            .max(0.);

        // Reserve space for the native titlebar buttons
        if self
            .config
            .window_decorations
            .contains(::window::WindowDecorations::INTEGRATED_BUTTONS)
            && self.config.integrated_title_button_style == IntegratedTitleButtonStyle::MacOsNative
            && !self.window_state.contains(window::WindowState::FULL_SCREEN)
        {
            left_status.push(
                Element::new(&font, ElementContent::Text("".to_string())).margin(BoxDimension {
                    left: Dimension::Cells(4.0), // FIXME: determine exact width of macos ... buttons
                    right: Dimension::Cells(0.),
                    top: Dimension::Cells(0.),
                    bottom: Dimension::Cells(0.),
                }),
            );
        }

        for item in items {
            match item.item {
                TabBarItem::LeftStatus => left_status.push(item_to_elem(item)),
                TabBarItem::None | TabBarItem::RightStatus => right_eles.push(item_to_elem(item)),
                TabBarItem::WindowButton(_) => {
                    if self.config.integrated_title_button_alignment
                        == IntegratedTitleButtonAlignment::Left
                    {
                        left_eles.push(item_to_elem(item))
                    } else {
                        right_eles.push(item_to_elem(item))
                    }
                }
                TabBarItem::Tab { tab_idx, active } => {
                    let mut elem = item_to_elem(item);
                    elem.max_width = Some(Dimension::Pixels(max_tab_width));
                    elem.content = match elem.content {
                        ElementContent::Text(_) => unreachable!(),
                        ElementContent::Poly { .. } => unreachable!(),
                        ElementContent::Children(mut kids) => {
                            let x_button = Element::new(
                                &font,
                                ElementContent::Poly {
                                    line_width: metrics.underline_height.max(2),
                                    poly: SizedPoly {
                                        poly: X_BUTTON,
                                        width: Dimension::Pixels(
                                            metrics.cell_size.height as f32 / 2.,
                                        ),
                                        height: Dimension::Pixels(
                                            metrics.cell_size.height as f32 / 2.,
                                        ),
                                    },
                                },
                            )
                            // Ensure that we draw our background over the
                            // top of the rest of the tab contents
                            .zindex(1)
                            .vertical_align(VerticalAlign::Middle)
                            .float(Float::Right)
                            .item_type(UIItemType::CloseTab(tab_idx))
                            .hover_colors({
                                let inactive_tab_hover = colors.inactive_tab_hover();
                                let active_tab = colors.active_tab();

                                Some(ElementColors {
                                    border: BorderColor::default(),
                                    bg: (if active {
                                        inactive_tab_hover.bg_color
                                    } else {
                                        active_tab.bg_color
                                    })
                                    .to_linear()
                                    .into(),
                                    text: (if active {
                                        inactive_tab_hover.fg_color
                                    } else {
                                        active_tab.fg_color
                                    })
                                    .to_linear()
                                    .into(),
                                })
                            })
                            .padding(BoxDimension {
                                left: Dimension::Cells(0.25),
                                right: Dimension::Cells(0.25),
                                top: Dimension::Cells(0.25),
                                bottom: Dimension::Cells(0.25),
                            })
                            .margin(BoxDimension {
                                left: Dimension::Cells(0.5),
                                right: Dimension::Cells(0.),
                                top: Dimension::Cells(0.),
                                bottom: Dimension::Cells(0.),
                            });

                            kids.push(x_button);
                            ElementContent::Children(kids)
                        }
                    };
                    left_eles.push(elem);
                }
                _ => left_eles.push(item_to_elem(item)),
            }
        }

        let mut children = vec![];

        if !left_status.is_empty() {
            children.push(
                Element::new(&font, ElementContent::Children(left_status))
                    .colors(bar_colors.clone()),
            );
        }

        let window_buttons_at_left = self
            .config
            .window_decorations
            .contains(window::WindowDecorations::INTEGRATED_BUTTONS)
            && (self.config.integrated_title_button_alignment
                == IntegratedTitleButtonAlignment::Left
                || self.config.integrated_title_button_style
                    == IntegratedTitleButtonStyle::MacOsNative);

        let left_padding = if window_buttons_at_left {
            if self.config.integrated_title_button_style == IntegratedTitleButtonStyle::MacOsNative
            {
                if !self.window_state.contains(window::WindowState::FULL_SCREEN) {
                    Dimension::Pixels(70.0)
                } else {
                    Dimension::Cells(0.5)
                }
            } else {
                Dimension::Pixels(0.0)
            }
        } else {
            Dimension::Cells(0.5)
        };

        children.push(
            Element::new(&font, ElementContent::Children(left_eles))
                .vertical_align(VerticalAlign::Bottom)
                .colors(bar_colors.clone())
                .padding(BoxDimension {
                    left: left_padding,
                    right: Dimension::Cells(0.),
                    top: Dimension::Cells(0.),
                    bottom: Dimension::Cells(0.),
                })
                .zindex(1),
        );
        children.push(
            Element::new(&font, ElementContent::Children(right_eles))
                .colors(bar_colors.clone())
                .float(Float::Right),
        );

        let content = ElementContent::Children(children);

        let tabs = Element::new(&font, content)
            .display(DisplayType::Block)
            .item_type(UIItemType::TabBar(TabBarItem::None))
            .min_width(Some(Dimension::Pixels(self.dimensions.pixel_width as f32)))
            .min_height(Some(Dimension::Pixels(tab_bar_height)))
            .vertical_align(VerticalAlign::Bottom)
            .colors(bar_colors);

        let border = self.get_os_border();

        let mut computed = self.compute_element(
            &LayoutContext {
                height: DimensionContext {
                    dpi: self.dimensions.dpi as f32,
                    pixel_max: self.dimensions.pixel_height as f32,
                    pixel_cell: metrics.cell_size.height as f32,
                },
                width: DimensionContext {
                    dpi: self.dimensions.dpi as f32,
                    pixel_max: self.dimensions.pixel_width as f32,
                    pixel_cell: metrics.cell_size.width as f32,
                },
                bounds: euclid::rect(
                    border.left.get() as f32,
                    0.,
                    self.dimensions.pixel_width as f32 - (border.left + border.right).get() as f32,
                    tab_bar_height,
                ),
                metrics: &metrics,
                gl_state: self.render_state.as_ref().unwrap(),
                zindex: 10,
            },
            &tabs,
        )?;

        computed.translate(euclid::vec2(
            0.,
            if self.config.tab_bar_at_bottom {
                self.dimensions.pixel_height as f32
                    - (computed.bounds.height() + border.bottom.get() as f32)
            } else {
                border.top.get() as f32
            },
        ));

        Ok(computed)
    }

    fn build_owt_lcars_window_tab_bar(
        &self,
        palette: &ColorPalette,
    ) -> anyhow::Result<ComputedElement> {
        let tab_bar_height = self.tab_bar_pixel_height()?;
        let font = self.fonts.title_font()?;
        let metrics = RenderMetrics::with_font_metrics(&font.metrics());
        let items = self.tab_bar.items();

        let black = lcars_chrome_color(0, 0, 0);
        let orange = lcars_chrome_color(255, 136, 0);
        let amber = lcars_chrome_color(255, 204, 112);
        let peach = lcars_chrome_color(255, 149, 96);
        let violet = lcars_chrome_color(197, 143, 255);
        let blue = lcars_chrome_color(137, 148, 255);
        let red = lcars_chrome_color(221, 84, 80);
        let root_button_corners = lcars_tab_active_corners();
        let bar_colors = ElementColors {
            border: BorderColor::default(),
            bg: black.into(),
            text: amber.into(),
        };

        let item_to_elem = |item: &TabEntry| -> Element {
            let element = Element::with_line(&font, &item.title, palette);
            let bg_color = item
                .title
                .get_cell(0)
                .and_then(|c| match c.attrs().background() {
                    ColorAttribute::Default => None,
                    col => Some(palette.resolve_bg(col).to_linear()),
                });
            let fg_color = item
                .title
                .get_cell(0)
                .and_then(|c| match c.attrs().foreground() {
                    ColorAttribute::Default => None,
                    col => Some(palette.resolve_fg(col).to_linear()),
                });

            match item.item {
                TabBarItem::RightStatus | TabBarItem::LeftStatus | TabBarItem::None => element
                    .item_type(UIItemType::TabBar(TabBarItem::None))
                    .line_height(Some(1.92))
                    .margin(BoxDimension::new(Dimension::Pixels(0.)))
                    .padding(BoxDimension {
                        left: Dimension::Cells(0.45),
                        right: Dimension::Cells(0.35),
                        top: Dimension::Cells(0.05),
                        bottom: Dimension::Cells(0.05),
                    })
                    .border(BoxDimension::new(Dimension::Pixels(0.)))
                    .colors(bar_colors.clone()),
                TabBarItem::NewTabButton => Element::new(
                    &font,
                    ElementContent::Poly {
                        line_width: metrics.underline_height.max(2),
                        poly: SizedPoly {
                            poly: PLUS_BUTTON,
                            width: Dimension::Pixels(metrics.cell_size.height as f32 * 0.52),
                            height: Dimension::Pixels(metrics.cell_size.height as f32 * 0.52),
                        },
                    },
                )
                .vertical_align(VerticalAlign::Middle)
                .item_type(UIItemType::TabBar(item.item.clone()))
                .margin(BoxDimension {
                    left: Dimension::Cells(0.22),
                    right: Dimension::Cells(0.18),
                    top: Dimension::Cells(0.48),
                    bottom: Dimension::Cells(0.20),
                })
                .padding(BoxDimension {
                    left: Dimension::Cells(0.68),
                    right: Dimension::Cells(0.68),
                    top: Dimension::Cells(0.22),
                    bottom: Dimension::Cells(0.22),
                })
                .border(BoxDimension::new(Dimension::Pixels(2.)))
                .border_corners(Some(lcars_tab_slab_corners()))
                .colors(ElementColors {
                    border: BorderColor::new(amber),
                    bg: amber.into(),
                    text: black.into(),
                })
                .hover_colors(Some(ElementColors {
                    border: BorderColor::new(peach),
                    bg: orange.into(),
                    text: black.into(),
                })),
                TabBarItem::Tab { active, .. } => {
                    let fill = bg_color.unwrap_or(if active { orange } else { blue });
                    let text = fg_color.unwrap_or(black);
                    let border = if active { amber } else { black };
                    let hover = if active { peach } else { violet };

                    element
                        .vertical_align(VerticalAlign::Bottom)
                        .item_type(UIItemType::TabBar(item.item.clone()))
                        .margin(BoxDimension {
                            left: Dimension::Cells(0.0),
                            right: Dimension::Cells(0.18),
                            top: Dimension::Cells(if active { 0.52 } else { 0.68 }),
                            bottom: Dimension::Cells(0.12),
                        })
                        .padding(BoxDimension {
                            left: Dimension::Cells(if active { 1.14 } else { 0.94 }),
                            right: Dimension::Cells(if active { 1.02 } else { 0.84 }),
                            top: Dimension::Cells(if active { 0.24 } else { 0.18 }),
                            bottom: Dimension::Cells(if active { 0.24 } else { 0.20 }),
                        })
                        .border(BoxDimension::new(Dimension::Pixels(if active {
                            2.5
                        } else {
                            1.5
                        })))
                        .border_corners(Some(if active {
                            root_button_corners.clone()
                        } else {
                            lcars_tab_slab_corners()
                        }))
                        .colors(ElementColors {
                            border: BorderColor::new(border),
                            bg: fill.into(),
                            text: text.into(),
                        })
                        .hover_colors(Some(ElementColors {
                            border: BorderColor::new(hover),
                            bg: hover.into(),
                            text: black.into(),
                        }))
                }
                TabBarItem::WindowButton(button) => self.owt_lcars_window_button_element(
                    button,
                    self.window_state.contains(window::WindowState::MAXIMIZED),
                    &font,
                    &metrics,
                    black,
                    amber,
                    blue,
                    red,
                    peach,
                ),
            }
        };

        let num_tabs: f32 = items
            .iter()
            .map(|item| match item.item {
                TabBarItem::NewTabButton | TabBarItem::Tab { .. } => 1.,
                _ => 0.,
            })
            .sum::<f32>()
            .max(1.0);
        let chrome_reserved = self.owt_lcars_window_chrome_left_reserved_pixels()
            + self.owt_lcars_window_chrome_right_reserved_pixels()
            + 32.0;
        let max_tab_width = (((self.dimensions.pixel_width as f32 - chrome_reserved) / num_tabs)
            - (1.4 * metrics.cell_size.width as f32))
            .max(metrics.cell_size.width as f32 * 7.0);

        let mut left_status = vec![];
        let mut left_eles = vec![];
        let mut right_eles = vec![];

        if self
            .config
            .window_decorations
            .contains(::window::WindowDecorations::INTEGRATED_BUTTONS)
            && self.config.integrated_title_button_style == IntegratedTitleButtonStyle::MacOsNative
            && !self.window_state.contains(window::WindowState::FULL_SCREEN)
        {
            left_status.push(
                Element::new(&font, ElementContent::Text("".to_string())).margin(BoxDimension {
                    left: Dimension::Cells(4.0),
                    right: Dimension::Cells(0.),
                    top: Dimension::Cells(0.),
                    bottom: Dimension::Cells(0.),
                }),
            );
        }

        left_eles.push(self.owt_lcars_window_root_corner_element(&font, &metrics, tab_bar_height));

        for item in items {
            match item.item {
                TabBarItem::LeftStatus => left_status.push(item_to_elem(item)),
                TabBarItem::None | TabBarItem::RightStatus => right_eles.push(item_to_elem(item)),
                TabBarItem::WindowButton(_) => {
                    if self.config.integrated_title_button_alignment
                        == IntegratedTitleButtonAlignment::Left
                    {
                        left_eles.push(item_to_elem(item))
                    } else {
                        right_eles.push(item_to_elem(item))
                    }
                }
                TabBarItem::Tab { tab_idx, active } => {
                    let mut elem = item_to_elem(item);
                    elem.max_width = Some(Dimension::Pixels(max_tab_width));
                    elem.content = match elem.content {
                        ElementContent::Text(_) => unreachable!(),
                        ElementContent::Poly { .. } => unreachable!(),
                        ElementContent::Children(mut kids) => {
                            let x_button = Element::new(
                                &font,
                                ElementContent::Poly {
                                    line_width: metrics.underline_height.max(2),
                                    poly: SizedPoly {
                                        poly: X_BUTTON,
                                        width: Dimension::Pixels(
                                            metrics.cell_size.height as f32 * 0.46,
                                        ),
                                        height: Dimension::Pixels(
                                            metrics.cell_size.height as f32 * 0.46,
                                        ),
                                    },
                                },
                            )
                            .zindex(1)
                            .vertical_align(VerticalAlign::Middle)
                            .float(Float::Right)
                            .item_type(UIItemType::CloseTab(tab_idx))
                            .hover_colors(Some(ElementColors {
                                border: BorderColor::default(),
                                bg: (if active { peach } else { amber }).into(),
                                text: (if active { amber } else { black }).into(),
                            }))
                            .padding(BoxDimension {
                                left: Dimension::Cells(0.25),
                                right: Dimension::Cells(0.25),
                                top: Dimension::Cells(0.20),
                                bottom: Dimension::Cells(0.20),
                            })
                            .margin(BoxDimension {
                                left: Dimension::Cells(0.55),
                                right: Dimension::Cells(0.),
                                top: Dimension::Cells(0.),
                                bottom: Dimension::Cells(0.),
                            });

                            kids.push(x_button);
                            ElementContent::Children(kids)
                        }
                    };
                    left_eles.push(elem);
                }
                _ => left_eles.push(item_to_elem(item)),
            }
        }

        let mut children = vec![];

        if !left_status.is_empty() {
            children.push(
                Element::new(&font, ElementContent::Children(left_status))
                    .colors(bar_colors.clone()),
            );
        }

        let window_buttons_at_left = self
            .config
            .window_decorations
            .contains(window::WindowDecorations::INTEGRATED_BUTTONS)
            && (self.config.integrated_title_button_alignment
                == IntegratedTitleButtonAlignment::Left
                || self.config.integrated_title_button_style
                    == IntegratedTitleButtonStyle::MacOsNative);

        let left_padding = if window_buttons_at_left {
            if self.config.integrated_title_button_style == IntegratedTitleButtonStyle::MacOsNative
            {
                if !self.window_state.contains(window::WindowState::FULL_SCREEN) {
                    Dimension::Pixels(70.0)
                } else {
                    Dimension::Cells(0.5)
                }
            } else {
                Dimension::Pixels(0.0)
            }
        } else {
            Dimension::Pixels(8.0)
        };

        children.push(
            Element::new(&font, ElementContent::Children(left_eles))
                .vertical_align(VerticalAlign::Bottom)
                .colors(bar_colors.clone())
                .padding(BoxDimension {
                    left: left_padding,
                    right: Dimension::Cells(0.),
                    top: Dimension::Cells(0.),
                    bottom: Dimension::Cells(0.),
                })
                .zindex(1),
        );
        children.push(
            Element::new(&font, ElementContent::Children(right_eles))
                .colors(bar_colors.clone())
                .float(Float::Right),
        );

        let tabs = Element::new(&font, ElementContent::Children(children))
            .display(DisplayType::Block)
            .item_type(UIItemType::TabBar(TabBarItem::None))
            .min_width(Some(Dimension::Pixels(self.dimensions.pixel_width as f32)))
            .min_height(Some(Dimension::Pixels(tab_bar_height)))
            .vertical_align(VerticalAlign::Bottom)
            .colors(bar_colors);

        let border = self.get_os_border();
        let mut computed = self.compute_element(
            &LayoutContext {
                height: DimensionContext {
                    dpi: self.dimensions.dpi as f32,
                    pixel_max: self.dimensions.pixel_height as f32,
                    pixel_cell: metrics.cell_size.height as f32,
                },
                width: DimensionContext {
                    dpi: self.dimensions.dpi as f32,
                    pixel_max: self.dimensions.pixel_width as f32,
                    pixel_cell: metrics.cell_size.width as f32,
                },
                bounds: euclid::rect(
                    border.left.get() as f32,
                    0.,
                    self.dimensions.pixel_width as f32 - (border.left + border.right).get() as f32,
                    tab_bar_height,
                ),
                metrics: &metrics,
                gl_state: self.render_state.as_ref().unwrap(),
                zindex: 10,
            },
            &tabs,
        )?;

        computed.translate(euclid::vec2(0., border.top.get() as f32));
        Ok(computed)
    }

    fn owt_lcars_window_root_corner_element(
        &self,
        font: &std::rc::Rc<wezterm_font::LoadedFont>,
        metrics: &RenderMetrics,
        tab_bar_height: f32,
    ) -> Element {
        let black = lcars_chrome_color(0, 0, 0);
        let rail_width = (self.owt_lcars_window_chrome_left_reserved_pixels()
            - OWT_LCARS_WINDOW_CHROME_RAIL_GAP)
            .max(metrics.cell_size.width as f32 * 5.0);

        Element::new(font, ElementContent::Text(String::new()))
            .vertical_align(VerticalAlign::Bottom)
            .min_width(Some(Dimension::Pixels(rail_width)))
            .min_height(Some(Dimension::Pixels(
                (tab_bar_height - 5.0).max(metrics.cell_size.height as f32),
            )))
            .margin(BoxDimension {
                left: Dimension::Pixels(0.0),
                right: Dimension::Cells(0.24),
                top: Dimension::Pixels(0.0),
                bottom: Dimension::Pixels(0.0),
            })
            .padding(BoxDimension::new(Dimension::Pixels(0.0)))
            .border(BoxDimension::new(Dimension::Pixels(0.0)))
            .colors(ElementColors {
                border: BorderColor::new(LinearRgba::TRANSPARENT),
                bg: black.into(),
                text: black.into(),
            })
    }

    fn owt_lcars_window_button_element(
        &self,
        window_button: window::IntegratedTitleButton,
        is_maximized: bool,
        font: &std::rc::Rc<wezterm_font::LoadedFont>,
        metrics: &RenderMetrics,
        black: LinearRgba,
        amber: LinearRgba,
        blue: LinearRgba,
        red: LinearRgba,
        peach: LinearRgba,
    ) -> Element {
        let (poly, fill, border) = match window_button {
            window::IntegratedTitleButton::Hide => (MINUS_BUTTON, amber, amber),
            window::IntegratedTitleButton::Maximize => {
                let poly = if is_maximized {
                    RESTORE_BUTTON
                } else {
                    MAXIMIZE_BUTTON
                };
                (poly, blue, blue)
            }
            window::IntegratedTitleButton::Close => (X_BUTTON, red, red),
        };

        Element::new(
            font,
            ElementContent::Poly {
                line_width: metrics.underline_height.max(2),
                poly: SizedPoly {
                    poly,
                    width: Dimension::Pixels(metrics.cell_size.height as f32 * 0.52),
                    height: Dimension::Pixels(metrics.cell_size.height as f32 * 0.52),
                },
            },
        )
        .zindex(2)
        .vertical_align(VerticalAlign::Middle)
        .item_type(UIItemType::TabBar(TabBarItem::WindowButton(window_button)))
        .margin(BoxDimension {
            left: Dimension::Cells(0.08),
            right: Dimension::Cells(0.10),
            top: Dimension::Cells(0.50),
            bottom: Dimension::Cells(0.22),
        })
        .padding(BoxDimension {
            left: Dimension::Cells(0.64),
            right: Dimension::Cells(0.64),
            top: Dimension::Cells(0.30),
            bottom: Dimension::Cells(0.30),
        })
        .border(BoxDimension::new(Dimension::Pixels(2.0)))
        .border_corners(Some(lcars_capsule_corners()))
        .colors(ElementColors {
            border: BorderColor::new(border),
            bg: fill.into(),
            text: black.into(),
        })
        .hover_colors(Some(ElementColors {
            border: BorderColor::new(peach),
            bg: peach.into(),
            text: black.into(),
        }))
    }

    pub fn paint_owt_lcars_window_chrome_rail(
        &mut self,
        layers: &mut TripleLayerQuadAllocator,
    ) -> anyhow::Result<()> {
        if !self.owt_lcars_window_chrome_active() {
            return Ok(());
        }

        let reserved = self.owt_lcars_window_chrome_left_reserved_pixels();
        if reserved <= 0.0 {
            return Ok(());
        }

        let border = self.get_os_border();
        let tab_bar_height = self.tab_bar_pixel_height()?;
        let left = border.left.get() as f32;
        let cell_width = self.render_metrics.cell_size.width as f32;
        let cell_height = self.render_metrics.cell_size.height as f32;
        let prompt_clearance = (cell_height * 1.25).clamp(18.0, 30.0);
        let top = border.top.get() as f32 + tab_bar_height + prompt_clearance;
        let bottom = self.dimensions.pixel_height as f32 - border.bottom.get() as f32 - 6.0;
        if bottom <= top + 24.0 {
            return Ok(());
        }

        let height = bottom - top;
        let black = lcars_chrome_color(0, 0, 0);
        let orange = lcars_chrome_color(255, 136, 0);
        let amber = lcars_chrome_color(255, 204, 112);
        let peach = lcars_chrome_color(255, 149, 96);
        let violet = lcars_chrome_color(197, 143, 255);
        let blue = lcars_chrome_color(137, 148, 255);
        let red = lcars_chrome_color(207, 79, 79);
        let left_geometry = owt_lcars_window_chrome_left_geometry(reserved, cell_width);
        let slab_width = left_geometry.slab_width.min(left_geometry.rail_width);
        let gutter_width = (left_geometry.reserved_width - slab_width)
            .max(left_geometry.boundary_mask_width)
            .max(0.0);

        let mast_top = border.top.get() as f32 + 6.0;
        let mast_left = left + left_geometry.reserved_width;
        let right_reserved = self.owt_lcars_window_chrome_right_reserved_pixels();
        let mast_right =
            self.dimensions.pixel_width as f32 - border.right.get() as f32 - right_reserved - 8.0;
        let mast_width = (mast_right - mast_left).max(0.0);

        if tab_bar_height > 24.0 {
            let top_layer = self.render_state.as_ref().unwrap().layer_for_zindex(12)?;
            let mut top_layers = top_layer.quad_allocator();
            let root_y = mast_top;
            let root_height = (top + 1.0 - root_y).max(tab_bar_height - 3.0).max(34.0);
            let root_radius = (slab_width * 0.58).min(root_height * 0.85);
            paint_lcars_rounded_rect(
                self,
                &mut top_layers,
                left,
                root_y,
                slab_width,
                root_height,
                root_radius,
                LCARS_CORNER_TOP_LEFT,
                red,
            )?;
            self.filled_rectangle(
                &mut top_layers,
                0,
                lcars_rect(
                    left + slab_width * 0.43,
                    root_y + root_height - 10.0,
                    slab_width * 0.42,
                    5.0,
                ),
                black,
            )?;
            self.filled_rectangle(
                &mut top_layers,
                0,
                lcars_rect(left + slab_width, root_y, gutter_width, root_height),
                black,
            )?;
            let label_cols = 3;
            let label_width = label_cols as f32 * cell_width;
            if slab_width > label_width + 12.0 && root_height > cell_height + 14.0 {
                self.paint_owt_panel_text(
                    &mut top_layers,
                    left + slab_width - label_width - 6.0,
                    root_y + root_height - cell_height - 13.0,
                    label_cols,
                    "OWT",
                    RgbColor::new_8bpc(0, 0, 0),
                    true,
                )?;
            }
            self.ui_items.push(UIItem {
                x: left.max(0.0) as usize,
                y: root_y.max(0.0) as usize,
                width: left_geometry.reserved_width.ceil() as usize,
                height: root_height.ceil() as usize,
                item_type: UIItemType::OwtLcarsWindowChrome,
            });

            if mast_width > 120.0 {
                self.filled_rectangle(
                    &mut top_layers,
                    0,
                    lcars_rect(mast_left, mast_top, mast_width, 7.0),
                    red,
                )?;
                paint_lcars_bar_run(
                    self,
                    &mut top_layers,
                    mast_left,
                    mast_top,
                    mast_width,
                    7.0,
                    &[
                        (red, 0.50),
                        (peach, 0.06),
                        (red, 0.20),
                        (violet, 0.20),
                        (peach, 0.04),
                    ],
                )?;
                paint_lcars_bar_run(
                    self,
                    layers,
                    mast_left,
                    border.top.get() as f32 + tab_bar_height - 7.0,
                    mast_width,
                    5.0,
                    &[(orange, 0.10), (amber, 0.26), (peach, 0.18), (blue, 0.18)],
                )?;
                let trailing_width = (mast_width * 0.10).clamp(42.0, 96.0);
                paint_lcars_rounded_rect(
                    self,
                    &mut top_layers,
                    mast_right - trailing_width,
                    mast_top,
                    trailing_width,
                    7.0,
                    4.0,
                    LCARS_CORNER_RIGHT,
                    blue,
                )?;
            }
        }

        self.filled_rectangle(layers, 0, lcars_rect(left, top, reserved, height), black)?;

        let segment_gap = OWT_LCARS_WINDOW_CHROME_BLACK_GAP;
        let cap_h = (height * 0.07).clamp(18.0, 32.0).min(height * 0.14);
        self.filled_rectangle(layers, 0, lcars_rect(left, top, slab_width, cap_h), red)?;
        self.filled_rectangle(
            layers,
            0,
            lcars_rect(left, top + cap_h + segment_gap, slab_width, 5.0),
            amber,
        )?;

        let footer_h = (height * 0.08).clamp(24.0, 40.0).min(height * 0.14);
        let available_top = top + cap_h + segment_gap + 7.0;
        let available_h = (bottom - footer_h - available_top - segment_gap).max(0.0);
        let segment_colors = [orange, peach, blue, violet];
        for (index, segment) in owt_lcars_window_chrome_body_segment_plan(available_h, segment_gap)
            .into_iter()
            .enumerate()
        {
            let fill = segment_colors[index % segment_colors.len()];
            let y = available_top + segment.offset;
            let width = slab_width * segment.width_factor;
            self.filled_rectangle(layers, 0, lcars_rect(left, y, width, segment.height), fill)?;
        }
        paint_lcars_rounded_rect(
            self,
            layers,
            left,
            bottom - footer_h,
            slab_width,
            footer_h,
            (slab_width * 0.52).min(footer_h * 0.75),
            LCARS_CORNER_BOTTOM_LEFT,
            violet,
        )?;
        self.filled_rectangle(
            layers,
            0,
            lcars_rect(left, bottom - footer_h - 7.0, slab_width, 4.0),
            blue,
        )?;
        self.filled_rectangle(
            layers,
            0,
            lcars_rect(left + slab_width, top, gutter_width, height),
            black,
        )?;

        if owt_lcars_design_grid_enabled() {
            paint_lcars_window_design_grid(self, layers, left, top, slab_width, height)?;
        }

        self.ui_items.push(UIItem {
            x: left.max(0.0) as usize,
            y: top.max(0.0) as usize,
            width: reserved.ceil() as usize,
            height: height.ceil() as usize,
            item_type: UIItemType::OwtLcarsWindowChrome,
        });

        Ok(())
    }

    pub fn paint_fancy_tab_bar(&self) -> anyhow::Result<Vec<UIItem>> {
        let computed = self.fancy_tab_bar.as_ref().ok_or_else(|| {
            anyhow::anyhow!("paint_fancy_tab_bar called but fancy_tab_bar is None")
        })?;
        let ui_items = computed.ui_items();

        let gl_state = self.render_state.as_ref().unwrap();
        self.render_element(&computed, gl_state, None)?;

        Ok(ui_items)
    }
}

fn lcars_capsule_corners() -> Corners {
    Corners {
        top_left: SizedPoly {
            width: Dimension::Cells(0.75),
            height: Dimension::Cells(0.75),
            poly: TOP_LEFT_ROUNDED_CORNER,
        },
        top_right: SizedPoly {
            width: Dimension::Cells(0.75),
            height: Dimension::Cells(0.75),
            poly: TOP_RIGHT_ROUNDED_CORNER,
        },
        bottom_left: SizedPoly {
            width: Dimension::Cells(0.75),
            height: Dimension::Cells(0.75),
            poly: BOTTOM_LEFT_ROUNDED_CORNER,
        },
        bottom_right: SizedPoly {
            width: Dimension::Cells(0.75),
            height: Dimension::Cells(0.75),
            poly: BOTTOM_RIGHT_ROUNDED_CORNER,
        },
    }
}

fn lcars_tab_slab_corners() -> Corners {
    Corners {
        top_left: SizedPoly::none(),
        top_right: SizedPoly {
            width: Dimension::Cells(0.48),
            height: Dimension::Cells(0.48),
            poly: TOP_RIGHT_ROUNDED_CORNER,
        },
        bottom_left: SizedPoly::none(),
        bottom_right: SizedPoly {
            width: Dimension::Cells(0.48),
            height: Dimension::Cells(0.48),
            poly: BOTTOM_RIGHT_ROUNDED_CORNER,
        },
    }
}

fn lcars_tab_active_corners() -> Corners {
    Corners {
        top_left: SizedPoly {
            width: Dimension::Cells(0.66),
            height: Dimension::Cells(0.66),
            poly: TOP_LEFT_ROUNDED_CORNER,
        },
        top_right: SizedPoly::none(),
        bottom_left: SizedPoly {
            width: Dimension::Cells(0.66),
            height: Dimension::Cells(0.66),
            poly: BOTTOM_LEFT_ROUNDED_CORNER,
        },
        bottom_right: SizedPoly::none(),
    }
}

#[derive(Clone, Copy)]
struct LcarsCornerMask {
    top_left: bool,
    top_right: bool,
    bottom_left: bool,
    bottom_right: bool,
}

const LCARS_CORNER_TOP_LEFT: LcarsCornerMask = LcarsCornerMask {
    top_left: true,
    top_right: false,
    bottom_left: false,
    bottom_right: false,
};

const LCARS_CORNER_BOTTOM_LEFT: LcarsCornerMask = LcarsCornerMask {
    top_left: false,
    top_right: false,
    bottom_left: true,
    bottom_right: false,
};

const LCARS_CORNER_RIGHT: LcarsCornerMask = LcarsCornerMask {
    top_left: false,
    top_right: true,
    bottom_left: false,
    bottom_right: true,
};

fn lcars_chrome_color(red: u8, green: u8, blue: u8) -> LinearRgba {
    RgbColor::new_8bpc(red, green, blue).to_linear_tuple_rgba()
}

fn lcars_rect(x: f32, y: f32, width: f32, height: f32) -> window::RectF {
    euclid::rect(x, y, width, height)
}

fn paint_lcars_bar_run(
    window: &mut crate::TermWindow,
    layers: &mut TripleLayerQuadAllocator,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    segments: &[(LinearRgba, f32)],
) -> anyhow::Result<()> {
    if width <= 0.0 || height <= 0.0 || segments.is_empty() {
        return Ok(());
    }

    let gap = OWT_LCARS_WINDOW_CHROME_BLACK_GAP;
    let total_gap = gap * segments.len().saturating_sub(1) as f32;
    let available_width = (width - total_gap).max(1.0);
    let total_weight = segments
        .iter()
        .map(|(_, weight)| weight.max(0.0))
        .sum::<f32>()
        .max(1.0);
    let mut cursor = x;
    for (index, (fill, weight)) in segments.iter().enumerate() {
        let is_last = index + 1 == segments.len();
        let segment_width = if is_last {
            (x + width - cursor).max(1.0)
        } else {
            (available_width * weight.max(0.0) / total_weight).max(1.0)
        };
        window.filled_rectangle(
            layers,
            0,
            lcars_rect(cursor, y, segment_width, height),
            *fill,
        )?;
        cursor += segment_width + gap;
    }
    Ok(())
}

fn paint_lcars_rounded_rect(
    window: &mut crate::TermWindow,
    layers: &mut TripleLayerQuadAllocator,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    radius: f32,
    corners: LcarsCornerMask,
    color: LinearRgba,
) -> anyhow::Result<()> {
    if width <= 0.0 || height <= 0.0 {
        return Ok(());
    }

    let radius = radius.min(width * 0.5).min(height * 0.5).max(0.0);
    if radius < 1.0 {
        window.filled_rectangle(layers, 0, lcars_rect(x, y, width, height), color)?;
        return Ok(());
    }

    let center_width = (width - radius * 2.0).max(0.0);
    if center_width > 0.0 {
        window.filled_rectangle(
            layers,
            0,
            lcars_rect(x + radius, y, center_width, height),
            color,
        )?;
    }

    let side_height = (height - radius * 2.0).max(0.0);
    if side_height > 0.0 {
        window.filled_rectangle(
            layers,
            0,
            lcars_rect(x, y + radius, radius, side_height),
            color,
        )?;
        window.filled_rectangle(
            layers,
            0,
            lcars_rect(x + width - radius, y + radius, radius, side_height),
            color,
        )?;
    }

    paint_lcars_rect_corner(
        window,
        layers,
        x,
        y,
        radius,
        corners.top_left,
        TOP_LEFT_ROUNDED_CORNER,
        color,
    )?;
    paint_lcars_rect_corner(
        window,
        layers,
        x + width - radius,
        y,
        radius,
        corners.top_right,
        TOP_RIGHT_ROUNDED_CORNER,
        color,
    )?;
    paint_lcars_rect_corner(
        window,
        layers,
        x,
        y + height - radius,
        radius,
        corners.bottom_left,
        BOTTOM_LEFT_ROUNDED_CORNER,
        color,
    )?;
    paint_lcars_rect_corner(
        window,
        layers,
        x + width - radius,
        y + height - radius,
        radius,
        corners.bottom_right,
        BOTTOM_RIGHT_ROUNDED_CORNER,
        color,
    )?;

    Ok(())
}

fn paint_lcars_rect_corner(
    window: &mut crate::TermWindow,
    layers: &mut TripleLayerQuadAllocator,
    x: f32,
    y: f32,
    size: f32,
    rounded: bool,
    poly: &'static [Poly],
    color: LinearRgba,
) -> anyhow::Result<()> {
    if rounded {
        window
            .poly_quad(
                layers,
                0,
                euclid::point2(x, y),
                poly,
                1,
                euclid::size2(size, size),
                color,
            )?
            .set_grayscale();
    } else {
        window.filled_rectangle(layers, 0, lcars_rect(x, y, size, size), color)?;
    }
    Ok(())
}

fn owt_lcars_design_grid_enabled() -> bool {
    std::env::var("OWT_LCARS_DESIGN_GRID")
        .map(|value| {
            let value = value.trim().to_ascii_lowercase();
            matches!(value.as_str(), "1" | "true" | "yes" | "on")
        })
        .unwrap_or(false)
}

fn paint_lcars_window_design_grid(
    window: &mut crate::TermWindow,
    layers: &mut TripleLayerQuadAllocator,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
) -> anyhow::Result<()> {
    let cyan = lcars_chrome_color(48, 170, 255);
    let amber = lcars_chrome_color(255, 204, 112);
    let mut gx = x;
    while gx <= x + width {
        window.filled_rectangle(layers, 0, lcars_rect(gx, y, 1.0, height), cyan)?;
        gx += 32.0;
    }
    let mut gy = y;
    while gy <= y + height {
        window.filled_rectangle(layers, 0, lcars_rect(x, gy, width, 1.0), amber)?;
        gy += 32.0;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        owt_lcars_window_chrome_body_segment_plan, owt_lcars_window_chrome_left_geometry,
        owt_lcars_window_chrome_left_reserved_for_cell_width,
        owt_lcars_window_chrome_viewport_ready, OWT_LCARS_WINDOW_CHROME_BLACK_GAP,
    };

    #[test]
    fn lcars_chrome_left_rail_reservation_keeps_daily_shell_gutter() {
        assert_eq!(
            owt_lcars_window_chrome_left_reserved_for_cell_width(8.0),
            64.0
        );
        assert_eq!(
            owt_lcars_window_chrome_left_reserved_for_cell_width(10.0),
            65.0
        );
        assert_eq!(
            owt_lcars_window_chrome_left_reserved_for_cell_width(16.0),
            88.0
        );
    }

    #[test]
    fn lcars_chrome_left_geometry_keeps_top_and_body_edges_aligned() {
        for cell_width in [8.0, 10.0, 16.0] {
            let reserved = owt_lcars_window_chrome_left_reserved_for_cell_width(cell_width);
            let geometry = owt_lcars_window_chrome_left_geometry(reserved, cell_width);

            assert_eq!(geometry.reserved_width, reserved);
            assert!(geometry.rail_width <= geometry.reserved_width);
            assert!(geometry.slab_width <= geometry.reserved_width);
            assert!(geometry.slab_width <= geometry.rail_width);
            assert!(geometry.boundary_mask_width > 0.0);
            assert!(geometry.reserved_width - geometry.slab_width >= geometry.boundary_mask_width);
            assert!(geometry.slab_width <= geometry.rail_width * 0.65);
        }
    }

    #[test]
    fn lcars_chrome_body_segments_are_regular_and_non_overlapping() {
        let gap = 5.0;
        let segments = owt_lcars_window_chrome_body_segment_plan(420.0, gap);

        assert_eq!(segments.len(), 4);
        for pair in segments.windows(2) {
            let previous = pair[0];
            let next = pair[1];
            assert!(previous.offset + previous.height + gap <= next.offset + 0.01);
        }
        for segment in segments {
            assert!(segment.width_factor <= 1.0);
            assert!(segment.width_factor >= 0.55);
            assert!(segment.height > 0.0);
        }
    }

    #[test]
    fn lcars_chrome_body_segments_leave_black_breathing_room() {
        let gap = OWT_LCARS_WINDOW_CHROME_BLACK_GAP;
        let available_height = 200.0;
        let segments = owt_lcars_window_chrome_body_segment_plan(available_height, gap);
        let painted_height = segments.iter().map(|segment| segment.height).sum::<f32>();
        let last_bottom = segments
            .last()
            .map(|segment| segment.offset + segment.height)
            .unwrap_or_default();

        assert_eq!(segments.len(), 4);
        assert!(painted_height < available_height * 0.72);
        assert!(last_bottom < available_height * 0.82);
        assert!(segments.iter().any(|segment| segment.width_factor < 0.75));
    }

    #[test]
    fn lcars_chrome_disables_below_minimal_viewport() {
        assert!(!owt_lcars_window_chrome_viewport_ready(319.0, 220.0));
        assert!(!owt_lcars_window_chrome_viewport_ready(360.0, 179.0));
        assert!(owt_lcars_window_chrome_viewport_ready(360.0, 220.0));
    }
}
