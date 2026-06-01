use crate::quad::TripleLayerQuadAllocator;
use crate::termwindow::render::RenderScreenLineParams;
use crate::termwindow::{
    MouseCapture, OwtLcarsSurfaceMenuMode, OwtLcarsSurfaceMenuState, UIItem, UIItemType,
};
use anyhow::Context;
use mux::renderable::{RenderableDimensions, StableCursorPosition};
use owt_control::{
    FactProvenance, FactState, InterfaceDocument, Scope, ScopeKind, TableCellFocusMovement,
    TableFocusMovement, UiNode, UiNodeKind,
};
use serde::Serialize;
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::process::Command;
use std::sync::Arc;
use termwiz::cell::Intensity;
use termwiz::color::{ColorSpec, RgbColor};
use termwiz::image::{ImageCell, ImageData, ImageDataType, TextureCoordinate};
use wezterm_term::{CellAttributes, Line};
use window::{
    color::LinearRgba, KeyCode, KeyEvent, Modifiers, MouseCursor, MouseEvent, MouseEventKind,
    MousePress, PhysKeyCode, PixelUnit, RectF, WindowOps,
};

const LCARS_PANEL_MARGIN: f32 = 8.0;
const LCARS_PANEL_GAP: f32 = 12.0;
const LCARS_PANEL_MIN_HEIGHT: f32 = 166.0;
const LCARS_PANEL_MAX_HEIGHT: f32 = 196.0;
const LCARS_ACTION_STRIP_MIN_HEIGHT: f32 = 54.0;
const LCARS_ACTION_STRIP_MAX_HEIGHT: f32 = 82.0;
const LCARS_STRUCTURAL_PANEL_MIN_HEIGHT: f32 = 352.0;
const LCARS_STRUCTURAL_PANEL_MAX_HEIGHT: f32 = 448.0;
const LCARS_DOCKED_SURFACE_PANEL_MIN_HEIGHT: f32 = 256.0;
const LCARS_DOCKED_SURFACE_PANEL_MAX_HEIGHT: f32 = 328.0;
const LCARS_DOCKED_SURFACE_PANEL_ROW_HEIGHT: f32 = 14.8;
const LCARS_THELCARS_PANEL_MIN_HEIGHT: f32 = 438.0;
const LCARS_THELCARS_PANEL_MAX_HEIGHT: f32 = 540.0;
const LCARS_THELCARS_PANEL_ROW_HEIGHT: f32 = 23.2;
const LCARS_THELCARS_SIDE_PANEL_MIN_WIDTH: f32 = 360.0;
const LCARS_THELCARS_SIDE_PANEL_MAX_WIDTH: f32 = 520.0;
const LCARS_THELCARS_BOTTOM_PANEL_MIN_HEIGHT: f32 = 220.0;
const LCARS_THELCARS_BOTTOM_PANEL_MAX_HEIGHT: f32 = 300.0;
const LCARS_THELCARS_BOTTOM_PANEL_ROW_HEIGHT: f32 = 12.0;
const LCARS_PANEL_ROW_HEIGHT: f32 = 8.2;
const LCARS_STRUCTURAL_PANEL_ROW_HEIGHT: f32 = 20.0;
const LCARS_LEFT_RAIL_RESERVED: f32 = 196.0;
const LCARS_SIDE_PANEL_MIN_WIDTH: f32 = 300.0;
const LCARS_SIDE_PANEL_MAX_WIDTH: f32 = 440.0;
const LCARS_BLOCK_SIDE_PANEL_MIN_WIDTH: f32 = 360.0;
const LCARS_BLOCK_SIDE_PANEL_MAX_WIDTH: f32 = 560.0;
const LCARS_BOTTOM_PANEL_MIN_HEIGHT: f32 = 132.0;
const LCARS_BOTTOM_PANEL_MAX_HEIGHT: f32 = 176.0;
const LCARS_BOTTOM_PANEL_ROW_HEIGHT: f32 = 7.0;
const LCARS_BLOCK_BOTTOM_PANEL_MIN_HEIGHT: f32 = 204.0;
const LCARS_BLOCK_BOTTOM_PANEL_MAX_HEIGHT: f32 = 276.0;
const LCARS_BLOCK_BOTTOM_PANEL_ROW_HEIGHT: f32 = 11.2;
const LCARS_MIN_TERMINAL_REMAINDER: f32 = 420.0;
const LCARS_MAX_PANEL_LINES: usize = 18;
const LCARS_KEY_ACTION_LIMIT: usize = 9;
const LCARS_SIGNAL_TEXT_MAX_LINES: usize = 2;
const LCARS_PRIMITIVE_LEGEND_COUNT: usize = 20;
const LCARS_SURFACE_MENU_MAX_SLOTS: usize = 8;
const LCARS_NATIVE_SURFACE_MIN_WIDTH: f32 = 320.0;
const LCARS_NATIVE_SURFACE_MIN_HEIGHT: f32 = 180.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LcarsPanelLayout {
    Top,
    Left,
    Right,
    Bottom,
    Overlay,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LcarsSurfaceOrigin {
    TopLeft,
    Top,
    Left,
    Right,
    Bottom,
    BottomRight,
    Overlay,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LcarsSurfaceReservation {
    Reserved,
    Overlay,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LcarsSurfaceOrientation {
    Horizontal,
    Vertical,
    Auto,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct LcarsSurfacePlacement {
    pub(crate) origin: LcarsSurfaceOrigin,
    pub(crate) layout: LcarsPanelLayout,
    pub(crate) reservation: LcarsSurfaceReservation,
    pub(crate) orientation: LcarsSurfaceOrientation,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct LcarsRenderProjection {
    pub(crate) visible: bool,
    pub(crate) renderer_path: &'static str,
    pub(crate) layout: &'static str,
    pub(crate) origin: &'static str,
    pub(crate) reservation: &'static str,
    pub(crate) orientation: &'static str,
    pub(crate) reserves_terminal_space: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) anchor: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) z_order: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) priority: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) min_terminal_cells: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) collapse_policy: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) floating_anchor: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) floating_x: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) floating_y: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) floating_width: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) floating_height: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) structural_profile: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) requested_profile: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) profile_family: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) palette_profile: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) table_density: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) cohort_key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) state_key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) severity_key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) lifecycle_controls: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) action_roles: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) refresh_policy: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) drilldown_policy: Option<String>,
}

#[derive(Debug, Clone, Copy)]
struct LcarsFloatingGeometry {
    left: f32,
    top: f32,
    width: f32,
    height: f32,
}

impl LcarsSurfacePlacement {
    const fn from_layout(layout: LcarsPanelLayout) -> Self {
        let origin = match layout {
            LcarsPanelLayout::Top => LcarsSurfaceOrigin::TopLeft,
            LcarsPanelLayout::Left => LcarsSurfaceOrigin::Left,
            LcarsPanelLayout::Right => LcarsSurfaceOrigin::Right,
            LcarsPanelLayout::Bottom => LcarsSurfaceOrigin::Bottom,
            LcarsPanelLayout::Overlay => LcarsSurfaceOrigin::Overlay,
        };
        Self {
            origin,
            layout,
            reservation: reservation_for_layout(layout),
            orientation: orientation_for_layout(layout),
        }
    }

    const fn from_origin(origin: LcarsSurfaceOrigin) -> Self {
        let layout = layout_for_origin(origin);
        Self {
            origin,
            layout,
            reservation: reservation_for_layout(layout),
            orientation: orientation_for_layout(layout),
        }
    }

    const fn reserves_terminal_space(self) -> bool {
        matches!(self.reservation, LcarsSurfaceReservation::Reserved)
            && !matches!(self.layout, LcarsPanelLayout::Overlay)
    }
}

const fn layout_for_origin(origin: LcarsSurfaceOrigin) -> LcarsPanelLayout {
    match origin {
        LcarsSurfaceOrigin::TopLeft | LcarsSurfaceOrigin::Top => LcarsPanelLayout::Top,
        LcarsSurfaceOrigin::Left => LcarsPanelLayout::Left,
        LcarsSurfaceOrigin::Right => LcarsPanelLayout::Right,
        LcarsSurfaceOrigin::Bottom | LcarsSurfaceOrigin::BottomRight => LcarsPanelLayout::Bottom,
        LcarsSurfaceOrigin::Overlay => LcarsPanelLayout::Overlay,
    }
}

const fn orientation_for_layout(layout: LcarsPanelLayout) -> LcarsSurfaceOrientation {
    match layout {
        LcarsPanelLayout::Left | LcarsPanelLayout::Right => LcarsSurfaceOrientation::Vertical,
        LcarsPanelLayout::Overlay => LcarsSurfaceOrientation::Auto,
        LcarsPanelLayout::Top | LcarsPanelLayout::Bottom => LcarsSurfaceOrientation::Horizontal,
    }
}

const fn reservation_for_layout(layout: LcarsPanelLayout) -> LcarsSurfaceReservation {
    match layout {
        LcarsPanelLayout::Overlay => LcarsSurfaceReservation::Overlay,
        _ => LcarsSurfaceReservation::Reserved,
    }
}

#[derive(Debug, Default, Clone, Copy)]
struct LcarsReservedPixels {
    left: f32,
    top: f32,
    right: f32,
    bottom: f32,
}

fn lcars_compact_button_reserved_pixels(button_height: f32) -> f32 {
    LCARS_PANEL_MARGIN + 8.0 + button_height + LCARS_PANEL_GAP
}

fn lcars_action_strip_side_width_from_metrics(
    cell_width: f32,
    longest_action_chars: usize,
    requested_width: Option<f32>,
    max_without_starving_terminal: f32,
) -> Option<f32> {
    let rail_width = (cell_width * 7.0).clamp(66.0, 92.0);
    let target_button_width =
        ((longest_action_chars as f32 + 2.0) * cell_width + 42.0).clamp(154.0, 248.0);
    let min_width = (rail_width + 16.0 + cell_width * 8.0).clamp(220.0, 260.0);
    let preferred_width = (rail_width + 16.0 + target_button_width).clamp(min_width, 360.0);
    if max_without_starving_terminal < min_width {
        return None;
    }
    Some(
        requested_width
            .unwrap_or(preferred_width)
            .clamp(min_width, max_without_starving_terminal.min(360.0)),
    )
}

fn owt_lcars_native_surface_viewport_ready(width: f32, height: f32) -> bool {
    width >= LCARS_NATIVE_SURFACE_MIN_WIDTH && height >= LCARS_NATIVE_SURFACE_MIN_HEIGHT
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct LcarsRenderSceneSummary {
    active_count: usize,
    reserved_count: usize,
    overlay_count: usize,
    slots: Vec<LcarsRenderSceneSlotSummary>,
    conflict_hints: Vec<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct LcarsRenderSceneSlotSummary {
    slot: &'static str,
    active_count: usize,
    reserved_count: usize,
    interface_ids: Vec<String>,
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum LcarsPaletteProfile {
    Classic,
    BrightClassic,
    ScienceStation,
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
const LCARS_BYTE_ORANGE: LcarsByteColor = LcarsByteColor::rgb(255, 136, 0);
const LCARS_BYTE_AMBER: LcarsByteColor = LcarsByteColor::rgb(255, 204, 112);
const LCARS_BYTE_PEACH: LcarsByteColor = LcarsByteColor::rgb(255, 149, 96);
const LCARS_BYTE_VIOLET: LcarsByteColor = LcarsByteColor::rgb(197, 143, 255);
const LCARS_BYTE_BLUE: LcarsByteColor = LcarsByteColor::rgb(137, 148, 255);
const LCARS_BYTE_CYAN: LcarsByteColor = LcarsByteColor::rgb(164, 212, 255);
const LCARS_BYTE_RED: LcarsByteColor = LcarsByteColor::rgb(207, 79, 79);

impl LcarsPalette {
    fn new() -> Self {
        Self::profile(LcarsPaletteProfile::Classic)
    }

    fn for_document(document: &InterfaceDocument) -> Self {
        Self::profile(lcars_palette_profile(document))
    }

    fn profile(profile: LcarsPaletteProfile) -> Self {
        match profile {
            LcarsPaletteProfile::Classic => Self::from_bytes(
                LCARS_BYTE_ORANGE,
                LCARS_BYTE_AMBER,
                LCARS_BYTE_PEACH,
                LCARS_BYTE_VIOLET,
                LCARS_BYTE_BLUE,
                LCARS_BYTE_CYAN,
                LCARS_BYTE_RED,
                0.76,
                0.80,
            ),
            LcarsPaletteProfile::BrightClassic => Self::from_bytes(
                LcarsByteColor::rgb(255, 128, 24),
                LcarsByteColor::rgb(255, 219, 92),
                LcarsByteColor::rgb(255, 176, 112),
                LcarsByteColor::rgb(204, 136, 255),
                LcarsByteColor::rgb(128, 164, 255),
                LcarsByteColor::rgb(132, 224, 255),
                LcarsByteColor::rgb(238, 72, 72),
                0.82,
                0.84,
            ),
            LcarsPaletteProfile::ScienceStation => Self::from_bytes(
                LcarsByteColor::rgb(192, 96, 72),
                LcarsByteColor::rgb(205, 176, 104),
                LcarsByteColor::rgb(214, 126, 96),
                LcarsByteColor::rgb(142, 122, 188),
                LcarsByteColor::rgb(92, 128, 180),
                LcarsByteColor::rgb(118, 174, 188),
                LcarsByteColor::rgb(174, 72, 78),
                0.68,
                0.72,
            ),
        }
    }

    fn from_bytes(
        orange_byte: LcarsByteColor,
        amber_byte: LcarsByteColor,
        peach_byte: LcarsByteColor,
        violet_byte: LcarsByteColor,
        blue_byte: LcarsByteColor,
        cyan_byte: LcarsByteColor,
        red_byte: LcarsByteColor,
        dim_blue_alpha: f32,
        dim_violet_alpha: f32,
    ) -> Self {
        let black = color(0, 0, 0);
        let blue = color(blue_byte.red, blue_byte.green, blue_byte.blue);
        let violet = color(violet_byte.red, violet_byte.green, violet_byte.blue);
        Self {
            black,
            orange: color(orange_byte.red, orange_byte.green, orange_byte.blue),
            amber: color(amber_byte.red, amber_byte.green, amber_byte.blue),
            peach: color(peach_byte.red, peach_byte.green, peach_byte.blue),
            violet,
            blue,
            cyan: color(cyan_byte.red, cyan_byte.green, cyan_byte.blue),
            red: color(red_byte.red, red_byte.green, red_byte.blue),
            dim_blue: with_alpha(blue, dim_blue_alpha),
            dim_violet: with_alpha(violet, dim_violet_alpha),
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

#[derive(Clone, Copy)]
enum LcarsSurfaceMenuAction {
    LoadSaved,
    Show,
    DockRightRail,
    DockBottomRight,
    DockBottomStrip,
    DockLeftRail,
    Hide,
    DockTopLeft,
}

#[derive(Clone, Copy)]
struct LcarsSurfaceMenuEntry {
    label: &'static str,
    detail: &'static str,
    action: LcarsSurfaceMenuAction,
}

const LCARS_SURFACE_MENU_ENTRIES: &[LcarsSurfaceMenuEntry] = &[
    LcarsSurfaceMenuEntry {
        label: "LOAD",
        detail: "LOAD SAVED LCARS",
        action: LcarsSurfaceMenuAction::LoadSaved,
    },
    LcarsSurfaceMenuEntry {
        label: "SHOW",
        detail: "RESTORE CURRENT",
        action: LcarsSurfaceMenuAction::Show,
    },
    LcarsSurfaceMenuEntry {
        label: "RIGHT",
        detail: "DOCK RIGHT RAIL",
        action: LcarsSurfaceMenuAction::DockRightRail,
    },
    LcarsSurfaceMenuEntry {
        label: "B-RIGHT",
        detail: "DOCK BOTTOM RIGHT",
        action: LcarsSurfaceMenuAction::DockBottomRight,
    },
    LcarsSurfaceMenuEntry {
        label: "BOTTOM",
        detail: "DOCK BOTTOM STRIP",
        action: LcarsSurfaceMenuAction::DockBottomStrip,
    },
    LcarsSurfaceMenuEntry {
        label: "LEFT",
        detail: "DOCK LEFT RAIL",
        action: LcarsSurfaceMenuAction::DockLeftRail,
    },
    LcarsSurfaceMenuEntry {
        label: "HIDE",
        detail: "HIDE CURRENT",
        action: LcarsSurfaceMenuAction::Hide,
    },
    LcarsSurfaceMenuEntry {
        label: "TOP",
        detail: "DOCK TOP LEFT",
        action: LcarsSurfaceMenuAction::DockTopLeft,
    },
];

#[derive(Clone)]
enum LcarsSavedMenuSlot {
    OwnerFilter,
    Interface(crate::owt_native::SavedInterfaceSummary),
    PreviousPage,
    NextPage,
}

enum LcarsSurfaceMenuCommand {
    OpenLoadSaved,
    Patch(Option<String>, BTreeMap<String, String>),
    Load(String),
    CycleOwnerFilter,
    PreviousPage,
    NextPage,
    BackToMain,
    Close,
    None,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct LcarsSavedOwnerOption {
    key: String,
    label: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum LcarsSurfaceMenuHit {
    Wedge(usize),
    Center,
    Background,
}

#[derive(Clone)]
struct LcarsSurfaceMenuSlot {
    label: String,
    detail: String,
    saved: Option<LcarsSavedMenuSlot>,
}

#[derive(Clone, Copy)]
struct LcarsSurfaceMenuLayout {
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    command_x: f32,
    command_y: f32,
    command_width: f32,
    slot_rects: [LcarsSceneRect; LCARS_SURFACE_MENU_MAX_SLOTS],
    status_x: f32,
    status_y: f32,
    status_width: f32,
    status_height: f32,
    close_x: f32,
    close_y: f32,
    close_width: f32,
    close_height: f32,
    anchor_x: f32,
    anchor_y: f32,
}

fn lcars_surface_menu_slot_rects(
    mode: OwtLcarsSurfaceMenuMode,
    command_x: f32,
    command_y: f32,
    command_width: f32,
    command_height: f32,
    command_gap: f32,
) -> [LcarsSceneRect; LCARS_SURFACE_MENU_MAX_SLOTS] {
    let column_gap = 10.0;
    let column_width = ((command_width - column_gap) * 0.5).max(72.0);
    let mut slots = [LcarsSceneRect {
        x: command_x,
        y: command_y,
        width: column_width,
        height: command_height,
    }; LCARS_SURFACE_MENU_MAX_SLOTS];

    match mode {
        OwtLcarsSurfaceMenuMode::Main => {
            let placements = [
                (0_usize, 0_usize), // LOAD
                (0, 1),             // SHOW
                (1, 2),             // RIGHT
                (1, 4),             // B-RIGHT
                (1, 3),             // BOTTOM
                (1, 1),             // LEFT
                (0, 2),             // HIDE
                (1, 0),             // TOP
            ];
            for (index, (column, row)) in placements.iter().copied().enumerate() {
                slots[index] = LcarsSceneRect {
                    x: command_x + column as f32 * (column_width + column_gap),
                    y: command_y + row as f32 * (command_height + command_gap),
                    width: column_width,
                    height: command_height,
                };
            }
        }
        OwtLcarsSurfaceMenuMode::LoadSaved => {
            for (index, slot) in slots.iter_mut().enumerate() {
                let column = index / 4;
                let row = index % 4;
                *slot = LcarsSceneRect {
                    x: command_x + column as f32 * (column_width + column_gap),
                    y: command_y + row as f32 * (command_height + command_gap),
                    width: column_width,
                    height: command_height,
                };
            }
        }
    }

    slots
}

fn lcars_surface_menu_properties(
    action: LcarsSurfaceMenuAction,
) -> Option<BTreeMap<String, String>> {
    let pairs: &[(&str, &str)] = match action {
        LcarsSurfaceMenuAction::Hide => &[
            ("visible", "false"),
            ("hidden", "true"),
            ("display", "none"),
        ],
        LcarsSurfaceMenuAction::Show => &[
            ("active", "true"),
            ("visible", "true"),
            ("enabled", "true"),
            ("hidden", "false"),
            ("display", "block"),
        ],
        LcarsSurfaceMenuAction::LoadSaved => return None,
        LcarsSurfaceMenuAction::DockTopLeft => &[
            ("active", "true"),
            ("layout", "docked_top"),
            ("placement", "top-left"),
            ("origin", "top-left"),
            ("dock", "top"),
            ("orientation", "horizontal"),
            ("reservation", "reserved"),
            ("reserve", "true"),
            ("visible", "true"),
            ("enabled", "true"),
            ("hidden", "false"),
            ("display", "block"),
        ],
        LcarsSurfaceMenuAction::DockLeftRail => &[
            ("active", "true"),
            ("layout", "left_rail"),
            ("placement", "left"),
            ("origin", "left"),
            ("dock", "left_rail"),
            ("orientation", "vertical"),
            ("reservation", "reserved"),
            ("reserve", "true"),
            ("visible", "true"),
            ("enabled", "true"),
            ("hidden", "false"),
            ("display", "block"),
        ],
        LcarsSurfaceMenuAction::DockRightRail => &[
            ("active", "true"),
            ("layout", "right_rail"),
            ("placement", "right"),
            ("origin", "right"),
            ("dock", "right_rail"),
            ("orientation", "vertical"),
            ("reservation", "reserved"),
            ("reserve", "true"),
            ("visible", "true"),
            ("enabled", "true"),
            ("hidden", "false"),
            ("display", "block"),
        ],
        LcarsSurfaceMenuAction::DockBottomStrip => &[
            ("active", "true"),
            ("layout", "bottom_strip"),
            ("placement", "bottom"),
            ("origin", "bottom"),
            ("dock", "bottom_strip"),
            ("orientation", "horizontal"),
            ("reservation", "reserved"),
            ("reserve", "true"),
            ("visible", "true"),
            ("enabled", "true"),
            ("hidden", "false"),
            ("display", "block"),
        ],
        LcarsSurfaceMenuAction::DockBottomRight => &[
            ("active", "true"),
            ("layout", "bottom_strip"),
            ("placement", "bottom-right"),
            ("origin", "bottom-right"),
            ("dock", "bottom_right"),
            ("orientation", "horizontal"),
            ("reservation", "reserved"),
            ("reserve", "true"),
            ("visible", "true"),
            ("enabled", "true"),
            ("hidden", "false"),
            ("display", "block"),
        ],
    };

    Some(
        pairs
            .iter()
            .map(|(key, value)| ((*key).to_string(), (*value).to_string()))
            .collect(),
    )
}

pub(crate) fn owt_lcars_drag_drop_layout_properties(
    x: f32,
    y: f32,
    width: f32,
    height: f32,
) -> BTreeMap<String, String> {
    if let Some(action) = owt_lcars_drag_drop_action(x, y, width, height) {
        return lcars_surface_menu_properties(action).unwrap_or_default();
    }

    let width = width.max(1.0);
    let height = height.max(1.0);
    let x = x.clamp(0.0, width);
    let y = y.clamp(0.0, height);
    let mut properties = BTreeMap::from([
        ("active".to_string(), "true".to_string()),
        ("placement".to_string(), "free".to_string()),
        ("layout".to_string(), "overlay".to_string()),
        ("origin".to_string(), "overlay".to_string()),
        ("dock".to_string(), "overlay".to_string()),
        ("orientation".to_string(), "auto".to_string()),
        ("reservation".to_string(), "overlay".to_string()),
        ("reserve".to_string(), "overlay".to_string()),
        ("allow_terminal_overlay".to_string(), "true".to_string()),
        ("visible".to_string(), "true".to_string()),
        ("enabled".to_string(), "true".to_string()),
        ("hidden".to_string(), "false".to_string()),
        ("display".to_string(), "block".to_string()),
        ("floating_anchor".to_string(), "center".to_string()),
        ("floating_x".to_string(), format!("{x:.0}")),
        ("floating_y".to_string(), format!("{y:.0}")),
    ]);
    let floating_width = (width * 0.54).clamp(420.0, 900.0).min(width);
    let floating_height = (height * 0.46).clamp(220.0, 520.0).min(height);
    properties.insert("floating_width".to_string(), format!("{floating_width:.0}"));
    properties.insert(
        "floating_height".to_string(),
        format!("{floating_height:.0}"),
    );
    properties
}

fn owt_lcars_drag_drop_action(
    x: f32,
    y: f32,
    width: f32,
    height: f32,
) -> Option<LcarsSurfaceMenuAction> {
    let width = width.max(1.0);
    let height = height.max(1.0);
    let x = x.clamp(0.0, width);
    let y = y.clamp(0.0, height);
    let edge_threshold = (width.min(height) * 0.24).clamp(72.0, 220.0);
    if width - x <= edge_threshold && height - y <= edge_threshold {
        return Some(LcarsSurfaceMenuAction::DockBottomRight);
    }
    let distances = [
        (x, LcarsSurfaceMenuAction::DockLeftRail),
        (width - x, LcarsSurfaceMenuAction::DockRightRail),
        (y, LcarsSurfaceMenuAction::DockTopLeft),
        (height - y, LcarsSurfaceMenuAction::DockBottomStrip),
    ];
    let (distance, action) = distances
        .iter()
        .copied()
        .min_by(|left, right| left.0.total_cmp(&right.0))
        .unwrap();
    if distance <= edge_threshold {
        Some(action)
    } else {
        None
    }
}

pub(crate) fn owt_lcars_drag_drop_target_text(
    x: f32,
    y: f32,
    width: f32,
    height: f32,
) -> (&'static str, &'static str) {
    match owt_lcars_drag_drop_action(x, y, width, height) {
        Some(LcarsSurfaceMenuAction::DockLeftRail) => ("LEFT", "DOCK LEFT RAIL"),
        Some(LcarsSurfaceMenuAction::DockRightRail) => ("RIGHT", "DOCK RIGHT RAIL"),
        Some(LcarsSurfaceMenuAction::DockTopLeft) => ("TOP", "DOCK TOP LEFT"),
        Some(LcarsSurfaceMenuAction::DockBottomStrip) => ("BOTTOM", "DOCK BOTTOM STRIP"),
        Some(LcarsSurfaceMenuAction::DockBottomRight) => ("B-RIGHT", "DOCK BOTTOM RIGHT"),
        _ => ("FREE", "FLOAT OVERLAY"),
    }
}

pub(crate) fn owt_lcars_resize_layout_properties(
    panel_left: f32,
    panel_top: f32,
    x: f32,
    y: f32,
    viewport_width: f32,
    viewport_height: f32,
) -> BTreeMap<String, String> {
    let viewport_width = viewport_width.max(1.0);
    let viewport_height = viewport_height.max(1.0);
    let margin = LCARS_PANEL_MARGIN;
    let left = panel_left.clamp(margin, (viewport_width - margin).max(margin));
    let top = panel_top.clamp(margin, (viewport_height - margin).max(margin));
    let max_width = (viewport_width - left - margin).max(1.0);
    let max_height = (viewport_height - top - margin).max(1.0);
    let min_width = 320.0_f32.min(max_width);
    let min_height = 166.0_f32.min(max_height);
    let floating_width = clamp_ordered(x - left, min_width, max_width);
    let floating_height = clamp_ordered(y - top, min_height, max_height);

    BTreeMap::from([
        ("active".to_string(), "true".to_string()),
        ("placement".to_string(), "free".to_string()),
        ("layout".to_string(), "overlay".to_string()),
        ("origin".to_string(), "overlay".to_string()),
        ("dock".to_string(), "overlay".to_string()),
        ("orientation".to_string(), "auto".to_string()),
        ("reservation".to_string(), "overlay".to_string()),
        ("reserve".to_string(), "overlay".to_string()),
        ("allow_terminal_overlay".to_string(), "true".to_string()),
        ("visible".to_string(), "true".to_string()),
        ("enabled".to_string(), "true".to_string()),
        ("hidden".to_string(), "false".to_string()),
        ("display".to_string(), "block".to_string()),
        ("floating_anchor".to_string(), "top_left".to_string()),
        ("floating_x".to_string(), format!("{left:.0}")),
        ("floating_y".to_string(), format!("{top:.0}")),
        ("floating_width".to_string(), format!("{floating_width:.0}")),
        (
            "floating_height".to_string(),
            format!("{floating_height:.0}"),
        ),
    ])
}

fn owt_lcars_click_sound_enabled() -> bool {
    std::env::var("OWT_UI_SOUND")
        .map(|value| {
            let value = value.trim().to_ascii_lowercase();
            !matches!(value.as_str(), "0" | "false" | "no" | "off")
        })
        .unwrap_or(true)
}

fn owt_lcars_audio_players() -> Vec<String> {
    if let Ok(player) = std::env::var("OWT_LCARS_CLICK_PLAYER") {
        if !player.trim().is_empty() {
            return vec![player];
        }
    }

    if cfg!(windows) {
        vec![
            "vlc.exe".to_string(),
            r"C:\Program Files\VideoLAN\VLC\vlc.exe".to_string(),
            r"C:\Program Files (x86)\VideoLAN\VLC\vlc.exe".to_string(),
        ]
    } else {
        vec![
            "vlc".to_string(),
            "cvlc".to_string(),
            "ffplay".to_string(),
            "mpg123".to_string(),
        ]
    }
}

fn owt_lcars_existing_click_sound() -> Option<PathBuf> {
    let mut candidates = Vec::new();
    if let Ok(sound) = std::env::var("OWT_LCARS_CLICK_SOUND") {
        push_owt_lcars_sound_candidate(&mut candidates, sound);
    }
    if let Ok(template_dir) = std::env::var("OWT_THELCARS_TEMPLATE_DIR") {
        push_owt_lcars_template_sound_candidates(&mut candidates, template_dir);
    }
    push_owt_lcars_template_sound_candidates(
        &mut candidates,
        "/home/buanzo/git/tools/rust/owt/private/thelcars-template",
    );

    let existing: Vec<_> = candidates
        .into_iter()
        .filter(|candidate| candidate.is_file())
        .collect();
    if existing.is_empty() {
        None
    } else {
        Some(existing[fastrand::usize(0..existing.len())].clone())
    }
}

fn push_owt_lcars_template_sound_candidates(
    candidates: &mut Vec<PathBuf>,
    template_dir: impl AsRef<str>,
) {
    let template_dir = template_dir.as_ref().trim();
    if template_dir.is_empty() {
        return;
    }
    for name in ["beep1.mp3", "beep2.mp3", "beep3.mp3", "beep4.mp3"] {
        push_owt_lcars_sound_candidate(candidates, format!("{template_dir}/assets/{name}"));
    }
}

fn push_owt_lcars_sound_candidate(candidates: &mut Vec<PathBuf>, path: impl AsRef<str>) {
    let path = path.as_ref().trim();
    if path.is_empty() {
        return;
    }
    candidates.push(PathBuf::from(path));
    if let Some(mapped) = owt_lcars_windows_wsl_path(path) {
        candidates.push(mapped);
    }
}

#[cfg(windows)]
fn owt_lcars_windows_wsl_path(path: &str) -> Option<PathBuf> {
    if !path.starts_with('/') {
        return None;
    }
    let distro = std::env::var("OWT_LCARS_WSL_DISTRO")
        .or_else(|_| std::env::var("WSL_DISTRO_NAME"))
        .unwrap_or_else(|_| "Ubuntu".to_string());
    let suffix = path.trim_start_matches('/').replace('/', r"\");
    Some(PathBuf::from(format!(r"\\wsl.localhost\{distro}\{suffix}")))
}

#[cfg(not(windows))]
fn owt_lcars_windows_wsl_path(_path: &str) -> Option<PathBuf> {
    None
}

impl crate::TermWindow {
    pub fn owt_lcars_reserved_left_pixels(&self) -> f32 {
        if self.owt_lcars_surface_menu.is_some() {
            return self.owt_lcars_window_chrome_left_reserved_pixels();
        }

        self.owt_lcars_reserved_pixels()
            .left
            .max(self.owt_lcars_window_chrome_left_reserved_pixels())
    }

    pub fn owt_lcars_reserved_top_pixels(&self) -> f32 {
        let mut top = if self.owt_lcars_surface_menu.is_some() {
            0.0
        } else {
            self.owt_lcars_reserved_pixels().top
        };
        if let Some(layout) = self.owt_lcars_surface_menu_layout() {
            top = top.max(layout.y + layout.height + LCARS_PANEL_GAP);
        }
        top
    }

    pub fn owt_lcars_reserved_right_pixels(&self) -> f32 {
        if self.owt_lcars_surface_menu.is_some() {
            return 0.0;
        }

        self.owt_lcars_reserved_pixels().right
    }

    pub fn owt_lcars_reserved_bottom_pixels(&self) -> f32 {
        if self.owt_lcars_surface_menu.is_some() {
            return 0.0;
        }

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

    pub(crate) fn dispatch_owt_lcars_action(
        &mut self,
        interface_id: &str,
        action_id: &str,
        context: &dyn WindowOps,
    ) {
        self.play_owt_lcars_private_click_sound();
        match crate::owt_native::dispatch_action(Some(interface_id), action_id) {
            Ok(dispatched) => {
                log::debug!(
                    "OWT LCARS action dispatched for {}: {}",
                    interface_id,
                    dispatched.action_id
                );
                context.invalidate();
            }
            Err(err) => {
                log::warn!(
                    "OWT LCARS action dispatch failed for {interface_id}/{action_id}: {err:#}"
                );
            }
        }
    }

    pub(crate) fn decide_owt_lcars_permission(
        &mut self,
        request_id: &str,
        allow: bool,
        context: &dyn WindowOps,
    ) {
        self.play_owt_lcars_private_click_sound();
        let result = if allow {
            crate::owt_native::approve_permission_request(request_id)
        } else {
            crate::owt_native::deny_permission_request(request_id)
        };
        match result {
            Ok(decision) => {
                log::debug!(
                    "OWT LCARS permission decision for {}: {:?}",
                    request_id,
                    decision.approval_state
                );
                context.invalidate();
            }
            Err(err) => {
                log::warn!("OWT LCARS permission decision failed for {request_id}: {err:#}");
            }
        }
    }

    pub(crate) fn focus_owt_lcars_table_cell(
        &mut self,
        interface_id: &str,
        column: &str,
        context: &dyn WindowOps,
    ) {
        self.play_owt_lcars_private_click_sound();
        match crate::owt_native::focus_table_cell(Some(interface_id), column) {
            Ok(focused) => {
                log::debug!(
                    "OWT LCARS table cell focused for {}: {} -> {}",
                    interface_id,
                    focused.table_node_id,
                    focused.focused_column
                );
                context.invalidate();
            }
            Err(err) => {
                log::warn!(
                    "OWT LCARS table cell focus failed for {interface_id}/{column}: {err:#}"
                );
            }
        }
    }

    pub(crate) fn play_owt_lcars_private_click_sound(&self) {
        if !owt_lcars_click_sound_enabled() {
            return;
        }

        std::thread::spawn(|| {
            let Some(sound) = owt_lcars_existing_click_sound() else {
                return;
            };
            for player in owt_lcars_audio_players() {
                let mut command = Command::new(&player);
                if cfg!(windows) {
                    command.args(["--intf", "dummy", "--play-and-exit", "--no-video"]);
                } else if player.contains("ffplay") {
                    command.args(["-nodisp", "-autoexit", "-loglevel", "quiet"]);
                } else if player.contains("mpg123") {
                    command.arg("-q");
                } else {
                    command.args(["--intf", "dummy", "--play-and-exit", "--no-video"]);
                }
                command.arg(&sound);
                if command.spawn().is_ok() {
                    return;
                }
            }
        });
    }

    pub(crate) fn patch_owt_lcars_layout_properties_for_interface(
        &mut self,
        interface_id: Option<String>,
        properties: BTreeMap<String, String>,
    ) {
        match crate::owt_native::patch_interface_layout_properties(
            interface_id.as_deref(),
            properties,
        ) {
            Ok(patched) => {
                log::debug!(
                    "OWT LCARS layout patched on node {}: {:?}",
                    patched.node_id,
                    patched.applied_properties
                );
                self.reflow_owt_lcars_layout();
            }
            Err(err) => {
                log::warn!("OWT LCARS layout patch failed: {err:#}");
            }
        }
    }

    pub(crate) fn load_owt_lcars_saved_interface(&mut self, store_id: &str) {
        match crate::owt_native::load_saved_interface(store_id) {
            Ok(loaded) => {
                log::debug!(
                    "OWT LCARS interface loaded from {} as {} for scope {:?}: {}",
                    loaded.store_id,
                    loaded.applied.interface_id,
                    loaded.applied.scope,
                    loaded.title
                );
                self.reflow_owt_lcars_layout();
            }
            Err(err) => {
                log::warn!("OWT LCARS saved interface load failed for {store_id}: {err:#}");
            }
        }
    }

    fn push_owt_lcars_surface_control_hitbox(
        &mut self,
        interface_id: &str,
        x: f32,
        y: f32,
        width: f32,
        height: f32,
    ) {
        if width < 4.0 || height < 4.0 {
            return;
        }
        self.ui_items.push(UIItem {
            x: x.max(0.0) as usize,
            y: y.max(0.0) as usize,
            width: width.ceil().max(1.0) as usize,
            height: height.ceil().max(1.0) as usize,
            item_type: UIItemType::OwtLcarsSurfaceControl(interface_id.to_string()),
        });
    }

    fn push_owt_lcars_surface_resize_hitbox(
        &mut self,
        interface_id: &str,
        x: f32,
        y: f32,
        width: f32,
        height: f32,
    ) {
        if width < 24.0 || height < 24.0 {
            return;
        }
        let size = width.min(height).min(34.0).max(18.0);
        self.ui_items.push(UIItem {
            x: (x + width - size).max(0.0) as usize,
            y: (y + height - size).max(0.0) as usize,
            width: size.ceil().max(1.0) as usize,
            height: size.ceil().max(1.0) as usize,
            item_type: UIItemType::OwtLcarsSurfaceResize {
                interface_id: interface_id.to_string(),
                panel_left: x.max(0.0) as usize,
                panel_top: y.max(0.0) as usize,
            },
        });
    }

    fn owt_lcars_rect_hovered(&self, x: f32, y: f32, width: f32, height: f32) -> bool {
        if width < 4.0 || height < 4.0 {
            return false;
        }
        if !matches!(self.current_mouse_capture, None | Some(MouseCapture::UI)) {
            return false;
        }
        let Some(event) = &self.current_mouse_event else {
            return false;
        };
        let mouse_x = event.coords.x as f32;
        let mouse_y = event.coords.y as f32;
        mouse_x >= x && mouse_x <= x + width && mouse_y >= y && mouse_y <= y + height
    }

    fn paint_owt_lcars_surface_focus_shell(
        &mut self,
        layers: &mut TripleLayerQuadAllocator,
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        lcars: LcarsPalette,
    ) -> anyhow::Result<()> {
        if !self.owt_lcars_rect_hovered(x, y, width, height) {
            return Ok(());
        }

        let inset = 3.0;
        let inner_width = (width - inset * 2.0).max(1.0);
        let inner_height = (height - inset * 2.0).max(1.0);
        let long = (width * 0.22).clamp(72.0, 220.0).min(inner_width);
        let short = (height * 0.18).clamp(36.0, 118.0).min(inner_height);
        let right = x + width;
        let bottom = y + height;
        let line = 3.0;

        self.filled_rectangle(
            layers,
            0,
            rect(x + inset, y + inset, long, line),
            lcars.peach,
        )?;
        self.filled_rectangle(
            layers,
            0,
            rect(x + inset, y + inset, line, short),
            lcars.blue,
        )?;
        self.filled_rectangle(
            layers,
            0,
            rect(right - inset - long, bottom - inset - line, long, line),
            lcars.violet,
        )?;
        self.filled_rectangle(
            layers,
            0,
            rect(right - inset - line, bottom - inset - short, line, short),
            lcars.amber,
        )?;
        self.filled_rectangle(
            layers,
            0,
            rect(x + inset + 8.0, y + inset + 8.0, 34.0, 3.0),
            lcars.black,
        )?;
        self.filled_rectangle(
            layers,
            0,
            rect(right - inset - 54.0, bottom - inset - 10.0, 42.0, 3.0),
            lcars.black,
        )?;
        Ok(())
    }

    fn paint_owt_lcars_surface_resize_handle(
        &mut self,
        layers: &mut TripleLayerQuadAllocator,
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        lcars: LcarsPalette,
    ) -> anyhow::Result<()> {
        if width < 72.0 || height < 72.0 {
            return Ok(());
        }

        let size = width.min(height).min(30.0).max(20.0);
        let inset = 9.0;
        let handle_x = x + width - size - inset;
        let handle_y = y + height - size - inset;
        let thickness = 4.0;

        self.filled_rectangle(
            layers,
            0,
            rect(handle_x, handle_y + size - thickness, size, thickness),
            lcars.amber,
        )?;
        self.filled_rectangle(
            layers,
            0,
            rect(handle_x + size - thickness, handle_y, thickness, size),
            lcars.peach,
        )?;
        self.filled_rectangle(
            layers,
            0,
            rect(
                handle_x + size * 0.34,
                handle_y + size - (thickness * 2.4),
                size * 0.58,
                thickness,
            ),
            lcars.violet,
        )?;
        self.filled_rectangle(
            layers,
            0,
            rect(
                handle_x + size - (thickness * 2.4),
                handle_y + size * 0.34,
                thickness,
                size * 0.58,
            ),
            lcars.blue,
        )?;
        Ok(())
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
        if lcars_table_focus_mode_key(&event.key, physical_key) {
            let Some(document) = crate::owt_native::active_interface_snapshots()
                .into_iter()
                .find(lcars_surface_visible)
            else {
                return false;
            };
            match crate::owt_native::toggle_focused_table_group_mode(Some(&document.id)) {
                Ok(focused) => {
                    self.play_owt_lcars_private_click_sound();
                    log::debug!(
                        "OWT LCARS table group focus mode {} for {}: {}",
                        if focused.active {
                            "entered"
                        } else {
                            "restored"
                        },
                        focused.table_node_id,
                        focused.focused_group
                    );
                    context.invalidate();
                    return true;
                }
                Err(err) => {
                    log::debug!("OWT LCARS table group focus mode skipped: {err:#}");
                    return false;
                }
            }
        }

        if let Some(movement) = lcars_table_focus_movement(&event.key, physical_key) {
            let Some(document) = crate::owt_native::active_interface_snapshots()
                .into_iter()
                .find(lcars_surface_visible)
            else {
                return false;
            };
            match crate::owt_native::focus_table_row_relative(Some(&document.id), movement) {
                Ok(focused) => {
                    self.play_owt_lcars_private_click_sound();
                    log::debug!(
                        "OWT LCARS table focus moved for {}: {} -> {}",
                        focused.interface_id,
                        focused.table_node_id,
                        focused.focused_row
                    );
                    context.invalidate();
                    return true;
                }
                Err(err) => {
                    log::debug!("OWT LCARS table focus navigation skipped: {err:#}");
                    return false;
                }
            }
        }

        if let Some(movement) = lcars_table_cell_focus_movement(&event.key, physical_key) {
            let Some(document) = crate::owt_native::active_interface_snapshots()
                .into_iter()
                .find(lcars_surface_visible)
            else {
                return false;
            };
            match crate::owt_native::focus_table_cell_relative(Some(&document.id), movement) {
                Ok(focused) => {
                    self.play_owt_lcars_private_click_sound();
                    log::debug!(
                        "OWT LCARS table cell focus moved for {}: {} -> {}",
                        focused.interface_id,
                        focused.table_node_id,
                        focused.focused_column
                    );
                    context.invalidate();
                    return true;
                }
                Err(err) => {
                    log::debug!("OWT LCARS table cell focus navigation skipped: {err:#}");
                    return false;
                }
            }
        }

        if lcars_table_activation_key(&event.key, physical_key) {
            let Some(document) = crate::owt_native::active_interface_snapshots()
                .into_iter()
                .find(lcars_surface_visible)
            else {
                return false;
            };
            match crate::owt_native::dispatch_focused_table_row_action(Some(&document.id)) {
                Ok(dispatched) => {
                    self.play_owt_lcars_private_click_sound();
                    log::debug!(
                        "OWT LCARS focused table action dispatched for {}: {}",
                        document.id,
                        dispatched.action_id
                    );
                    context.invalidate();
                    return true;
                }
                Err(err) => {
                    log::debug!("OWT LCARS focused table activation skipped: {err:#}");
                    return false;
                }
            }
        }

        let Some(document) = crate::owt_native::active_interface_snapshots()
            .into_iter()
            .find(lcars_surface_visible)
        else {
            return false;
        };

        if let Some(delta) = lcars_action_page_movement(&event.key) {
            if self.move_owt_lcars_action_page(&document, delta) {
                self.play_owt_lcars_private_click_sound();
                context.invalidate();
                return true;
            }
            return false;
        }

        if lcars_action_palette_key(&event.key) {
            self.open_owt_lcars_surface_menu(Some(document.id.clone()), 0.0, 0.0);
            self.play_owt_lcars_private_click_sound();
            context.invalidate();
            return true;
        }

        let Some(index) = event
            .raw
            .as_ref()
            .and_then(|raw| lcars_shortcut_index(&raw.key, raw.phys_code.as_ref()))
            .or_else(|| lcars_shortcut_index(&event.key, physical_key))
        else {
            return false;
        };
        let action_page = self.owt_lcars_current_action_page(&document);
        let Some(action_id) = lcars_keyboard_action_slots_for_page(&document, action_page)
            .get(index)
            .cloned()
        else {
            return false;
        };

        self.dispatch_owt_lcars_action(&document.id, &action_id, context);
        true
    }

    fn owt_lcars_current_action_page(&self, document: &InterfaceDocument) -> usize {
        let page_count = lcars_action_page_count(document);
        self.owt_lcars_action_page_by_interface
            .get(&document.id)
            .copied()
            .unwrap_or(0)
            .min(page_count.saturating_sub(1))
    }

    fn move_owt_lcars_action_page(&mut self, document: &InterfaceDocument, delta: isize) -> bool {
        let page_count = lcars_action_page_count(document);
        if page_count <= 1 {
            self.owt_lcars_action_page_by_interface.remove(&document.id);
            return false;
        }
        let current = self.owt_lcars_current_action_page(document) as isize;
        let next = (current + delta).rem_euclid(page_count as isize) as usize;
        self.owt_lcars_action_page_by_interface
            .insert(document.id.clone(), next);
        true
    }

    pub(crate) fn open_owt_lcars_surface_menu(
        &mut self,
        target_interface_id: Option<String>,
        anchor_x: f32,
        anchor_y: f32,
    ) {
        let target_owner = target_interface_id
            .as_ref()
            .and_then(|target_id| {
                crate::owt_native::active_interface_snapshots()
                    .into_iter()
                    .find(|document| &document.id == target_id)
            })
            .map(|document| lcars_scope_owner_label(&document.scope));
        let status_line = match (target_interface_id.as_ref(), target_owner.as_deref()) {
            (Some(id), Some(owner)) => Some(format!("TARGET {id} / {owner}")),
            (Some(id), None) => Some(format!("TARGET {id}")),
            (None, Some(owner)) => Some(format!("TARGET OWNER {owner}")),
            (None, None) => None,
        };
        self.owt_lcars_surface_menu = Some(OwtLcarsSurfaceMenuState {
            mode: OwtLcarsSurfaceMenuMode::Main,
            selected_idx: 0,
            saved_page: 0,
            saved_owner_filter: None,
            saved_interfaces: Vec::new(),
            status_line,
            target_interface_id,
            anchor_x,
            anchor_y,
        });
        self.reflow_owt_lcars_layout();
    }

    pub(crate) fn handle_owt_lcars_surface_menu_key(
        &mut self,
        event: &KeyEvent,
        context: &dyn WindowOps,
    ) -> bool {
        if self.owt_lcars_surface_menu.is_none() {
            return false;
        }

        let command = match event.key {
            KeyCode::Char('\u{1b}') | KeyCode::Char('\u{7f}') | KeyCode::Char('q') => {
                LcarsSurfaceMenuCommand::Close
            }
            KeyCode::Char('\u{8}') | KeyCode::Char('b') => {
                if self
                    .owt_lcars_surface_menu
                    .as_ref()
                    .map(|state| state.mode == OwtLcarsSurfaceMenuMode::LoadSaved)
                    .unwrap_or(false)
                {
                    LcarsSurfaceMenuCommand::BackToMain
                } else {
                    LcarsSurfaceMenuCommand::Close
                }
            }
            KeyCode::LeftArrow | KeyCode::UpArrow => {
                self.owt_lcars_surface_menu_move_selection(-1);
                context.invalidate();
                return true;
            }
            KeyCode::RightArrow | KeyCode::DownArrow => {
                self.owt_lcars_surface_menu_move_selection(1);
                context.invalidate();
                return true;
            }
            KeyCode::Char('\r') => {
                let index = self
                    .owt_lcars_surface_menu
                    .as_ref()
                    .map(|state| state.selected_idx)
                    .unwrap_or(0);
                self.owt_lcars_surface_menu_command_for_index(index)
            }
            KeyCode::Char(c) if ('1'..='8').contains(&c) => {
                self.owt_lcars_surface_menu_command_for_index((c as u32 - '1' as u32) as usize)
            }
            KeyCode::Numpad(n) if (1..=8).contains(&n) => {
                self.owt_lcars_surface_menu_command_for_index(n as usize - 1)
            }
            _ => LcarsSurfaceMenuCommand::None,
        };

        self.owt_lcars_surface_menu_execute(command, context);
        true
    }

    pub(crate) fn handle_owt_lcars_surface_menu_mouse(
        &mut self,
        event: &MouseEvent,
        context: &dyn WindowOps,
    ) -> bool {
        if self.owt_lcars_surface_menu.is_none() {
            return false;
        }

        let hit = self.owt_lcars_surface_menu_hit(event.coords.x as f32, event.coords.y as f32);
        match event.kind {
            MouseEventKind::Move => {
                if let Some(hit) = hit {
                    match hit {
                        LcarsSurfaceMenuHit::Wedge(index) => {
                            context.set_cursor(Some(MouseCursor::Hand));
                            self.owt_lcars_surface_menu_set_selection(index);
                        }
                        LcarsSurfaceMenuHit::Center => {
                            context.set_cursor(Some(MouseCursor::Hand));
                        }
                        LcarsSurfaceMenuHit::Background => {
                            context.set_cursor(Some(MouseCursor::Arrow));
                        }
                    }
                    context.invalidate();
                    return true;
                }
                false
            }
            MouseEventKind::VertWheel(amount) => {
                self.owt_lcars_surface_menu_move_selection(if amount > 0 { -1 } else { 1 });
                context.invalidate();
                true
            }
            MouseEventKind::Press(MousePress::Left) | MouseEventKind::Press(MousePress::Right) => {
                match hit {
                    Some(LcarsSurfaceMenuHit::Wedge(index)) => {
                        self.play_owt_lcars_private_click_sound();
                        self.owt_lcars_surface_menu_set_selection(index);
                        let command = self.owt_lcars_surface_menu_command_for_index(index);
                        self.owt_lcars_surface_menu_execute(command, context);
                    }
                    Some(LcarsSurfaceMenuHit::Center) => {
                        self.play_owt_lcars_private_click_sound();
                        let command = if self
                            .owt_lcars_surface_menu
                            .as_ref()
                            .map(|state| state.mode == OwtLcarsSurfaceMenuMode::LoadSaved)
                            .unwrap_or(false)
                        {
                            LcarsSurfaceMenuCommand::BackToMain
                        } else {
                            LcarsSurfaceMenuCommand::Close
                        };
                        self.owt_lcars_surface_menu_execute(command, context);
                    }
                    Some(LcarsSurfaceMenuHit::Background) => {
                        context.invalidate();
                    }
                    None => {
                        self.owt_lcars_surface_menu = None;
                        self.reflow_owt_lcars_layout();
                        context.invalidate();
                    }
                }
                true
            }
            MouseEventKind::Press(_) => {
                self.owt_lcars_surface_menu = None;
                self.reflow_owt_lcars_layout();
                context.invalidate();
                true
            }
            MouseEventKind::Release(_) => hit.is_some(),
            MouseEventKind::HorzWheel(_) => hit.is_some(),
        }
    }

    fn owt_lcars_surface_menu_execute(
        &mut self,
        command: LcarsSurfaceMenuCommand,
        context: &dyn WindowOps,
    ) {
        match command {
            LcarsSurfaceMenuCommand::OpenLoadSaved => {
                self.owt_lcars_surface_menu_open_saved();
                context.invalidate();
            }
            LcarsSurfaceMenuCommand::Patch(target, properties) => {
                self.owt_lcars_surface_menu = None;
                self.reflow_owt_lcars_layout();
                self.patch_owt_lcars_layout_properties_for_interface(target, properties);
                context.invalidate();
            }
            LcarsSurfaceMenuCommand::Load(store_id) => {
                self.owt_lcars_surface_menu = None;
                self.reflow_owt_lcars_layout();
                self.load_owt_lcars_saved_interface(&store_id);
                context.invalidate();
            }
            LcarsSurfaceMenuCommand::CycleOwnerFilter => {
                self.owt_lcars_surface_menu_cycle_owner_filter();
                context.invalidate();
            }
            LcarsSurfaceMenuCommand::PreviousPage => {
                if let Some(state) = self.owt_lcars_surface_menu.as_mut() {
                    state.saved_page = state.saved_page.saturating_sub(1);
                    state.selected_idx = 0;
                }
                context.invalidate();
            }
            LcarsSurfaceMenuCommand::NextPage => {
                if let Some(state) = self.owt_lcars_surface_menu.as_mut() {
                    state.saved_page = state.saved_page.saturating_add(1);
                    state.selected_idx = 0;
                }
                self.owt_lcars_surface_menu_clamp_selection();
                context.invalidate();
            }
            LcarsSurfaceMenuCommand::BackToMain => {
                if let Some(state) = self.owt_lcars_surface_menu.as_mut() {
                    state.mode = OwtLcarsSurfaceMenuMode::Main;
                    state.selected_idx = 0;
                    state.saved_owner_filter = None;
                    state.status_line = state
                        .target_interface_id
                        .as_ref()
                        .map(|id| format!("TARGET {id}"));
                }
                context.invalidate();
            }
            LcarsSurfaceMenuCommand::Close => {
                self.owt_lcars_surface_menu = None;
                self.reflow_owt_lcars_layout();
                context.invalidate();
            }
            LcarsSurfaceMenuCommand::None => {}
        }
    }

    fn owt_lcars_surface_menu_open_saved(&mut self) {
        let result = crate::owt_native::saved_interface_summaries();
        if let Some(state) = self.owt_lcars_surface_menu.as_mut() {
            state.mode = OwtLcarsSurfaceMenuMode::LoadSaved;
            state.selected_idx = 0;
            state.saved_page = 0;
            state.saved_owner_filter = None;
            match result {
                Ok(saved) if saved.is_empty() => {
                    state.saved_interfaces.clear();
                    state.status_line = Some("NO SAVED LCARS INTERFACES".to_string());
                }
                Ok(mut saved) => {
                    saved.sort_by(|left, right| {
                        left.owner_key
                            .cmp(&right.owner_key)
                            .then_with(|| left.title.cmp(&right.title))
                            .then_with(|| left.store_id.cmp(&right.store_id))
                    });
                    state.saved_interfaces = saved;
                    state.status_line = Some(lcars_saved_menu_status(state));
                }
                Err(err) => {
                    state.saved_interfaces.clear();
                    state.status_line = Some(format!("LOAD FAILED {err}"));
                }
            }
        }
    }

    fn owt_lcars_surface_menu_cycle_owner_filter(&mut self) {
        if let Some(state) = self.owt_lcars_surface_menu.as_mut() {
            state.saved_owner_filter = lcars_next_saved_owner_filter(
                &state.saved_interfaces,
                state.saved_owner_filter.as_deref(),
            );
            state.saved_page = 0;
            state.selected_idx = 0;
            state.status_line = Some(lcars_saved_menu_status(state));
        }
        self.owt_lcars_surface_menu_clamp_selection();
    }

    fn owt_lcars_surface_menu_command_for_index(&self, index: usize) -> LcarsSurfaceMenuCommand {
        let Some(state) = self.owt_lcars_surface_menu.as_ref() else {
            return LcarsSurfaceMenuCommand::None;
        };

        match state.mode {
            OwtLcarsSurfaceMenuMode::Main => {
                let Some(entry) = LCARS_SURFACE_MENU_ENTRIES.get(index) else {
                    return LcarsSurfaceMenuCommand::None;
                };
                match entry.action {
                    LcarsSurfaceMenuAction::LoadSaved => LcarsSurfaceMenuCommand::OpenLoadSaved,
                    action => {
                        let Some(properties) = lcars_surface_menu_properties(action) else {
                            return LcarsSurfaceMenuCommand::None;
                        };
                        LcarsSurfaceMenuCommand::Patch(
                            state.target_interface_id.clone(),
                            properties,
                        )
                    }
                }
            }
            OwtLcarsSurfaceMenuMode::LoadSaved => {
                let slots = self.owt_lcars_surface_menu_slots();
                let Some(slot) = slots.get(index) else {
                    return LcarsSurfaceMenuCommand::None;
                };
                match slot.saved.as_ref() {
                    Some(LcarsSavedMenuSlot::OwnerFilter) => {
                        LcarsSurfaceMenuCommand::CycleOwnerFilter
                    }
                    Some(LcarsSavedMenuSlot::Interface(saved)) => {
                        LcarsSurfaceMenuCommand::Load(saved.store_id.clone())
                    }
                    Some(LcarsSavedMenuSlot::PreviousPage) => LcarsSurfaceMenuCommand::PreviousPage,
                    Some(LcarsSavedMenuSlot::NextPage) => LcarsSurfaceMenuCommand::NextPage,
                    None => LcarsSurfaceMenuCommand::None,
                }
            }
        }
    }

    fn owt_lcars_surface_menu_slots(&self) -> Vec<LcarsSurfaceMenuSlot> {
        let Some(state) = self.owt_lcars_surface_menu.as_ref() else {
            return Vec::new();
        };

        match state.mode {
            OwtLcarsSurfaceMenuMode::Main => LCARS_SURFACE_MENU_ENTRIES
                .iter()
                .map(|entry| LcarsSurfaceMenuSlot {
                    label: entry.label.to_string(),
                    detail: entry.detail.to_string(),
                    saved: None,
                })
                .collect(),
            OwtLcarsSurfaceMenuMode::LoadSaved => {
                let owner_options = lcars_saved_owner_options(&state.saved_interfaces);
                let has_owner_control = owner_options.len() > 1;
                let visible_interfaces = lcars_saved_visible_interfaces(state);
                let total = visible_interfaces.len();
                if total == 0 && !has_owner_control {
                    return Vec::new();
                }

                let interface_capacity = if has_owner_control { 7 } else { 8 };
                let page_size = if total > interface_capacity {
                    if has_owner_control {
                        5
                    } else {
                        6
                    }
                } else {
                    interface_capacity
                };
                let max_page = total.saturating_sub(1) / page_size;
                let page = state.saved_page.min(max_page);
                let start = page.saturating_mul(page_size).min(total);
                let end = (start + page_size).min(total);
                let mut slots = Vec::new();
                if has_owner_control {
                    slots.push(LcarsSurfaceMenuSlot {
                        label: "OWNER".to_string(),
                        detail: lcars_saved_owner_filter_detail(state),
                        saved: Some(LcarsSavedMenuSlot::OwnerFilter),
                    });
                }
                for saved in &visible_interfaces[start..end] {
                    let title = saved.title.as_deref().unwrap_or(saved.store_id.as_str());
                    slots.push(LcarsSurfaceMenuSlot {
                        label: lcars_compact_menu_label(title),
                        detail: lcars_saved_interface_menu_detail(saved),
                        saved: Some(LcarsSavedMenuSlot::Interface((*saved).clone())),
                    });
                }
                if total > interface_capacity {
                    if page > 0 {
                        slots.push(LcarsSurfaceMenuSlot {
                            label: "PREV".to_string(),
                            detail: format!("PAGE {page}"),
                            saved: Some(LcarsSavedMenuSlot::PreviousPage),
                        });
                    }
                    if end < total {
                        slots.push(LcarsSurfaceMenuSlot {
                            label: "NEXT".to_string(),
                            detail: format!("PAGE {}", page + 2),
                            saved: Some(LcarsSavedMenuSlot::NextPage),
                        });
                    }
                }
                slots
            }
        }
    }

    fn owt_lcars_surface_menu_selectable_len(&self) -> usize {
        self.owt_lcars_surface_menu_slots().len()
    }

    fn owt_lcars_surface_menu_clamp_selection(&mut self) {
        let len = self.owt_lcars_surface_menu_selectable_len();
        if let Some(state) = self.owt_lcars_surface_menu.as_mut() {
            state.selected_idx = if len == 0 {
                0
            } else {
                state.selected_idx.min(len - 1)
            };
        }
    }

    fn owt_lcars_surface_menu_set_selection(&mut self, index: usize) {
        let len = self.owt_lcars_surface_menu_selectable_len();
        if len == 0 {
            return;
        }
        if let Some(state) = self.owt_lcars_surface_menu.as_mut() {
            state.selected_idx = index.min(len - 1);
        }
    }

    fn owt_lcars_surface_menu_move_selection(&mut self, delta: isize) {
        let len = self.owt_lcars_surface_menu_selectable_len();
        if len == 0 {
            return;
        }
        if let Some(state) = self.owt_lcars_surface_menu.as_mut() {
            let current = state.selected_idx.min(len - 1) as isize;
            state.selected_idx = (current + delta).rem_euclid(len as isize) as usize;
        }
    }

    fn owt_lcars_surface_menu_layout(&self) -> Option<LcarsSurfaceMenuLayout> {
        let state = self.owt_lcars_surface_menu.as_ref()?;
        let width = self.dimensions.pixel_width as f32;
        let height = self.dimensions.pixel_height as f32;
        if width < 320.0 || height < 240.0 {
            return None;
        }

        let border = self.get_os_border();
        let tab_bar_height = if self.show_tab_bar && !self.config.tab_bar_at_bottom {
            self.tab_bar_pixel_height().unwrap_or(0.0)
        } else {
            0.0
        };
        let top_floor = border.top.get() as f32 + tab_bar_height + 8.0;
        let chrome_clearance =
            (self.owt_lcars_window_chrome_left_reserved_pixels() + 10.0).max(66.0);
        let available_width = (width - chrome_clearance - 12.0).max(320.0);
        let available_height = (height - top_floor - 10.0).max(230.0);
        let panel_width = (width * 0.54)
            .clamp(520.0, 640.0)
            .min(available_width)
            .max(360.0);
        let panel_height = (height * 0.38)
            .clamp(248.0, 320.0)
            .min(available_height)
            .max(230.0);
        let x = clamp_ordered(
            chrome_clearance + 8.0,
            chrome_clearance,
            width - panel_width - 10.0,
        );
        let y = clamp_ordered(top_floor, top_floor, height - panel_height - 10.0);
        let command_width = (panel_width * 0.50).clamp(252.0, 326.0);
        let command_x = x + 14.0;
        let command_y = y + 72.0;
        let command_gap = 4.0;
        let command_height = ((panel_height - 112.0 - (command_gap * 4.0)) / 5.0).clamp(24.0, 34.0);
        let slot_rects = lcars_surface_menu_slot_rects(
            state.mode,
            command_x,
            command_y,
            command_width,
            command_height,
            command_gap,
        );
        let status_x = command_x + command_width + 18.0;
        let status_y = command_y;
        let status_width = (x + panel_width - status_x - 14.0).max(176.0);
        let status_height = (command_height * 5.0 + command_gap * 4.0).max(126.0);
        let close_width = 96.0_f32.min(panel_width * 0.18).max(78.0);
        let close_height = 28.0;
        let close_x = x + panel_width - close_width - 12.0;
        let close_y = y + 18.0;

        Some(LcarsSurfaceMenuLayout {
            x,
            y,
            width: panel_width,
            height: panel_height,
            command_x,
            command_y,
            command_width,
            slot_rects,
            status_x,
            status_y,
            status_width,
            status_height,
            close_x,
            close_y,
            close_width,
            close_height,
            anchor_x: state.anchor_x,
            anchor_y: state.anchor_y,
        })
    }

    fn owt_lcars_surface_menu_hit(&self, x: f32, y: f32) -> Option<LcarsSurfaceMenuHit> {
        let layout = self.owt_lcars_surface_menu_layout()?;
        let in_rect =
            |rx: f32, ry: f32, rw: f32, rh: f32| x >= rx && x <= rx + rw && y >= ry && y <= ry + rh;

        if in_rect(
            layout.close_x,
            layout.close_y,
            layout.close_width,
            layout.close_height,
        ) {
            return Some(LcarsSurfaceMenuHit::Center);
        }
        let len = self.owt_lcars_surface_menu_selectable_len();
        for index in 0..len.min(8) {
            let slot = layout.slot_rects[index];
            if in_rect(slot.x, slot.y, slot.width, slot.height) {
                return Some(LcarsSurfaceMenuHit::Wedge(index));
            }
        }

        if in_rect(layout.x, layout.y, layout.width, layout.height) {
            return Some(LcarsSurfaceMenuHit::Background);
        }

        None
    }

    fn owt_lcars_reserved_pixels(&self) -> LcarsReservedPixels {
        let mut reserved = LcarsReservedPixels::default();
        for document in crate::owt_native::active_interface_snapshots() {
            let next = self.owt_lcars_document_reserved_pixels(&document);
            reserved.left = reserved.left.max(next.left);
            reserved.top = reserved.top.max(next.top);
            reserved.right = reserved.right.max(next.right);
            reserved.bottom = reserved.bottom.max(next.bottom);
        }
        reserved
    }

    fn owt_lcars_document_reserved_pixels(
        &self,
        document: &InterfaceDocument,
    ) -> LcarsReservedPixels {
        if !owt_lcars_native_surface_viewport_ready(
            self.dimensions.pixel_width as f32,
            self.dimensions.pixel_height as f32,
        ) {
            return LcarsReservedPixels::default();
        }
        if !lcars_surface_visible(document) {
            return LcarsReservedPixels::default();
        }

        let placement = lcars_surface_placement(document);
        if !placement.reserves_terminal_space() {
            return LcarsReservedPixels::default();
        }

        match placement.layout {
            LcarsPanelLayout::Top => {
                if lcars_corner_button_mode(document) {
                    return LcarsReservedPixels {
                        top: lcars_compact_button_reserved_pixels(
                            self.owt_lcars_corner_button_height_for(document),
                        ),
                        ..Default::default()
                    };
                }
                if lcars_action_strip_mode(document) {
                    return LcarsReservedPixels {
                        top: LCARS_PANEL_MARGIN
                            + self.owt_lcars_action_strip_height_for(document)
                            + LCARS_PANEL_GAP,
                        ..Default::default()
                    };
                }

                let available_height =
                    self.dimensions.pixel_height as f32 - (LCARS_PANEL_MARGIN * 2.0);
                let structural = has_structural_lcars_layout(document);
                let Some(panel_height) = self.owt_lcars_top_panel_height_for(
                    available_height,
                    lcars_structural_mode(document),
                    structural,
                ) else {
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
            side @ (LcarsPanelLayout::Left | LcarsPanelLayout::Right) => {
                let available_width =
                    self.dimensions.pixel_width as f32 - (LCARS_PANEL_MARGIN * 2.0);
                let panel_width = if lcars_action_strip_mode(document) {
                    self.owt_lcars_action_strip_side_width_for(document, available_width)
                } else {
                    self.owt_lcars_side_panel_width_for(
                        available_width,
                        lcars_structural_mode(document),
                    )
                };
                let Some(panel_width) = panel_width else {
                    return LcarsReservedPixels::default();
                };
                let reserved = LCARS_PANEL_MARGIN + panel_width + LCARS_PANEL_GAP;
                match side {
                    LcarsPanelLayout::Left => LcarsReservedPixels {
                        left: reserved,
                        ..Default::default()
                    },
                    _ => LcarsReservedPixels {
                        right: reserved,
                        ..Default::default()
                    },
                }
            }
            LcarsPanelLayout::Bottom => {
                let panel_height = if lcars_corner_button_mode(document) {
                    self.owt_lcars_corner_button_height_for(document)
                } else if lcars_action_strip_mode(document) {
                    self.owt_lcars_action_strip_height_for(document)
                } else {
                    let available_height =
                        self.dimensions.pixel_height as f32 - (LCARS_PANEL_MARGIN * 2.0);
                    let Some(panel_height) = self.owt_lcars_bottom_panel_height_for(
                        available_height,
                        lcars_structural_mode(document),
                    ) else {
                        return LcarsReservedPixels::default();
                    };
                    panel_height
                };
                let bottom = if lcars_corner_button_mode(document) {
                    lcars_compact_button_reserved_pixels(panel_height)
                } else {
                    LCARS_PANEL_MARGIN + panel_height + LCARS_PANEL_GAP
                };
                LcarsReservedPixels {
                    bottom,
                    ..Default::default()
                }
            }
            LcarsPanelLayout::Overlay => LcarsReservedPixels::default(),
        }
    }

    fn owt_lcars_corner_button_height_for(&self, document: &InterfaceDocument) -> f32 {
        let cell_height = self.render_metrics.cell_size.height as f32;
        lcars_float_property(
            document,
            &[
                "floating_height",
                "float_height",
                "widget_height",
                "surface_height",
            ],
        )
        .unwrap_or((cell_height * 1.9).clamp(30.0, 42.0))
        .clamp(28.0, 54.0)
    }

    fn owt_lcars_action_strip_height_for(&self, document: &InterfaceDocument) -> f32 {
        let cell_height = self.render_metrics.cell_size.height as f32;
        let button_height = (cell_height * 1.8)
            .clamp(28.0, 38.0)
            .min(LCARS_ACTION_STRIP_MAX_HEIGHT);
        lcars_float_property(
            document,
            &[
                "floating_height",
                "float_height",
                "widget_height",
                "surface_height",
            ],
        )
        .unwrap_or(button_height + 18.0)
        .clamp(LCARS_ACTION_STRIP_MIN_HEIGHT, LCARS_ACTION_STRIP_MAX_HEIGHT)
    }

    fn owt_lcars_action_strip_side_width_for(
        &self,
        document: &InterfaceDocument,
        available_width: f32,
    ) -> Option<f32> {
        let cell_width = self.render_metrics.cell_size.width as f32;
        if available_width < LCARS_MIN_TERMINAL_REMAINDER + (cell_width * 10.0) {
            return None;
        }
        let max_without_starving_terminal =
            (available_width - LCARS_MIN_TERMINAL_REMAINDER).max(cell_width * 10.0);
        let longest_action_chars = panel_lines(document, 32)
            .items
            .iter()
            .filter(|line| line.action_id.is_some())
            .map(|line| line.text.chars().count())
            .max()
            .unwrap_or(8);
        let requested_width = lcars_float_property(
            document,
            &[
                "floating_width",
                "float_width",
                "widget_width",
                "surface_width",
            ],
        );
        lcars_action_strip_side_width_from_metrics(
            cell_width,
            longest_action_chars,
            requested_width,
            max_without_starving_terminal,
        )
    }

    pub fn paint_owt_lcars_panel(
        &mut self,
        layers: &mut TripleLayerQuadAllocator,
    ) -> anyhow::Result<()> {
        if self.owt_lcars_surface_menu.is_some() {
            self.paint_owt_lcars_surface_menu(layers)?;
            return Ok(());
        }

        let documents: Vec<_> = crate::owt_native::active_interface_snapshots()
            .into_iter()
            .filter(lcars_surface_visible)
            .collect();
        if !documents.is_empty() {
            let interface_ids = documents
                .iter()
                .map(|document| document.id.clone())
                .collect::<Vec<_>>();
            let scene_summary = lcars_render_scene_summary(&documents);
            crate::owt_native::record_lcars_render_pass(&interface_ids);
            for document in &documents {
                self.paint_owt_lcars_document(layers, document)?;
            }
            self.paint_owt_lcars_scene_summary_badge(layers, &scene_summary)?;
        }
        self.paint_owt_lcars_permission_prompt(layers)?;
        self.paint_owt_lcars_surface_menu(layers)?;
        self.paint_owt_lcars_drag_preview(layers)?;
        Ok(())
    }

    fn paint_owt_lcars_drag_preview(
        &mut self,
        layers: &mut TripleLayerQuadAllocator,
    ) -> anyhow::Result<()> {
        let Some(preview) = self.owt_lcars_drag_preview.clone() else {
            return Ok(());
        };
        let _interface_id = &preview.interface_id;
        let viewport_width = self.dimensions.pixel_width as f32;
        let viewport_height = self.dimensions.pixel_height as f32;
        if viewport_width < 80.0 || viewport_height < 80.0 {
            return Ok(());
        }

        let lcars = LcarsPalette::new();
        let (target, detail) =
            owt_lcars_drag_drop_target_text(preview.x, preview.y, viewport_width, viewport_height);
        let band = 10.0;
        match target {
            "LEFT" => {
                self.filled_rectangle(
                    layers,
                    0,
                    rect(0.0, 0.0, band, viewport_height),
                    with_alpha(lcars.blue, 0.68),
                )?;
            }
            "RIGHT" => {
                self.filled_rectangle(
                    layers,
                    0,
                    rect(viewport_width - band, 0.0, band, viewport_height),
                    with_alpha(lcars.blue, 0.68),
                )?;
            }
            "TOP" => {
                self.filled_rectangle(
                    layers,
                    0,
                    rect(0.0, 0.0, viewport_width, band),
                    with_alpha(lcars.amber, 0.72),
                )?;
            }
            "BOTTOM" | "B-RIGHT" => {
                self.filled_rectangle(
                    layers,
                    0,
                    rect(0.0, viewport_height - band, viewport_width, band),
                    with_alpha(lcars.peach, 0.72),
                )?;
            }
            _ => {
                self.filled_rectangle(
                    layers,
                    0,
                    rect(preview.x - 28.0, preview.y - 2.0, 56.0, 4.0),
                    with_alpha(lcars.violet, 0.78),
                )?;
                self.filled_rectangle(
                    layers,
                    0,
                    rect(preview.x - 2.0, preview.y - 28.0, 4.0, 56.0),
                    with_alpha(lcars.violet, 0.78),
                )?;
            }
        }

        let cell_width = self.render_metrics.cell_size.width as f32;
        let cell_height = self.render_metrics.cell_size.height as f32;
        let panel_width = (cell_width * 30.0).clamp(230.0, 340.0);
        let panel_height = (cell_height * 2.6).clamp(46.0, 60.0);
        let panel_x = clamp_ordered(
            preview.x + 16.0,
            LCARS_PANEL_MARGIN,
            viewport_width - panel_width - LCARS_PANEL_MARGIN,
        );
        let panel_y = clamp_ordered(
            preview.y + 16.0,
            LCARS_PANEL_MARGIN,
            viewport_height - panel_height - LCARS_PANEL_MARGIN,
        );
        let label = format!("DROP {target}");
        let label_cols = ((panel_width - 34.0) / cell_width).floor().max(6.0) as usize;

        self.filled_rectangle(
            layers,
            0,
            rect(panel_x, panel_y, panel_width, panel_height),
            with_alpha(lcars.black, 0.94),
        )?;
        paint_lcars_left_cap_bar(
            self,
            layers,
            panel_x,
            panel_y,
            (panel_width * 0.34).clamp(78.0, 118.0),
            panel_height,
            LCARS_BYTE_AMBER,
        )?;
        self.filled_rectangle(
            layers,
            0,
            rect(
                panel_x + 22.0,
                panel_y + 6.0,
                panel_width - 34.0,
                panel_height - 12.0,
            ),
            lcars.black,
        )?;
        self.filled_rectangle(
            layers,
            0,
            rect(
                panel_x + 28.0,
                panel_y + panel_height - 7.0,
                panel_width * 0.42,
                3.0,
            ),
            lcars.peach,
        )?;
        self.paint_owt_panel_text(
            layers,
            panel_x + 30.0,
            panel_y + 7.0,
            label_cols,
            &fit_text_ellipsis(&label, label_cols),
            RgbColor::new_8bpc(255, 240, 176),
            true,
        )?;
        let detail_cols = ((panel_width - 38.0) / cell_width).floor().max(8.0) as usize;
        self.paint_owt_panel_text(
            layers,
            panel_x + 30.0,
            panel_y + 7.0 + cell_height,
            detail_cols,
            &fit_text_ellipsis(detail, detail_cols),
            RgbColor::new_8bpc(153, 204, 255),
            true,
        )
    }

    fn paint_owt_lcars_scene_summary_badge(
        &mut self,
        layers: &mut TripleLayerQuadAllocator,
        summary: &LcarsRenderSceneSummary,
    ) -> anyhow::Result<()> {
        let Some((title, detail)) = lcars_render_scene_badge_lines(summary) else {
            return Ok(());
        };
        let lcars = LcarsPalette::new();
        let cell_width = self.render_metrics.cell_size.width as f32;
        let cell_height = self.render_metrics.cell_size.height as f32;
        let width = (cell_width * 42.0).clamp(310.0, 460.0);
        let height = (cell_height * 2.2).clamp(34.0, 48.0);
        let margin = LCARS_PANEL_MARGIN + 8.0;
        let x = (self.dimensions.pixel_width as f32 - width - margin).max(margin);
        let y = if summary
            .slots
            .iter()
            .any(|slot| slot.slot == "bottom_strip" && slot.reserved_count > 0)
        {
            margin + 44.0
        } else {
            (self.dimensions.pixel_height as f32 - height - margin).max(margin)
        };
        let label_width = (width * 0.36).clamp(cell_width * 10.0, cell_width * 17.0);
        let detail_cols = ((width - label_width - 24.0) / cell_width).floor().max(8.0) as usize;
        let title_cols = ((label_width - 24.0) / cell_width).floor().max(6.0) as usize;
        let conflict = !summary.conflict_hints.is_empty();
        paint_lcars_left_cap_bar(
            self,
            layers,
            x,
            y,
            label_width,
            height,
            if conflict {
                LCARS_BYTE_RED
            } else {
                LCARS_BYTE_BLUE
            },
        )?;
        self.filled_rectangle(
            layers,
            0,
            rect(
                x + label_width * 0.28,
                y + 4.0,
                label_width * 0.64,
                height - 8.0,
            ),
            lcars.black,
        )?;
        self.filled_rectangle(
            layers,
            0,
            rect(x + label_width + 8.0, y, width - label_width - 8.0, height),
            lcars.black,
        )?;
        self.filled_rectangle(
            layers,
            0,
            rect(x + label_width + 12.0, y + 4.0, width * 0.18, 3.0),
            if conflict { lcars.red } else { lcars.cyan },
        )?;
        self.filled_rectangle(
            layers,
            0,
            rect(x + width - 88.0, y + height - 6.0, 70.0, 3.0),
            lcars.amber,
        )?;
        self.paint_owt_panel_text(
            layers,
            x + 18.0,
            y + (height - cell_height).max(0.0) * 0.5,
            title_cols,
            &fit_text_ellipsis(&title, title_cols),
            RgbColor::new_8bpc(255, 204, 112),
            true,
        )?;
        self.paint_owt_panel_text(
            layers,
            x + label_width + 18.0,
            y + (height - cell_height).max(0.0) * 0.5,
            detail_cols,
            &fit_text_ellipsis(&detail, detail_cols),
            if conflict {
                RgbColor::new_8bpc(255, 149, 96)
            } else {
                RgbColor::new_8bpc(164, 212, 255)
            },
            true,
        )?;
        Ok(())
    }

    fn paint_owt_lcars_action_strip(
        &mut self,
        layers: &mut TripleLayerQuadAllocator,
        document: &InterfaceDocument,
        placement: LcarsSurfacePlacement,
    ) -> anyhow::Result<()> {
        let lcars = LcarsPalette::for_document(document);
        let cell_width = self.render_metrics.cell_size.width as f32;
        let cell_height = self.render_metrics.cell_size.height as f32;
        let lines = panel_lines(document, 32);
        let all_actions = lines
            .items
            .iter()
            .filter(|line| line.action_id.is_some())
            .collect::<Vec<_>>();
        let action_page = self.owt_lcars_current_action_page(document);
        let actions = all_actions
            .iter()
            .copied()
            .skip(action_page.saturating_mul(LCARS_KEY_ACTION_LIMIT))
            .take(LCARS_KEY_ACTION_LIMIT)
            .collect::<Vec<_>>();
        if actions.is_empty() {
            return Ok(());
        }

        let margin = LCARS_PANEL_MARGIN + 8.0;
        let viewport_width = self.dimensions.pixel_width as f32;
        let viewport_height = self.dimensions.pixel_height as f32;
        let border = self.get_os_border();
        let tab_bar_height = if self.show_tab_bar && !self.config.tab_bar_at_bottom {
            self.tab_bar_pixel_height()
                .context("compute tab bar height for OWT LCARS action strip")?
        } else {
            0.0
        };
        let usable_top = border.top.get() as f32 + tab_bar_height + margin;
        let max_width = (viewport_width - margin * 2.0).max(LCARS_NATIVE_SURFACE_MIN_WIDTH);
        let button_height = (cell_height * 1.8)
            .clamp(28.0, 38.0)
            .min(LCARS_ACTION_STRIP_MAX_HEIGHT);
        let gap = 8.0;
        let label_width = (cell_width * 10.0).clamp(96.0, 132.0);
        let requested_width = lcars_float_property(
            document,
            &[
                "floating_width",
                "float_width",
                "widget_width",
                "surface_width",
            ],
        );
        let longest_action_chars = actions
            .iter()
            .map(|line| line.text.chars().count())
            .max()
            .unwrap_or(0) as f32;
        let target_button_width =
            ((longest_action_chars + 2.0) * cell_width + 42.0).clamp(154.0, 248.0);
        let default_width = (label_width
            + gap
            + actions.len() as f32 * target_button_width
            + gap * actions.len().saturating_sub(1) as f32)
            .min(max_width)
            .max(360.0);
        let last_action_id = crate::owt_native::last_dispatched_action_for_interface(&document.id)
            .map(|action| action.action_id);

        if lcars_action_strip_vertical_rail(placement) {
            let panel_width = self
                .owt_lcars_action_strip_side_width_for(document, max_width)
                .unwrap_or((cell_width * 28.0).clamp(240.0, 360.0).min(max_width));
            let rail_width = (cell_width * 7.0).clamp(66.0, 92.0).min(panel_width * 0.36);
            let button_width = (panel_width - rail_width - gap * 2.0).max(cell_width * 8.0);
            let visible_count = actions.len().max(1);
            let page_count = all_actions.len().max(1).div_ceil(LCARS_KEY_ACTION_LIMIT);
            let page_height = if page_count > 1 {
                cell_height + gap
            } else {
                0.0
            };
            let desired_height = gap * 2.0
                + visible_count as f32 * button_height
                + visible_count.saturating_sub(1) as f32 * gap
                + page_height;
            let available_height =
                (viewport_height - usable_top - margin).max(LCARS_ACTION_STRIP_MIN_HEIGHT);
            let panel_height = lcars_float_property(
                document,
                &[
                    "floating_height",
                    "float_height",
                    "widget_height",
                    "surface_height",
                ],
            )
            .unwrap_or(desired_height)
            .clamp(
                LCARS_ACTION_STRIP_MIN_HEIGHT.min(available_height),
                available_height.max(LCARS_ACTION_STRIP_MIN_HEIGHT),
            );
            let left = match placement.layout {
                LcarsPanelLayout::Right => (viewport_width - panel_width - margin).max(margin),
                _ => margin,
            };
            let top = usable_top;
            let button_left = left + rail_width + gap;
            let button_cols = ((button_width - 34.0) / cell_width).floor().max(4.0) as usize;
            let title_cols = ((rail_width - 16.0) / cell_width).floor().max(3.0) as usize;

            self.filled_rectangle(
                layers,
                0,
                rect(left, top, panel_width, panel_height),
                with_alpha(lcars.black, 0.96),
            )?;
            paint_lcars_left_cap_bar(
                self,
                layers,
                left,
                top,
                rail_width,
                panel_height,
                LCARS_BYTE_AMBER,
            )?;
            self.push_owt_lcars_surface_control_hitbox(
                &document.id,
                left,
                top,
                rail_width,
                panel_height,
            );
            self.paint_owt_panel_text(
                layers,
                left + 8.0,
                top + gap,
                title_cols,
                &fit_text_ellipsis(&document.title.to_uppercase(), title_cols),
                RgbColor::new_8bpc(0, 0, 0),
                true,
            )?;

            for (index, line) in actions.iter().enumerate() {
                let y = top + gap + index as f32 * (button_height + gap);
                if y + button_height > top + panel_height + 1.0 {
                    break;
                }
                let active = last_action_id.as_deref() == line.action_id.as_deref();
                paint_lcars_action_button(
                    self,
                    layers,
                    button_left,
                    y,
                    button_width,
                    button_height,
                    button_cols,
                    &line.text,
                    lcars_action_text_color(index),
                    lcars.action_fill(index),
                    lcars_action_fill_byte(index),
                    active,
                    Some(index + 1),
                    lcars,
                )?;
                if let Some(action_id) = &line.action_id {
                    self.ui_items.push(UIItem {
                        x: button_left.max(0.0) as usize,
                        y: y.max(0.0) as usize,
                        width: button_width.ceil().max(1.0) as usize,
                        height: button_height.ceil().max(1.0) as usize,
                        item_type: UIItemType::OwtLcarsAction {
                            interface_id: document.id.clone(),
                            action_id: action_id.clone(),
                        },
                    });
                }
            }
            if page_count > 1 {
                let page_y = (top + panel_height - cell_height - gap).max(top + gap);
                self.paint_owt_panel_text(
                    layers,
                    button_left,
                    page_y,
                    button_cols,
                    &format!("ACTIONS {}/{}", action_page + 1, page_count),
                    RgbColor::new_8bpc(255, 204, 112),
                    true,
                )?;
            }

            return Ok(());
        }

        let panel_height = self
            .owt_lcars_action_strip_height_for(document)
            .min((viewport_height - margin * 2.0).max(LCARS_ACTION_STRIP_MIN_HEIGHT));
        let panel_width = requested_width
            .unwrap_or(default_width)
            .clamp(320.0_f32.min(max_width), max_width);
        let (left, top) = match placement.layout {
            LcarsPanelLayout::Left => (margin, usable_top),
            LcarsPanelLayout::Right => (
                (viewport_width - panel_width - margin).max(margin),
                usable_top,
            ),
            LcarsPanelLayout::Top => {
                let left = match placement.origin {
                    LcarsSurfaceOrigin::Top => ((viewport_width - panel_width) * 0.5).max(margin),
                    _ => margin,
                };
                (left, usable_top)
            }
            LcarsPanelLayout::Bottom => {
                let left = match placement.origin {
                    LcarsSurfaceOrigin::BottomRight => {
                        (viewport_width - panel_width - margin).max(margin)
                    }
                    _ => margin,
                };
                (
                    left,
                    (viewport_height - panel_height - margin).max(usable_top),
                )
            }
            LcarsPanelLayout::Overlay => {
                let default_left = (viewport_width - panel_width - margin).max(margin);
                let default_top = usable_top;
                if let Some(floating) = self.owt_lcars_floating_geometry(
                    document,
                    default_left,
                    default_top,
                    panel_width,
                    panel_height,
                    320.0,
                    LCARS_ACTION_STRIP_MIN_HEIGHT,
                ) {
                    (floating.left, floating.top)
                } else {
                    (default_left, default_top)
                }
            }
        };
        let control_width = label_width.min(panel_width * 0.34);
        let action_left = left + control_width + gap;
        let action_width_total = (panel_width - control_width - gap).max(1.0);
        let visible_count = actions.len().max(1);
        let button_width = ((action_width_total - gap * (visible_count.saturating_sub(1)) as f32)
            / visible_count as f32)
            .clamp(112.0, target_button_width.max(210.0));
        let button_y = top + ((panel_height - button_height) * 0.5).max(0.0);
        let text_cols = ((button_width - 34.0) / cell_width).floor().max(4.0) as usize;
        let title_cols = ((control_width - 18.0) / cell_width).floor().max(4.0) as usize;

        self.filled_rectangle(
            layers,
            0,
            rect(left, top, panel_width, panel_height),
            with_alpha(lcars.black, 0.96),
        )?;
        paint_lcars_left_cap_bar(
            self,
            layers,
            left,
            top,
            control_width,
            panel_height,
            LCARS_BYTE_AMBER,
        )?;
        self.push_owt_lcars_surface_control_hitbox(
            &document.id,
            left,
            top,
            control_width,
            panel_height,
        );
        self.paint_owt_panel_text(
            layers,
            left + 12.0,
            top + ((panel_height - cell_height) * 0.5).max(0.0),
            title_cols,
            &fit_text_ellipsis(&document.title.to_uppercase(), title_cols),
            RgbColor::new_8bpc(0, 0, 0),
            true,
        )?;

        for (index, line) in actions.iter().enumerate() {
            let x = action_left + index as f32 * (button_width + gap);
            if x + button_width > left + panel_width + 1.0 {
                break;
            }
            let active = last_action_id.as_deref() == line.action_id.as_deref();
            paint_lcars_action_button(
                self,
                layers,
                x,
                button_y,
                button_width,
                button_height,
                text_cols,
                &line.text,
                lcars_action_text_color(index),
                lcars.action_fill(index),
                lcars_action_fill_byte(index),
                active,
                Some(index + 1),
                lcars,
            )?;
            if let Some(action_id) = &line.action_id {
                self.ui_items.push(UIItem {
                    x: x.max(0.0) as usize,
                    y: button_y.max(0.0) as usize,
                    width: button_width.ceil().max(1.0) as usize,
                    height: button_height.ceil().max(1.0) as usize,
                    item_type: UIItemType::OwtLcarsAction {
                        interface_id: document.id.clone(),
                        action_id: action_id.clone(),
                    },
                });
            }
        }
        let page_count = all_actions.len().max(1).div_ceil(LCARS_KEY_ACTION_LIMIT);
        if page_count > 1 {
            self.paint_owt_panel_text(
                layers,
                left + panel_width - 116.0,
                top + 8.0,
                13,
                &format!("ACTIONS {}/{}", action_page + 1, page_count),
                RgbColor::new_8bpc(255, 204, 112),
                true,
            )?;
        }

        Ok(())
    }

    fn paint_owt_lcars_corner_button(
        &mut self,
        layers: &mut TripleLayerQuadAllocator,
        document: &InterfaceDocument,
        placement: LcarsSurfacePlacement,
    ) -> anyhow::Result<()> {
        let lcars = LcarsPalette::for_document(document);
        let cell_width = self.render_metrics.cell_size.width as f32;
        let lines = panel_lines(document, 32);
        let Some(line) = lines.items.iter().find(|line| line.action_id.is_some()) else {
            return Ok(());
        };

        let margin = LCARS_PANEL_MARGIN + 8.0;
        let viewport_width = self.dimensions.pixel_width as f32;
        let viewport_height = self.dimensions.pixel_height as f32;
        let border = self.get_os_border();
        let tab_bar_height = if self.show_tab_bar && !self.config.tab_bar_at_bottom {
            self.tab_bar_pixel_height()
                .context("compute tab bar height for OWT LCARS corner button")?
        } else {
            0.0
        };
        let usable_top = border.top.get() as f32 + tab_bar_height + margin;
        let button_height = self.owt_lcars_corner_button_height_for(document);
        let text_width = line.text.chars().count() as f32 * cell_width;
        let button_width = lcars_float_property(
            document,
            &[
                "floating_width",
                "float_width",
                "widget_width",
                "surface_width",
            ],
        )
        .unwrap_or((text_width + 118.0).clamp(176.0, 300.0))
        .clamp(132.0, (viewport_width - margin * 2.0).max(132.0));
        let default_left = match placement.layout {
            LcarsPanelLayout::Left => margin,
            LcarsPanelLayout::Right => (viewport_width - button_width - margin).max(margin),
            LcarsPanelLayout::Top => match placement.origin {
                LcarsSurfaceOrigin::Top => ((viewport_width - button_width) * 0.5).max(margin),
                _ => margin,
            },
            LcarsPanelLayout::Bottom => match placement.origin {
                LcarsSurfaceOrigin::BottomRight => {
                    (viewport_width - button_width - margin).max(margin)
                }
                _ => margin,
            },
            LcarsPanelLayout::Overlay => (viewport_width - button_width - margin).max(margin),
        };
        let default_top = match placement.layout {
            LcarsPanelLayout::Bottom => (viewport_height - button_height - margin).max(usable_top),
            _ => usable_top,
        };
        let (left, top, width, height) = self
            .owt_lcars_floating_geometry(
                document,
                default_left,
                default_top,
                button_width,
                button_height,
                132.0,
                28.0,
            )
            .map(|floating| (floating.left, floating.top, floating.width, floating.height))
            .unwrap_or((default_left, default_top, button_width, button_height));
        let text_cols = ((width - 42.0) / cell_width).floor().max(4.0) as usize;
        let last_action_id = crate::owt_native::last_dispatched_action_for_interface(&document.id)
            .map(|action| action.action_id);
        let active = last_action_id.as_deref() == line.action_id.as_deref();

        paint_lcars_action_button(
            self,
            layers,
            left,
            top,
            width,
            height,
            text_cols,
            &line.text,
            lcars_action_text_color(0),
            lcars.action_fill(0),
            lcars_action_fill_byte(0),
            active,
            line.hotkey,
            lcars,
        )?;
        if let Some(action_id) = &line.action_id {
            self.ui_items.push(UIItem {
                x: left.max(0.0) as usize,
                y: top.max(0.0) as usize,
                width: width.ceil().max(1.0) as usize,
                height: height.ceil().max(1.0) as usize,
                item_type: UIItemType::OwtLcarsAction {
                    interface_id: document.id.clone(),
                    action_id: action_id.clone(),
                },
            });
        }

        Ok(())
    }

    fn paint_owt_lcars_permission_prompt(
        &mut self,
        layers: &mut TripleLayerQuadAllocator,
    ) -> anyhow::Result<()> {
        let Some(request) = crate::owt_native::pending_permission_requests()
            .into_iter()
            .last()
        else {
            return Ok(());
        };
        let lcars = LcarsPalette::new();
        let cell_width = self.render_metrics.cell_size.width as f32;
        let cell_height = self.render_metrics.cell_size.height as f32;
        let width = (cell_width * 56.0).clamp(380.0, 560.0);
        let height = (cell_height * 7.0).clamp(116.0, 152.0);
        let margin = LCARS_PANEL_MARGIN + 8.0;
        let border = self.get_os_border();
        let tab_bar_height = if self.show_tab_bar && !self.config.tab_bar_at_bottom {
            self.tab_bar_pixel_height()
                .context("compute tab bar height for OWT LCARS permission prompt")?
        } else {
            0.0
        };
        let x = (self.dimensions.pixel_width as f32 - width - margin).max(margin);
        let y = border.top.get() as f32 + tab_bar_height + margin;
        let rail_width = (width * 0.24).clamp(88.0, 132.0);
        let body_x = x + rail_width + 8.0;
        let body_w = width - rail_width - 8.0;
        let button_h = (cell_height * 1.7).clamp(24.0, 34.0);
        let button_w = (body_w - 10.0) * 0.5;
        let button_y = y + height - button_h - 10.0;

        self.filled_rectangle(layers, 0, rect(x, y, width, height), lcars.black)?;
        paint_lcars_left_cap_bar(self, layers, x, y, rail_width, height, LCARS_BYTE_RED)?;
        self.filled_rectangle(
            layers,
            0,
            rect(body_x, y + 4.0, body_w, height - 8.0),
            with_alpha(lcars.black, 0.96),
        )?;
        self.filled_rectangle(
            layers,
            0,
            rect(body_x + 8.0, y + 8.0, body_w * 0.38, 4.0),
            lcars.red,
        )?;
        self.filled_rectangle(
            layers,
            0,
            rect(
                body_x + body_w - body_w * 0.30 - 8.0,
                y + 8.0,
                body_w * 0.30,
                4.0,
            ),
            lcars.amber,
        )?;

        let label_cols = ((rail_width - 22.0) / cell_width).floor().max(6.0) as usize;
        self.paint_owt_panel_text(
            layers,
            x + 14.0,
            y + 14.0,
            label_cols,
            "PERMISSION",
            RgbColor::new_8bpc(255, 204, 112),
            true,
        )?;
        self.paint_owt_panel_text(
            layers,
            x + 14.0,
            y + 14.0 + cell_height,
            label_cols,
            &request.request_kind.to_ascii_uppercase(),
            RgbColor::new_8bpc(164, 212, 255),
            true,
        )?;

        let body_cols = ((body_w - 24.0) / cell_width).floor().max(12.0) as usize;
        let kind = request
            .kind
            .as_ref()
            .map(|kind| format!("{kind:?}").to_ascii_uppercase())
            .unwrap_or_else(|| "REQUEST".to_string());
        let title = format!("{} / {}", kind, request.label);
        self.paint_owt_panel_text(
            layers,
            body_x + 12.0,
            y + 20.0,
            body_cols,
            &fit_text_ellipsis(&title, body_cols),
            RgbColor::new_8bpc(210, 225, 255),
            true,
        )?;
        let target = request
            .target
            .as_deref()
            .unwrap_or(request.action_id.as_deref().unwrap_or("external execution"));
        let target = format!("{target} @ {}", request.interface_id);
        self.paint_owt_panel_text(
            layers,
            body_x + 12.0,
            y + 20.0 + cell_height,
            body_cols,
            &fit_text_ellipsis(&target, body_cols),
            RgbColor::new_8bpc(164, 212, 255),
            false,
        )?;
        if let Some(source) = request.source.as_deref() {
            self.paint_owt_panel_text(
                layers,
                body_x + 12.0,
                y + 20.0 + cell_height * 2.0,
                body_cols,
                &fit_text_ellipsis(source, body_cols),
                RgbColor::new_8bpc(197, 143, 255),
                false,
            )?;
        }

        for (index, (label, allow, fill, fill_byte)) in [
            ("ALLOW", true, lcars.blue, LCARS_BYTE_BLUE),
            ("DENY", false, lcars.red, LCARS_BYTE_RED),
        ]
        .iter()
        .copied()
        .enumerate()
        {
            let button_x = body_x + index as f32 * (button_w + 10.0);
            paint_lcars_action_button(
                self,
                layers,
                button_x,
                button_y,
                button_w,
                button_h,
                ((button_w - 18.0) / cell_width).floor().max(5.0) as usize,
                label,
                RgbColor::new_8bpc(0, 0, 0),
                fill,
                fill_byte,
                false,
                None,
                lcars,
            )?;
            self.ui_items.push(UIItem {
                x: button_x.max(0.0) as usize,
                y: button_y.max(0.0) as usize,
                width: button_w.ceil().max(1.0) as usize,
                height: button_h.ceil().max(1.0) as usize,
                item_type: UIItemType::OwtLcarsPermissionDecision {
                    request_id: request.request_id.clone(),
                    allow,
                },
            });
        }
        Ok(())
    }

    fn paint_owt_lcars_surface_menu(
        &mut self,
        layers: &mut TripleLayerQuadAllocator,
    ) -> anyhow::Result<()> {
        let Some(state) = self.owt_lcars_surface_menu.clone() else {
            return Ok(());
        };
        let Some(layout) = self.owt_lcars_surface_menu_layout() else {
            return Ok(());
        };
        let slots = self.owt_lcars_surface_menu_slots();
        let lcars = LcarsPalette::new();
        let cell_width = self.render_metrics.cell_size.width as f32;
        let cell_height = self.render_metrics.cell_size.height as f32;
        let slot_colors = [
            LCARS_BYTE_AMBER,
            LCARS_BYTE_ORANGE,
            LCARS_BYTE_BLUE,
            LCARS_BYTE_PEACH,
            LCARS_BYTE_VIOLET,
            LCARS_BYTE_BLUE,
            LCARS_BYTE_RED,
            LCARS_BYTE_ORANGE,
        ];

        self.paint_owt_lcars_surface_menu_connector(layers, &layout, lcars)?;

        self.filled_rectangle(
            layers,
            0,
            rect(layout.x, layout.y, layout.width, layout.height),
            lcars.black,
        )?;
        self.filled_rectangle(
            layers,
            0,
            rect(layout.x + 4.0, layout.y + 4.0, layout.width - 8.0, 5.0),
            with_alpha(lcars.red, 0.62),
        )?;
        self.filled_rectangle(
            layers,
            0,
            rect(
                layout.x + 4.0,
                layout.y + layout.height - 8.0,
                layout.width - 8.0,
                4.0,
            ),
            with_alpha(lcars.violet, 0.58),
        )?;
        self.filled_rectangle(
            layers,
            0,
            rect(
                layout.command_x - 8.0,
                layout.command_y - 8.0,
                layout.command_width + 16.0,
                layout.status_height + 16.0,
            ),
            with_alpha(lcars.blue, 0.16),
        )?;
        self.filled_rectangle(
            layers,
            0,
            rect(
                layout.status_x - 10.0,
                layout.status_y - 8.0,
                layout.status_width + 18.0,
                layout.status_height + 16.0,
            ),
            with_alpha(lcars.violet, 0.14),
        )?;
        self.filled_rectangle(
            layers,
            0,
            rect(
                layout.command_x - 8.0,
                layout.command_y - 8.0,
                layout.command_width * 0.54,
                4.0,
            ),
            lcars.orange,
        )?;
        self.filled_rectangle(
            layers,
            0,
            rect(
                layout.status_x - 10.0,
                layout.status_y + layout.status_height + 4.0,
                layout.status_width * 0.74,
                4.0,
            ),
            lcars.peach,
        )?;

        let header_h = 58.0;
        paint_lcars_left_cap_bar(
            self,
            layers,
            layout.x,
            layout.y,
            layout.width * 0.36,
            header_h,
            LCARS_BYTE_RED,
        )?;
        self.filled_rectangle(
            layers,
            0,
            rect(
                layout.x + layout.width * 0.14,
                layout.y + 16.0,
                layout.width * 0.25,
                header_h - 22.0,
            ),
            lcars.black,
        )?;
        paint_lcars_right_cap_bar(
            self,
            layers,
            layout.x + layout.width * 0.38,
            layout.y,
            layout.width * 0.16,
            header_h,
            LCARS_BYTE_AMBER,
        )?;
        self.filled_rectangle(
            layers,
            0,
            rect(
                layout.x + layout.width * 0.56,
                layout.y,
                layout.width * 0.26,
                6.0,
            ),
            lcars.red,
        )?;
        self.filled_rectangle(
            layers,
            0,
            rect(
                layout.x + layout.width * 0.84,
                layout.y,
                layout.width * 0.12,
                6.0,
            ),
            lcars.violet,
        )?;
        let close_bridge_x = layout.x + layout.width * 0.66 + 10.0;
        if layout.close_x > close_bridge_x + 10.0 {
            self.filled_rectangle(
                layers,
                0,
                rect(
                    close_bridge_x,
                    layout.y + header_h * 0.46,
                    layout.close_x - close_bridge_x + 4.0,
                    4.0,
                ),
                lcars.peach,
            )?;
        }

        let title = match state.mode {
            OwtLcarsSurfaceMenuMode::Main => "SURFACE CONTROL",
            OwtLcarsSurfaceMenuMode::LoadSaved => "PROFILE STORE",
        };
        let title_x = layout.x + layout.width * 0.165;
        let title_cols = ((layout.width * 0.24) / cell_width).floor().max(8.0) as usize;
        self.paint_owt_panel_text(
            layers,
            title_x,
            layout.y + 24.0,
            title_cols,
            title,
            RgbColor::new_8bpc(255, 204, 112),
            true,
        )?;

        let close_label = if state.mode == OwtLcarsSurfaceMenuMode::LoadSaved {
            "BACK"
        } else {
            "EXIT"
        };
        paint_lcars_right_cap_bar(
            self,
            layers,
            layout.close_x,
            layout.close_y,
            layout.close_width,
            layout.close_height,
            if state.mode == OwtLcarsSurfaceMenuMode::LoadSaved {
                LCARS_BYTE_VIOLET
            } else {
                LCARS_BYTE_PEACH
            },
        )?;
        self.filled_rectangle(
            layers,
            0,
            rect(
                layout.close_x + 10.0,
                layout.close_y + 4.0,
                layout.close_width - 22.0,
                layout.close_height - 8.0,
            ),
            lcars.black,
        )?;
        let close_cols = ((layout.close_width - 24.0) / cell_width).floor().max(4.0) as usize;
        self.paint_owt_panel_text(
            layers,
            layout.close_x + 14.0,
            layout.close_y + (layout.close_height - cell_height).max(0.0) * 0.5,
            close_cols,
            close_label,
            RgbColor::new_8bpc(255, 204, 112),
            true,
        )?;

        let rail_x = layout.x + 8.0;
        let rail_y = layout.y + header_h + 8.0;
        let rail_w = 8.0;
        let rail_h = layout.height - header_h - 24.0;
        self.filled_rectangle(
            layers,
            0,
            rect(rail_x, rail_y, rail_w, rail_h),
            lcars.orange,
        )?;
        self.filled_rectangle(layers, 0, rect(rail_x, rail_y, rail_w, 42.0), lcars.red)?;
        self.filled_rectangle(
            layers,
            0,
            rect(rail_x, rail_y + rail_h - 58.0, rail_w, 58.0),
            lcars.violet,
        )?;

        for (index, slot) in slots.iter().take(8).enumerate() {
            let slot_rect = layout.slot_rects[index];
            let active = index == state.selected_idx;
            let fill_byte = if active {
                LCARS_BYTE_AMBER
            } else {
                slot_colors[index].with_alpha(236)
            };
            paint_lcars_left_cap_bar(
                self,
                layers,
                slot_rect.x,
                slot_rect.y,
                slot_rect.width,
                slot_rect.height,
                fill_byte,
            )?;
            let slot_material = if active {
                with_alpha(lcars.amber, 0.30)
            } else {
                with_alpha(lcars.action_fill(index), 0.22)
            };
            self.filled_rectangle(
                layers,
                0,
                rect(
                    slot_rect.x + 28.0,
                    slot_rect.y + 3.0,
                    slot_rect.width - 34.0,
                    slot_rect.height - 6.0,
                ),
                slot_material,
            )?;
            self.filled_rectangle(
                layers,
                0,
                rect(
                    slot_rect.x + 34.0,
                    slot_rect.y + 6.0,
                    slot_rect.width - 46.0,
                    slot_rect.height - 12.0,
                ),
                lcars.black,
            )?;
            if active {
                self.filled_rectangle(
                    layers,
                    0,
                    rect(
                        slot_rect.x - 4.0,
                        slot_rect.y + 3.0,
                        3.0,
                        slot_rect.height - 6.0,
                    ),
                    lcars.peach,
                )?;
                self.filled_rectangle(
                    layers,
                    0,
                    rect(
                        slot_rect.x + 30.0,
                        slot_rect.y + slot_rect.height - 5.0,
                        slot_rect.width * 0.46,
                        2.0,
                    ),
                    lcars.amber,
                )?;
            }
            let cols = ((slot_rect.width - 42.0) / cell_width).floor().max(4.0) as usize;
            self.paint_owt_panel_text(
                layers,
                slot_rect.x + 36.0,
                slot_rect.y + (slot_rect.height - cell_height).max(0.0) * 0.5,
                cols,
                &fit_text_ellipsis(&slot.label.to_uppercase(), cols),
                if active {
                    RgbColor::new_8bpc(255, 240, 176)
                } else {
                    RgbColor::new_8bpc(255, 204, 112)
                },
                true,
            )?;
        }

        self.paint_owt_lcars_surface_menu_status(layers, &layout, &state, &slots, lcars)?;
        let ruler_y = layout.y + layout.height - 18.0;
        self.filled_rectangle(
            layers,
            0,
            rect(layout.status_x, ruler_y, layout.status_width * 0.26, 4.0),
            lcars.peach,
        )?;
        self.filled_rectangle(
            layers,
            0,
            rect(
                layout.status_x + layout.status_width * 0.31,
                ruler_y,
                layout.status_width * 0.18,
                4.0,
            ),
            lcars.amber,
        )?;
        self.filled_rectangle(
            layers,
            0,
            rect(
                layout.status_x + layout.status_width * 0.54,
                ruler_y,
                layout.status_width * 0.28,
                4.0,
            ),
            lcars.violet,
        )?;
        if owt_lcars_design_grid_enabled() {
            paint_lcars_design_grid(
                self,
                layers,
                layout.x,
                layout.y,
                layout.width,
                layout.height,
            )?;
            paint_lcars_design_grid(
                self,
                layers,
                layout.command_x,
                layout.command_y,
                layout.command_width,
                layout.height - (layout.command_y - layout.y) - 16.0,
            )?;
            paint_lcars_design_grid(
                self,
                layers,
                layout.status_x,
                layout.status_y,
                layout.status_width,
                layout.status_height,
            )?;
        }
        Ok(())
    }

    fn paint_owt_lcars_surface_menu_connector(
        &mut self,
        layers: &mut TripleLayerQuadAllocator,
        layout: &LcarsSurfaceMenuLayout,
        lcars: LcarsPalette,
    ) -> anyhow::Result<()> {
        let target_x = layout.x + 24.0;
        let target_y = layout.y + 22.0;
        let dx = target_x - layout.anchor_x;
        let dy = target_y - layout.anchor_y;
        if dx.abs() < 18.0 && dy.abs() < 18.0 {
            return Ok(());
        }
        let elbow_x = clamp_ordered(
            layout.anchor_x + dx * 0.58,
            layout.anchor_x.min(target_x),
            layout.anchor_x.max(target_x),
        );
        let rail_h = 8.0;
        let rail_w = (elbow_x - layout.anchor_x).abs().max(12.0);
        let rail_x = layout.anchor_x.min(elbow_x);
        self.filled_rectangle(
            layers,
            0,
            rect(rail_x, layout.anchor_y - rail_h * 0.5, rail_w, rail_h),
            lcars.orange,
        )?;
        let vertical_y = layout.anchor_y.min(target_y);
        let vertical_h = (target_y - layout.anchor_y).abs().max(12.0);
        self.filled_rectangle(
            layers,
            0,
            rect(elbow_x - 3.0, vertical_y, 6.0, vertical_h),
            lcars.amber,
        )?;
        let final_w = (target_x - elbow_x).abs().max(12.0);
        let final_x = elbow_x.min(target_x);
        self.filled_rectangle(
            layers,
            0,
            rect(final_x, target_y - 3.0, final_w, 6.0),
            lcars.violet,
        )?;
        Ok(())
    }

    fn paint_owt_lcars_surface_menu_status(
        &mut self,
        layers: &mut TripleLayerQuadAllocator,
        layout: &LcarsSurfaceMenuLayout,
        state: &OwtLcarsSurfaceMenuState,
        slots: &[LcarsSurfaceMenuSlot],
        lcars: LcarsPalette,
    ) -> anyhow::Result<()> {
        let cell_width = self.render_metrics.cell_size.width as f32;
        let cell_height = self.render_metrics.cell_size.height as f32;
        let selected_detail = slots
            .get(state.selected_idx)
            .map(|slot| slot.detail.as_str())
            .or_else(|| state.status_line.as_deref())
            .unwrap_or("LCARS SURFACE MENU");
        let status = state.status_line.as_deref().unwrap_or(match state.mode {
            OwtLcarsSurfaceMenuMode::Main => "WINDOW SURFACE CONTROL",
            OwtLcarsSurfaceMenuMode::LoadSaved => "LOCAL PROFILE STORE",
        });
        let bay_x = layout.status_x;
        let bay_y = layout.status_y;
        let bay_width = layout.status_width;
        let bay_height = layout.status_height;
        self.filled_rectangle(
            layers,
            0,
            rect(bay_x, bay_y, bay_width, bay_height),
            lcars.black,
        )?;
        self.filled_rectangle(
            layers,
            0,
            rect(bay_x - 8.0, bay_y + 2.0, 5.0, bay_height - 4.0),
            lcars.amber,
        )?;
        self.filled_rectangle(
            layers,
            0,
            rect(
                bay_x + bay_width - 5.0,
                bay_y + 12.0,
                5.0,
                bay_height - 24.0,
            ),
            lcars.violet,
        )?;
        self.filled_rectangle(
            layers,
            0,
            rect(
                bay_x + bay_width * 0.34,
                bay_y + bay_height - 6.0,
                bay_width * 0.42,
                3.0,
            ),
            lcars.peach,
        )?;
        self.filled_rectangle(
            layers,
            0,
            rect(bay_x, bay_y, bay_width * 0.32, 4.0),
            lcars.peach,
        )?;
        self.filled_rectangle(
            layers,
            0,
            rect(bay_x + bay_width * 0.36, bay_y, bay_width * 0.24, 4.0),
            lcars.amber,
        )?;
        self.filled_rectangle(
            layers,
            0,
            rect(bay_x + bay_width * 0.66, bay_y, bay_width * 0.20, 4.0),
            lcars.violet,
        )?;
        let cols = ((bay_width - 18.0) / cell_width).floor().max(8.0) as usize;
        self.paint_owt_panel_text(
            layers,
            bay_x + 10.0,
            bay_y + 12.0,
            cols,
            &fit_text_ellipsis(status, cols),
            RgbColor::new_8bpc(255, 204, 112),
            true,
        )?;
        self.paint_owt_panel_text(
            layers,
            bay_x + 10.0,
            bay_y + 14.0 + cell_height,
            cols,
            &fit_text_ellipsis(selected_detail, cols),
            RgbColor::new_8bpc(164, 212, 255),
            true,
        )?;

        let mode = match state.mode {
            OwtLcarsSurfaceMenuMode::Main => "MODE LAYOUT",
            OwtLcarsSurfaceMenuMode::LoadSaved => "MODE LOAD",
        };
        let index_text = if slots.is_empty() {
            "SEL --".to_string()
        } else {
            format!("SEL {:02}", state.selected_idx.min(slots.len() - 1) + 1)
        };
        let meta_y = bay_y + bay_height - cell_height - 10.0;
        let meta_w = (bay_width * 0.26).clamp(cell_width * 8.0, cell_width * 14.0);
        self.filled_rectangle(
            layers,
            0,
            rect(bay_x + 10.0, meta_y - 4.0, meta_w, cell_height + 8.0),
            lcars.amber,
        )?;
        self.filled_rectangle(
            layers,
            0,
            rect(bay_x + 14.0, meta_y, meta_w - 10.0, cell_height),
            lcars.black,
        )?;
        let meta_cols = ((meta_w - 14.0) / cell_width).floor().max(6.0) as usize;
        self.paint_owt_panel_text(
            layers,
            bay_x + 18.0,
            meta_y,
            meta_cols,
            &fit_text_ellipsis(&index_text, meta_cols),
            RgbColor::new_8bpc(255, 204, 112),
            true,
        )?;
        let mode_cols = ((bay_width - meta_w - 34.0) / cell_width).floor().max(8.0) as usize;
        self.paint_owt_panel_text(
            layers,
            bay_x + meta_w + 24.0,
            meta_y,
            mode_cols,
            &fit_text_ellipsis(mode, mode_cols),
            RgbColor::new_8bpc(255, 149, 96),
            true,
        )
    }

    fn owt_lcars_floating_geometry(
        &self,
        document: &InterfaceDocument,
        default_left: f32,
        default_top: f32,
        default_width: f32,
        default_height: f32,
        min_width: f32,
        min_height: f32,
    ) -> Option<LcarsFloatingGeometry> {
        if lcars_surface_placement(document).layout != LcarsPanelLayout::Overlay {
            return None;
        }

        let margin = LCARS_PANEL_MARGIN;
        let viewport_width = self.dimensions.pixel_width as f32;
        let viewport_height = self.dimensions.pixel_height as f32;
        let max_width = (viewport_width - (margin * 2.0)).max(1.0);
        let max_height = (viewport_height - (margin * 2.0)).max(1.0);
        let width = lcars_float_property(
            document,
            &[
                "floating_width",
                "float_width",
                "widget_width",
                "surface_width",
            ],
        )
        .unwrap_or(default_width)
        .clamp(min_width.min(max_width), max_width);
        let height = lcars_float_property(
            document,
            &[
                "floating_height",
                "float_height",
                "widget_height",
                "surface_height",
            ],
        )
        .unwrap_or(default_height)
        .clamp(min_height.min(max_height), max_height);
        let anchor = lcars_root_property_value(
            document,
            &["floating_anchor", "float_anchor", "widget_anchor"],
        )
        .unwrap_or("top_left")
        .trim()
        .to_ascii_lowercase()
        .replace(['-', ' '], "_");
        let x = lcars_float_property(
            document,
            &["floating_x", "float_x", "widget_x", "surface_x"],
        );
        let y = lcars_float_property(
            document,
            &["floating_y", "float_y", "widget_y", "surface_y"],
        );
        let (mut left, mut top) = match (x, y) {
            (Some(x), Some(y)) if matches!(anchor.as_str(), "center" | "centre" | "cursor") => {
                (x - (width / 2.0), y - (height / 2.0))
            }
            (Some(x), Some(y)) => (x, y),
            _ => (default_left, default_top),
        };

        left = clamp_ordered(left, margin, viewport_width - width - margin);
        top = clamp_ordered(top, margin, viewport_height - height - margin);
        Some(LcarsFloatingGeometry {
            left,
            top,
            width,
            height,
        })
    }

    fn paint_owt_lcars_document(
        &mut self,
        layers: &mut TripleLayerQuadAllocator,
        document: &InterfaceDocument,
    ) -> anyhow::Result<()> {
        if !owt_lcars_native_surface_viewport_ready(
            self.dimensions.pixel_width as f32,
            self.dimensions.pixel_height as f32,
        ) {
            return Ok(());
        }

        let placement = lcars_surface_placement(document);
        let layout = placement.layout;
        let structural_mode = lcars_structural_mode(document);

        if lcars_corner_button_mode(document) {
            return self.paint_owt_lcars_corner_button(layers, document, placement);
        }

        if lcars_action_strip_mode(document) {
            return self.paint_owt_lcars_action_strip(layers, document, placement);
        }

        if matches!(structural_mode, LcarsStructuralMode::TheLcarsControlPanel) {
            return self.paint_owt_lcars_thelcars_control_panel(layers, document, placement);
        }

        if matches!(structural_mode, LcarsStructuralMode::BlockComposition) {
            match layout {
                LcarsPanelLayout::Left | LcarsPanelLayout::Right => {
                    return self
                        .paint_owt_lcars_block_composition_side_panel(layers, document, layout);
                }
                LcarsPanelLayout::Bottom => {
                    return self.paint_owt_lcars_block_composition_bottom_panel(layers, document);
                }
                LcarsPanelLayout::Top | LcarsPanelLayout::Overlay => {}
            }
        }
        if matches!(layout, LcarsPanelLayout::Left | LcarsPanelLayout::Right) {
            return self.paint_owt_lcars_side_panel(layers, document, layout);
        }
        if layout == LcarsPanelLayout::Bottom {
            return self.paint_owt_lcars_bottom_panel(layers, document);
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
        if has_structural_lcars_layout(document) {
            return self.paint_owt_lcars_structural_top_panel(layers, document, layout);
        }

        let Some(mut panel_height) =
            self.owt_lcars_top_panel_height_for(available_height, structural_mode, false)
        else {
            return Ok(());
        };

        let mut panel_width = (self.dimensions.pixel_width as f32 - (margin * 2.0)).max(360.0);
        let mut left = margin;
        let mut top = top;
        if let Some(floating) = self.owt_lcars_floating_geometry(
            document,
            left,
            top,
            panel_width.min(self.dimensions.pixel_width as f32 * 0.62),
            panel_height,
            360.0,
            cell_height * 8.0,
        ) {
            left = floating.left;
            top = floating.top;
            panel_width = floating.width;
            panel_height = floating.height;
        }
        let right = left + panel_width;
        let bottom = top + panel_height;

        let lcars = LcarsPalette::for_document(document);
        let panel_bg = lcars.panel_background(layout == LcarsPanelLayout::Overlay);
        let last_action_id = crate::owt_native::last_dispatched_action_for_interface(&document.id)
            .map(|action| action.action_id);

        self.filled_rectangle(
            layers,
            0,
            rect(left, top, panel_width, panel_height),
            panel_bg,
        )?;
        self.push_owt_lcars_surface_control_hitbox(
            &document.id,
            left,
            top,
            panel_width,
            panel_height,
        );

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
        let lines = panel_lines(document, max_cols);
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
            RgbColor::new_8bpc(255, 204, 112),
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
                Some(index + 1),
                lcars,
            )?;
            if let Some(action_id) = &line.action_id {
                self.ui_items.push(UIItem {
                    x: action_left.max(0.0) as usize,
                    y: action_y.max(0.0) as usize,
                    width: action_width.ceil().max(1.0) as usize,
                    height: button_height.ceil().max(1.0) as usize,
                    item_type: UIItemType::OwtLcarsAction {
                        interface_id: document.id.clone(),
                        action_id: action_id.clone(),
                    },
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
                RgbColor::new_8bpc(207, 79, 79),
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
        self.paint_owt_lcars_surface_focus_shell(
            layers,
            left,
            top,
            panel_width,
            panel_height,
            lcars,
        )?;
        if layout == LcarsPanelLayout::Overlay {
            self.paint_owt_lcars_surface_resize_handle(
                layers,
                left,
                top,
                panel_width,
                panel_height,
                lcars,
            )?;
            self.push_owt_lcars_surface_resize_hitbox(
                &document.id,
                left,
                top,
                panel_width,
                panel_height,
            );
        }

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
        let structural_mode = lcars_structural_mode(document);
        let Some(mut panel_height) =
            self.owt_lcars_top_panel_height_for(available_height, structural_mode, true)
        else {
            return Ok(());
        };

        let mut panel_width = (self.dimensions.pixel_width as f32 - (margin * 2.0)).max(520.0);
        let mut left = margin;
        let mut top = top;
        if let Some(floating) = self.owt_lcars_floating_geometry(
            document,
            left,
            top,
            panel_width.min(self.dimensions.pixel_width as f32 * 0.66),
            panel_height,
            520.0,
            cell_height * 12.0,
        ) {
            left = floating.left;
            top = floating.top;
            panel_width = floating.width;
            panel_height = floating.height;
        }
        let right = left + panel_width;
        let bottom = top + panel_height;
        let window_bottom = self.dimensions.pixel_height as f32
            - border.bottom.get() as f32
            - bottom_bar_height
            - margin;

        let lcars = LcarsPalette::for_document(document);
        let panel_bg = lcars.panel_background(layout == LcarsPanelLayout::Overlay);
        let last_action_id = crate::owt_native::last_dispatched_action_for_interface(&document.id)
            .map(|action| action.action_id);
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
        self.push_owt_lcars_surface_control_hitbox(
            &document.id,
            left,
            top,
            panel_width,
            panel_height,
        );

        let rail_left = left + 10.0;
        let rail_width = 108.0;
        let content_left = left + LCARS_LEFT_RAIL_RESERVED;
        let content_right = right - 12.0;
        let content_width = (content_right - content_left).max(320.0);
        let header_y = top + 10.0;
        let header_h = 42.0;
        let docked_surface = matches!(structural_mode, LcarsStructuralMode::DockedSurface);
        let block_composition = matches!(structural_mode, LcarsStructuralMode::BlockComposition);
        let rail_bottom = window_bottom.max(bottom);
        if docked_surface {
            paint_lcars_docked_side_rail_chrome(
                self,
                layers,
                rail_left,
                top + 10.0,
                rail_width,
                (rail_bottom - top - 18.0).max(panel_height),
                lcars,
            )?;
            paint_lcars_docked_primary_elbow(
                self,
                layers,
                rail_left,
                header_y,
                (content_left - rail_left + (content_width * 0.50)).clamp(380.0, 900.0),
                (panel_height * 0.76).clamp(208.0, 310.0),
                rail_width,
                header_h,
                lcars,
            )?;
            paint_lcars_docked_header_run(
                self,
                layers,
                content_left,
                header_y,
                content_width,
                header_h,
                lcars,
            )?;
        } else {
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
                (content_left - rail_left + (content_width * 0.42)).clamp(320.0, 780.0),
                (panel_height * 0.70).clamp(188.0, 282.0),
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
        }
        self.push_owt_lcars_surface_control_hitbox(
            &document.id,
            rail_left,
            top + 10.0,
            rail_width,
            (rail_bottom - top - 18.0).max(panel_height),
        );

        if !docked_surface {
            paint_lcars_minor_bar_rhythm(
                self,
                layers,
                content_left,
                header_y + header_h + 9.0,
                content_width,
                8.0,
                lcars,
            )?;
        }

        let promote_detail = matches!(structural_mode, LcarsStructuralMode::Composition);
        let has_table =
            promote_detail && document_has_kind(document, |kind| matches!(kind, UiNodeKind::Table));
        let has_data_cascade = promote_detail
            && document_has_kind(document, |kind| matches!(kind, UiNodeKind::DataCascade));
        let all_action_lines = lines
            .items
            .iter()
            .filter(|line| line.action_id.is_some())
            .collect::<Vec<_>>();
        let action_page = self.owt_lcars_current_action_page(document);
        let action_lines = all_action_lines
            .iter()
            .copied()
            .skip(action_page.saturating_mul(LCARS_KEY_ACTION_LIMIT))
            .take(LCARS_KEY_ACTION_LIMIT)
            .collect::<Vec<_>>();
        let scene = compute_lcars_structural_scene(
            content_left,
            content_right,
            top,
            bottom,
            window_bottom,
            cell_width,
            cell_height,
            has_table,
            has_data_cascade,
            all_action_lines.len(),
            structural_mode,
        );
        let layout_plan = scene.plan;
        let signal_top = scene.signal_field.y;
        let signal_limit = scene.signal_field.bottom();
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
        let button_height = scene
            .command_bank
            .map(|bank| bank.button_height)
            .unwrap_or_else(|| (cell_height * 2.15).clamp(36.0, 46.0));
        let button_gap = scene
            .command_bank
            .map(|bank| bank.button_gap)
            .unwrap_or(12.0);
        let two_columns = layout_plan.two_action_columns;
        let button_width = scene
            .command_bank
            .map(|bank| bank.button_width)
            .unwrap_or(layout_plan.command_width);

        self.paint_owt_panel_text(
            layers,
            content_left + 12.0,
            header_y + 6.0,
            ((content_width * 0.46) / cell_width).max(8.0) as usize,
            &lines.title,
            RgbColor::new_8bpc(0, 0, 0),
            true,
        )?;

        paint_lcars_structural_console_chrome(
            self,
            layers,
            content_left,
            content_right,
            &scene,
            lcars,
        )?;
        paint_lcars_content_bay_frame(
            self,
            layers,
            scene.content_bay.x,
            scene.content_bay.y,
            scene.content_bay.width,
            scene.content_bay.height,
            docked_surface,
            lcars,
        )?;

        paint_lcars_embedded_label_tab(
            self,
            layers,
            scene.scope_tab.x,
            scene.scope_tab.y,
            scene.scope_tab.width,
            scene.scope_tab.height,
            LCARS_BYTE_BLUE,
            lcars,
        )?;
        self.push_owt_lcars_surface_control_hitbox(
            &document.id,
            scene.scope_tab.x,
            scene.scope_tab.y,
            scene.scope_tab.width,
            scene.scope_tab.height,
        );
        let scope_cols = (scene.scope_tab.width / cell_width).floor().max(1.0) as usize;
        let scope_text_cols = scope_cols.saturating_sub(5).max(4);
        let scope_text = fit_text_ellipsis(&lines.scope, scope_text_cols);
        self.paint_owt_panel_text(
            layers,
            scene.scope_tab.x + 26.0,
            scene.scope_tab.y + 6.0,
            scope_text_cols,
            &scope_text,
            RgbColor::new_8bpc(153, 204, 255),
            true,
        )?;

        paint_lcars_embedded_label_tab(
            self,
            layers,
            scene.content_label.x,
            scene.content_label.y,
            scene.content_label.width,
            scene.content_label.height,
            LCARS_BYTE_BLUE,
            lcars,
        )?;
        let content_label_cols = (scene.content_label.width / cell_width).floor().max(1.0) as usize;
        let content_label_text_cols = content_label_cols.saturating_sub(4).max(4);
        self.paint_owt_panel_text(
            layers,
            scene.content_label.x + 24.0,
            scene.content_label.y + 5.0,
            content_label_text_cols,
            &fit_text_ellipsis("CONTENT BAY", content_label_text_cols),
            RgbColor::new_8bpc(153, 204, 255),
            true,
        )?;

        if let Some(command_bank) = scene.command_bank {
            paint_lcars_embedded_label_tab(
                self,
                layers,
                command_bank.label.x,
                command_bank.label.y,
                command_bank.label.width,
                command_bank.label.height,
                LCARS_BYTE_VIOLET,
                lcars,
            )?;
            let command_label_cols =
                (command_bank.label.width / cell_width).floor().max(1.0) as usize;
            let command_label = if matches!(structural_mode, LcarsStructuralMode::PrimitiveLegend) {
                "ACTION TEST".to_string()
            } else if block_composition {
                lcars_block_composition_data(document).command_bank_title
            } else {
                "COMMAND BANK".to_string()
            };
            self.paint_owt_panel_text(
                layers,
                command_bank.label.x + 24.0,
                command_bank.label.y + 6.0,
                command_label_cols.saturating_sub(5).max(4),
                &fit_text_ellipsis(&command_label, command_label_cols.saturating_sub(5).max(4)),
                RgbColor::new_8bpc(255, 149, 96),
                true,
            )?;
        }

        if matches!(structural_mode, LcarsStructuralMode::PrimitiveLegend) {
            paint_lcars_primitive_legend(
                self,
                layers,
                &scene,
                document,
                cell_width,
                cell_height,
                lcars,
            )?;
        } else if block_composition {
            paint_lcars_block_composition(
                self,
                layers,
                &scene,
                document,
                cell_width,
                cell_height,
                lcars,
            )?;
        } else {
            let mut signal_y = signal_top;
            let signal_lines = lines
                .items
                .iter()
                .filter(|line| line.action_id.is_none())
                .filter(|line| !is_structural_chrome_signal(line.kind))
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
                    docked_surface,
                )?;
                paint_lcars_builder_highlight_outline(
                    self,
                    layers,
                    content_left + 12.0,
                    signal_y - 3.0,
                    (layout_plan.signal_width - 24.0).max(1.0),
                    (cell_height * text_rows as f32 + 6.0).max(cell_height),
                    line,
                    index,
                    cell_width,
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
            if !docked_surface
                && hidden_signals > 0
                && signal_y + cell_height <= signal_visual_limit
            {
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
                    RgbColor::new_8bpc(255, 149, 96),
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
        }

        let mut action_count = 0usize;
        let action_slot_limit = if matches!(structural_mode, LcarsStructuralMode::PrimitiveLegend) {
            layout_plan.action_slots.min(1)
        } else {
            layout_plan.action_slots
        };
        for (index, line) in action_lines
            .iter()
            .copied()
            .take(action_slot_limit)
            .enumerate()
        {
            let col = if two_columns { index % 2 } else { 0 };
            let row = if two_columns { index / 2 } else { index };
            let x = layout_plan.command_left + (col as f32 * (button_width + button_gap));
            let y = scene
                .command_bank
                .map(|bank| bank.button_top)
                .unwrap_or(top + 92.0)
                + (row as f32 * (button_height + button_gap));
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
                Some(index + 1),
                lcars,
            )?;
            if let Some(action_id) = &line.action_id {
                self.ui_items.push(UIItem {
                    x: x.max(0.0) as usize,
                    y: y.max(0.0) as usize,
                    width: button_width.ceil().max(1.0) as usize,
                    height: button_height.ceil().max(1.0) as usize,
                    item_type: UIItemType::OwtLcarsAction {
                        interface_id: document.id.clone(),
                        action_id: action_id.clone(),
                    },
                });
            }
            action_count += 1;
        }
        let page_count = all_action_lines
            .len()
            .max(1)
            .div_ceil(LCARS_KEY_ACTION_LIMIT);
        let hidden_actions = all_action_lines
            .len()
            .saturating_sub(action_page.saturating_mul(LCARS_KEY_ACTION_LIMIT) + action_count);
        if layout_plan.command_visible && hidden_actions > 0 {
            self.paint_owt_panel_text(
                layers,
                layout_plan.command_left + 12.0,
                (signal_limit - cell_height).max(top + 92.0),
                action_cols,
                &if page_count > 1 {
                    format!(
                        "ACTIONS {}/{}  +{}",
                        action_page + 1,
                        page_count,
                        hidden_actions
                    )
                } else {
                    format!("+{hidden_actions} ACTIONS")
                },
                RgbColor::new_8bpc(255, 204, 112),
                true,
            )?;
        } else if layout_plan.command_visible && page_count > 1 {
            self.paint_owt_panel_text(
                layers,
                layout_plan.command_left + 12.0,
                (signal_limit - cell_height).max(top + 92.0),
                action_cols,
                &format!("ACTIONS {}/{}", action_page + 1, page_count),
                RgbColor::new_8bpc(255, 204, 112),
                true,
            )?;
        }

        if !docked_surface {
            let bay_label = if last_action_id.is_some() {
                "ACK STATE"
            } else if matches!(structural_mode, LcarsStructuralMode::PrimitiveLegend) {
                "LEGEND READY"
            } else if action_count > 0 {
                "KEY READY"
            } else {
                "FRAME READY"
            };
            paint_lcars_embedded_label_tab(
                self,
                layers,
                scene.status_tab.x,
                scene.status_tab.y,
                scene.status_tab.width,
                scene.status_tab.height,
                if last_action_id.is_some() {
                    LCARS_BYTE_PEACH
                } else {
                    LCARS_BYTE_BLUE
                },
                lcars,
            )?;
            let bay_tab_cols = (scene.status_tab.width / cell_width).floor().max(1.0) as usize;
            let bay_label_cols = bay_tab_cols.saturating_sub(4).max(4);
            self.paint_owt_panel_text(
                layers,
                scene.status_tab.x + 24.0,
                scene.status_tab.y + 5.0,
                bay_label_cols,
                &fit_text_ellipsis(bay_label, bay_label_cols),
                if last_action_id.is_some() {
                    RgbColor::new_8bpc(255, 149, 96)
                } else {
                    RgbColor::new_8bpc(153, 204, 255)
                },
                true,
            )?;
        }
        self.paint_owt_lcars_surface_focus_shell(
            layers,
            left,
            top,
            panel_width,
            panel_height,
            lcars,
        )?;
        if layout == LcarsPanelLayout::Overlay {
            self.paint_owt_lcars_surface_resize_handle(
                layers,
                left,
                top,
                panel_width,
                panel_height,
                lcars,
            )?;
            self.push_owt_lcars_surface_resize_hitbox(
                &document.id,
                left,
                top,
                panel_width,
                panel_height,
            );
        }

        Ok(())
    }

    fn paint_owt_lcars_thelcars_control_panel(
        &mut self,
        layers: &mut TripleLayerQuadAllocator,
        document: &InterfaceDocument,
        placement: LcarsSurfacePlacement,
    ) -> anyhow::Result<()> {
        let cell_width = self.render_metrics.cell_size.width as f32;
        let cell_height = self.render_metrics.cell_size.height as f32;
        let border = self.get_os_border();
        let tab_bar_height = if self.show_tab_bar && !self.config.tab_bar_at_bottom {
            self.tab_bar_pixel_height()
                .context("compute tab bar height for OWT TheLCARS panel")?
        } else {
            0.0
        };
        let bottom_tab_bar_height = if self.show_tab_bar && self.config.tab_bar_at_bottom {
            self.tab_bar_pixel_height()
                .context("compute bottom tab bar height for OWT TheLCARS panel")?
        } else {
            0.0
        };

        let margin = LCARS_PANEL_MARGIN;
        let viewport_width = self.dimensions.pixel_width as f32;
        let viewport_height = self.dimensions.pixel_height as f32;
        let surface_top = border.top.get() as f32 + tab_bar_height + margin;
        let surface_bottom =
            viewport_height - border.bottom.get() as f32 - bottom_tab_bar_height - margin;
        let available_height = (surface_bottom - surface_top).max(1.0);
        let available_width = (viewport_width - (margin * 2.0)).max(1.0);
        let (mut left, mut top, mut panel_width, mut panel_height) = match placement.layout {
            LcarsPanelLayout::Left | LcarsPanelLayout::Right => {
                let Some(panel_width) = self.owt_lcars_side_panel_width_for(
                    available_width,
                    LcarsStructuralMode::TheLcarsControlPanel,
                ) else {
                    return Ok(());
                };
                let left = if placement.layout == LcarsPanelLayout::Left {
                    margin
                } else {
                    viewport_width - margin - panel_width
                };
                (left, surface_top, panel_width, available_height)
            }
            LcarsPanelLayout::Bottom => {
                let Some(panel_height) = self.owt_lcars_bottom_panel_height_for(
                    available_height,
                    LcarsStructuralMode::TheLcarsControlPanel,
                ) else {
                    return Ok(());
                };
                (
                    margin,
                    surface_bottom - panel_height,
                    available_width,
                    panel_height,
                )
            }
            LcarsPanelLayout::Top | LcarsPanelLayout::Overlay => {
                let Some(panel_height) = self.owt_lcars_top_panel_height_for(
                    available_height,
                    LcarsStructuralMode::TheLcarsControlPanel,
                    true,
                ) else {
                    return Ok(());
                };
                (margin, surface_top, available_width, panel_height)
            }
        };
        if let Some(floating) = self.owt_lcars_floating_geometry(
            document,
            left,
            top,
            panel_width.min(viewport_width * 0.66),
            panel_height,
            520.0,
            cell_height * 14.0,
        ) {
            left = floating.left;
            top = floating.top;
            panel_width = floating.width;
            panel_height = floating.height;
        }
        let palette = LcarsPalette::for_document(document);
        let panel_bg = palette.panel_background(placement.layout == LcarsPanelLayout::Overlay);
        let last_action_id = crate::owt_native::last_dispatched_action_for_interface(&document.id)
            .map(|action| action.action_id);
        let lines = panel_lines(
            document,
            ((panel_width - 80.0) / cell_width).floor().max(8.0) as usize,
        );
        let demo = thelcars_demo_metadata(document);
        let action_lines = lines
            .items
            .iter()
            .filter(|line| line.action_id.is_some())
            .collect::<Vec<_>>();
        let signal_lines = thelcars_signal_lines(&lines.items, 10);
        let Some(grid) = compute_thelcars_cockpit_grid(
            LcarsSceneRect {
                x: left,
                y: top,
                width: panel_width,
                height: panel_height,
            },
            placement,
            cell_width,
            cell_height,
            action_lines.len(),
            signal_lines.len(),
        ) else {
            return Ok(());
        };

        self.filled_rectangle(
            layers,
            0,
            rect(
                grid.panel.x,
                grid.panel.y,
                grid.panel.width,
                grid.panel.height,
            ),
            panel_bg,
        )?;
        self.push_owt_lcars_surface_control_hitbox(
            &document.id,
            grid.panel.x,
            grid.panel.y,
            grid.panel.width,
            grid.panel.height,
        );

        if matches!(
            grid.placement.layout,
            LcarsPanelLayout::Top | LcarsPanelLayout::Overlay
        ) && grid.header.width >= 720.0
        {
            paint_lcars_thelcars_header(
                self,
                layers,
                grid.header.x,
                grid.header.y,
                grid.header.width,
                grid.header.height,
                grid.rail.width,
                &lines.title,
                palette,
            )?;
        } else {
            paint_lcars_thelcars_compact_header(
                self,
                layers,
                grid.header,
                &lines.title,
                cell_width,
                cell_height,
                palette,
            )?;
        }
        paint_lcars_thelcars_cockpit_backbone(self, layers, &grid)?;
        if grid.rail.height >= 190.0 && grid.rail.width >= 110.0 {
            paint_lcars_thelcars_nav_rail(
                self,
                layers,
                grid.rail.x,
                grid.rail.y,
                grid.rail.width,
                grid.rail.height,
                &lines.items,
                palette,
            )?;
        } else {
            paint_lcars_thelcars_compact_rail(self, layers, grid.rail, &lines.items, palette)?;
        }

        paint_lcars_embedded_label_tab(
            self,
            layers,
            grid.scope_tab.x,
            grid.scope_tab.y,
            grid.scope_tab.width,
            grid.scope_tab.height,
            LCARS_BYTE_BLUE,
            palette,
        )?;
        let scope_cols = ((grid.scope_tab.width - 36.0) / cell_width)
            .floor()
            .max(4.0) as usize;
        self.paint_owt_panel_text(
            layers,
            grid.scope_tab.x + 30.0,
            grid.scope_tab.y + 6.0,
            scope_cols,
            &fit_text_ellipsis(&lines.scope, scope_cols),
            RgbColor::new_8bpc(153, 204, 255),
            true,
        )?;

        if let Some(theme_strip) = grid.theme_strip {
            paint_lcars_thelcars_theme_strip(
                self,
                layers,
                theme_strip.x,
                theme_strip.y,
                theme_strip.width,
                theme_strip.height,
                &demo,
                cell_width,
                cell_height,
                palette,
            )?;
        }

        let split_at = signal_lines.len().min(5);
        let (primary_lines, secondary_lines) = signal_lines.split_at(split_at);
        paint_lcars_thelcars_signal_bay(
            self,
            layers,
            grid.primary_bay.x,
            grid.primary_bay.y,
            grid.primary_bay.width,
            grid.primary_bay.height,
            "PRIMARY BAY",
            primary_lines,
            cell_width,
            cell_height,
            palette,
        )?;
        if let Some(secondary_bay) = grid.secondary_bay {
            paint_lcars_thelcars_signal_bay(
                self,
                layers,
                secondary_bay.x,
                secondary_bay.y,
                secondary_bay.width,
                secondary_bay.height,
                "TELEMETRY",
                secondary_lines,
                cell_width,
                cell_height,
                palette,
            )?;
        }

        if let Some(chart_strip) = grid.chart_strip {
            paint_lcars_thelcars_chart_strip(
                self,
                layers,
                chart_strip.x,
                chart_strip.y,
                chart_strip.width,
                chart_strip.height,
                &demo,
                cell_width,
                cell_height,
                palette,
            )?;
        }
        if let Some(workspace) = grid.terminal_workspace {
            paint_lcars_thelcars_workspace_frame(self, layers, workspace, cell_width, palette)?;
        }

        if let Some(command_stack) = grid.command_stack {
            paint_lcars_thelcars_command_stack(
                self,
                layers,
                command_stack.x,
                command_stack.y,
                command_stack.width,
                command_stack.height,
                &action_lines,
                &document.id,
                last_action_id.as_deref(),
                cell_width,
                cell_height,
                palette,
            )?;
        }

        self.paint_owt_lcars_surface_focus_shell(
            layers,
            grid.panel.x,
            grid.panel.y,
            grid.panel.width,
            grid.panel.height,
            palette,
        )?;
        if placement.layout == LcarsPanelLayout::Overlay {
            self.paint_owt_lcars_surface_resize_handle(
                layers,
                grid.panel.x,
                grid.panel.y,
                grid.panel.width,
                grid.panel.height,
                palette,
            )?;
            self.push_owt_lcars_surface_resize_hitbox(
                &document.id,
                grid.panel.x,
                grid.panel.y,
                grid.panel.width,
                grid.panel.height,
            );
        }

        Ok(())
    }

    fn paint_owt_lcars_block_composition_side_panel(
        &mut self,
        layers: &mut TripleLayerQuadAllocator,
        document: &InterfaceDocument,
        layout: LcarsPanelLayout,
    ) -> anyhow::Result<()> {
        let cell_width = self.render_metrics.cell_size.width as f32;
        let cell_height = self.render_metrics.cell_size.height as f32;
        let border = self.get_os_border();
        let top_bar_height = if self.show_tab_bar && !self.config.tab_bar_at_bottom {
            self.tab_bar_pixel_height()
                .context("compute tab bar height for OWT LCARS block side panel")?
        } else {
            0.0
        };
        let bottom_bar_height = if self.show_tab_bar && self.config.tab_bar_at_bottom {
            self.tab_bar_pixel_height()
                .context("compute bottom tab bar height for OWT LCARS block side panel")?
        } else {
            0.0
        };

        let margin = LCARS_PANEL_MARGIN;
        let available_width = self.dimensions.pixel_width as f32 - (margin * 2.0);
        let Some(panel_width) = self
            .owt_lcars_side_panel_width_for(available_width, LcarsStructuralMode::BlockComposition)
        else {
            return Ok(());
        };
        let left = if layout == LcarsPanelLayout::Left {
            margin
        } else {
            self.dimensions.pixel_width as f32 - margin - panel_width
        };
        let top = border.top.get() as f32 + top_bar_height + margin;
        let bottom = self.dimensions.pixel_height as f32
            - border.bottom.get() as f32
            - bottom_bar_height
            - margin;
        let panel_height = (bottom - top).max(0.0);
        if panel_height < cell_height * 14.0 {
            return Ok(());
        }

        let lcars = LcarsPalette::for_document(document);
        let lines = panel_lines(
            document,
            ((panel_width - 40.0) / cell_width).max(8.0) as usize,
        );
        let action_lines = lines
            .items
            .iter()
            .filter(|line| line.action_id.is_some())
            .collect::<Vec<_>>();
        let last_action_id = crate::owt_native::last_dispatched_action_for_interface(&document.id)
            .map(|action| action.action_id);

        self.filled_rectangle(
            layers,
            0,
            rect(left, top, panel_width, panel_height),
            lcars.black,
        )?;
        self.push_owt_lcars_surface_control_hitbox(
            &document.id,
            left,
            top,
            panel_width,
            panel_height,
        );

        let rail_width = (panel_width * 0.20).clamp(70.0, 92.0);
        let rail_x = if layout == LcarsPanelLayout::Left {
            left + 8.0
        } else {
            left + panel_width - rail_width - 8.0
        };
        let content_left = if layout == LcarsPanelLayout::Left {
            rail_x + rail_width + 14.0
        } else {
            left + 14.0
        };
        let content_right = if layout == LcarsPanelLayout::Left {
            left + panel_width - 12.0
        } else {
            rail_x - 14.0
        };
        let content_width = (content_right - content_left).max(cell_width * 18.0);
        paint_lcars_interface_switcher_spine(
            self,
            layers,
            rail_x,
            top + 12.0,
            rail_width,
            panel_height - 24.0,
            lcars,
        )?;
        self.push_owt_lcars_surface_control_hitbox(
            &document.id,
            rail_x,
            top + 12.0,
            rail_width,
            panel_height - 24.0,
        );

        let header_h = (cell_height + 10.0).clamp(25.0, 32.0);
        paint_lcars_embedded_label_tab(
            self,
            layers,
            content_left,
            top + 12.0,
            content_width,
            header_h,
            LCARS_BYTE_BLUE,
            lcars,
        )?;
        self.push_owt_lcars_surface_control_hitbox(
            &document.id,
            content_left,
            top + 12.0,
            content_width,
            header_h,
        );
        let title_cols = ((content_width - 38.0) / cell_width).floor().max(5.0) as usize;
        self.paint_owt_panel_text(
            layers,
            content_left + 26.0,
            top + 17.0,
            title_cols,
            &fit_text_ellipsis(&lines.title, title_cols),
            RgbColor::new_8bpc(153, 204, 255),
            true,
        )?;

        let action_count = action_lines.len().min(3);
        let button_height = (cell_height * 1.55).clamp(27.0, 36.0);
        let button_gap = 8.0;
        let command_height = if action_count > 0 {
            header_h
                + 10.0
                + (action_count as f32 * button_height)
                + (action_count.saturating_sub(1) as f32 * button_gap)
        } else {
            header_h + 38.0
        };
        let body_top = top + 12.0 + header_h + 16.0;
        let body_bottom = bottom - 16.0 - command_height;
        let body_height = (body_bottom - body_top).max(cell_height * 7.0);
        let data = lcars_block_composition_data(document);
        let gap = 12.0;
        let data_height = if body_height >= cell_height * 12.0 {
            clamp_ordered(
                body_height * 0.44,
                cell_height * 5.4,
                body_height - cell_height * 5.4,
            )
        } else {
            (body_height * 0.58).max(cell_height * 5.0)
        };
        paint_lcars_labelled_data_block(
            self,
            layers,
            content_left,
            body_top,
            content_width,
            data_height,
            &data,
            cell_width,
            cell_height,
            lcars,
        )?;

        let graphics_y = body_top + data_height + gap;
        let graphics_height = (body_bottom - graphics_y).max(0.0);
        if graphics_height >= cell_height * 4.6 {
            paint_lcars_block_detail_bay(
                self,
                layers,
                document,
                content_left,
                graphics_y,
                content_width,
                graphics_height,
                &data,
                cell_width,
                cell_height,
                lcars,
            )?;
        }

        let command_y = bottom - command_height - 10.0;
        let command_label_w = content_width.min(cell_width * 18.0).max(cell_width * 10.0);
        paint_lcars_embedded_label_tab(
            self,
            layers,
            content_left,
            command_y,
            command_label_w,
            header_h,
            LCARS_BYTE_VIOLET,
            lcars,
        )?;
        self.paint_owt_panel_text(
            layers,
            content_left + 24.0,
            command_y + 5.0,
            ((command_label_w - 34.0) / cell_width).floor().max(4.0) as usize,
            &fit_text_ellipsis(
                &data.command_bank_title,
                ((command_label_w - 34.0) / cell_width).floor().max(4.0) as usize,
            ),
            RgbColor::new_8bpc(255, 149, 96),
            true,
        )?;

        let stack_y = command_y + header_h + 8.0;
        let stack_height = (bottom - stack_y - 12.0).max(button_height);
        let painted = self.paint_owt_lcars_action_stack(
            layers,
            &action_lines,
            content_left,
            stack_y,
            content_width,
            stack_height,
            3,
            button_height,
            button_gap,
            &document.id,
            last_action_id.as_deref(),
            lcars,
        )?;
        if painted == 0 {
            paint_lcars_surface_menu_slab(
                self,
                layers,
                content_left,
                stack_y,
                content_width,
                button_height,
                "SURFACE MENU",
                lcars,
            )?;
            self.push_owt_lcars_surface_control_hitbox(
                &document.id,
                content_left,
                stack_y,
                content_width,
                button_height,
            );
        }
        self.paint_owt_lcars_surface_focus_shell(
            layers,
            left,
            top,
            panel_width,
            panel_height,
            lcars,
        )?;

        Ok(())
    }

    fn paint_owt_lcars_block_composition_bottom_panel(
        &mut self,
        layers: &mut TripleLayerQuadAllocator,
        document: &InterfaceDocument,
    ) -> anyhow::Result<()> {
        let cell_width = self.render_metrics.cell_size.width as f32;
        let cell_height = self.render_metrics.cell_size.height as f32;
        let border = self.get_os_border();
        let bottom_bar_height = if self.show_tab_bar && self.config.tab_bar_at_bottom {
            self.tab_bar_pixel_height()
                .context("compute bottom tab bar height for OWT LCARS block bottom panel")?
        } else {
            0.0
        };

        let margin = LCARS_PANEL_MARGIN;
        let available_height = self.dimensions.pixel_height as f32 - (margin * 2.0);
        let Some(panel_height) = self.owt_lcars_bottom_panel_height_for(
            available_height,
            LcarsStructuralMode::BlockComposition,
        ) else {
            return Ok(());
        };
        let left = margin;
        let width = (self.dimensions.pixel_width as f32 - (margin * 2.0)).max(520.0);
        let bottom = self.dimensions.pixel_height as f32
            - border.bottom.get() as f32
            - bottom_bar_height
            - margin;
        let top = bottom - panel_height;

        let lcars = LcarsPalette::for_document(document);
        let lines = panel_lines(document, ((width - 80.0) / cell_width).max(8.0) as usize);
        let action_lines = lines
            .items
            .iter()
            .filter(|line| line.action_id.is_some())
            .collect::<Vec<_>>();
        let last_action_id = crate::owt_native::last_dispatched_action_for_interface(&document.id)
            .map(|action| action.action_id);

        self.filled_rectangle(layers, 0, rect(left, top, width, panel_height), lcars.black)?;
        self.push_owt_lcars_surface_control_hitbox(&document.id, left, top, width, panel_height);
        let strip_width = (width * 0.16).clamp(150.0, 220.0);
        paint_lcars_interface_switcher_strip(
            self,
            layers,
            left + 10.0,
            top + 12.0,
            strip_width,
            panel_height - 24.0,
            lcars,
        )?;
        self.push_owt_lcars_surface_control_hitbox(
            &document.id,
            left + 10.0,
            top + 12.0,
            strip_width,
            panel_height - 24.0,
        );

        let body_y = top + 16.0;
        let body_h = panel_height - 32.0;
        let action_width = if action_lines.is_empty() {
            (width * 0.14).clamp(140.0, 210.0)
        } else {
            (width * 0.22).clamp(250.0, 380.0)
        };
        let action_left = left + width - action_width - 14.0;
        let content_left = left + 18.0 + strip_width + 16.0;
        let content_right = action_left - 16.0;
        let content_width = (content_right - content_left).max(cell_width * 26.0);
        let data = lcars_block_composition_data(document);
        let show_detail = data.has_graphics
            || first_node_matching(document, |node| node.kind == UiNodeKind::Table).is_some();
        let graphics_width = if show_detail {
            clamp_ordered(
                content_width * 0.42,
                cell_width * 24.0,
                content_width * 0.52,
            )
        } else {
            0.0
        };
        let data_width = if show_detail {
            (content_width - graphics_width - 14.0).max(cell_width * 24.0)
        } else {
            content_width
        };
        paint_lcars_labelled_data_block(
            self,
            layers,
            content_left,
            body_y,
            data_width,
            body_h,
            &data,
            cell_width,
            cell_height,
            lcars,
        )?;
        if show_detail {
            paint_lcars_block_detail_bay(
                self,
                layers,
                document,
                content_left + data_width + 14.0,
                body_y,
                graphics_width,
                body_h,
                &data,
                cell_width,
                cell_height,
                lcars,
            )?;
        }

        let label_h = (cell_height + 8.0).clamp(24.0, 31.0);
        paint_lcars_embedded_label_tab(
            self,
            layers,
            action_left,
            body_y,
            action_width,
            label_h,
            LCARS_BYTE_VIOLET,
            lcars,
        )?;
        self.paint_owt_panel_text(
            layers,
            action_left + 24.0,
            body_y + 5.0,
            ((action_width - 34.0) / cell_width).floor().max(4.0) as usize,
            &fit_text_ellipsis(
                &data.command_bank_title,
                ((action_width - 34.0) / cell_width).floor().max(4.0) as usize,
            ),
            RgbColor::new_8bpc(255, 149, 96),
            true,
        )?;
        let button_height = (cell_height * 1.55).clamp(28.0, 38.0);
        let button_gap = 8.0;
        let stack_y = body_y + label_h + 9.0;
        let painted = self.paint_owt_lcars_action_stack(
            layers,
            &action_lines,
            action_left,
            stack_y,
            action_width,
            body_h - label_h - 9.0,
            3,
            button_height,
            button_gap,
            &document.id,
            last_action_id.as_deref(),
            lcars,
        )?;
        if painted == 0 {
            paint_lcars_surface_menu_slab(
                self,
                layers,
                action_left,
                stack_y,
                action_width,
                button_height,
                "SURFACE MENU",
                lcars,
            )?;
            self.push_owt_lcars_surface_control_hitbox(
                &document.id,
                action_left,
                stack_y,
                action_width,
                button_height,
            );
        }
        self.paint_owt_lcars_surface_focus_shell(layers, left, top, width, panel_height, lcars)?;

        Ok(())
    }

    fn paint_owt_lcars_side_panel(
        &mut self,
        layers: &mut TripleLayerQuadAllocator,
        document: &InterfaceDocument,
        layout: LcarsPanelLayout,
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
        let Some(panel_width) =
            self.owt_lcars_side_panel_width_for(available_width, lcars_structural_mode(document))
        else {
            return Ok(());
        };
        let left = if layout == LcarsPanelLayout::Left {
            margin
        } else {
            self.dimensions.pixel_width as f32 - margin - panel_width
        };
        let top = border.top.get() as f32 + top_bar_height + margin;
        let bottom = self.dimensions.pixel_height as f32
            - border.bottom.get() as f32
            - bottom_bar_height
            - margin;
        let panel_height = (bottom - top).max(0.0);
        if panel_height < cell_height * 9.0 {
            return Ok(());
        }

        let lcars = LcarsPalette::for_document(document);
        let panel_bg = lcars.black;
        let last_action_id = crate::owt_native::last_dispatched_action_for_interface(&document.id)
            .map(|action| action.action_id);

        self.filled_rectangle(
            layers,
            0,
            rect(left, top, panel_width, panel_height),
            panel_bg,
        )?;
        self.push_owt_lcars_surface_control_hitbox(
            &document.id,
            left,
            top,
            panel_width,
            panel_height,
        );
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
            RgbColor::new_8bpc(255, 204, 112),
            false,
        )?;

        let mut signal_y = top + 58.0 + (cell_height * 1.35);
        let action_start_floor = bottom - 156.0;
        for (index, line) in prioritized_side_panel_signal_lines(&lines.items, 7)
            .into_iter()
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
                line.hotkey,
                lcars,
            )?;
            if let Some(action_id) = &line.action_id {
                self.ui_items.push(UIItem {
                    x: text_left.max(0.0) as usize,
                    y: action_y.max(0.0) as usize,
                    width: button_width.ceil().max(1.0) as usize,
                    height: button_height.ceil().max(1.0) as usize,
                    item_type: UIItemType::OwtLcarsAction {
                        interface_id: document.id.clone(),
                        action_id: action_id.clone(),
                    },
                });
            }
            action_count += 1;
            action_y += button_height + 8.0;
        }

        let rail_label = if layout == LcarsPanelLayout::Left {
            "LEFT RAIL"
        } else {
            "RIGHT RAIL"
        };
        let footer_text = if let Some(action_id) = last_action_id.as_deref() {
            format!("{rail_label} / ACK {action_id}")
        } else if action_count > 0 {
            format!("{rail_label} / HITBOX + KEY DISPATCH")
        } else {
            format!("{rail_label} / DISPLAY ONLY")
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
        self.paint_owt_lcars_surface_focus_shell(
            layers,
            left,
            top,
            panel_width,
            panel_height,
            lcars,
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
        let Some(panel_height) = self
            .owt_lcars_bottom_panel_height_for(available_height, lcars_structural_mode(document))
        else {
            return Ok(());
        };
        let left = margin;
        let width = (self.dimensions.pixel_width as f32 - (margin * 2.0)).max(360.0);
        let bottom = self.dimensions.pixel_height as f32
            - border.bottom.get() as f32
            - bottom_bar_height
            - margin;
        let top = bottom - panel_height;

        let lcars = LcarsPalette::for_document(document);
        let panel_bg = lcars.black;
        let last_action_id = crate::owt_native::last_dispatched_action_for_interface(&document.id)
            .map(|action| action.action_id);

        self.filled_rectangle(layers, 0, rect(left, top, width, panel_height), panel_bg)?;
        self.push_owt_lcars_surface_control_hitbox(&document.id, left, top, width, panel_height);
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
            RgbColor::new_8bpc(255, 204, 112),
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
                line.hotkey,
                lcars,
            )?;
            if let Some(action_id) = &line.action_id {
                self.ui_items.push(UIItem {
                    x: action_left.max(0.0) as usize,
                    y: action_y.max(0.0) as usize,
                    width: action_width.ceil().max(1.0) as usize,
                    height: button_height.ceil().max(1.0) as usize,
                    item_type: UIItemType::OwtLcarsAction {
                        interface_id: document.id.clone(),
                        action_id: action_id.clone(),
                    },
                });
            }
            action_y += button_height + 8.0;
        }

        self.paint_owt_lcars_surface_focus_shell(layers, left, top, width, panel_height, lcars)?;

        Ok(())
    }

    fn paint_owt_lcars_action_stack(
        &mut self,
        layers: &mut TripleLayerQuadAllocator,
        action_lines: &[&PanelLine],
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        max_actions: usize,
        button_height: f32,
        button_gap: f32,
        interface_id: &str,
        last_action_id: Option<&str>,
        lcars: LcarsPalette,
    ) -> anyhow::Result<usize> {
        let cell_width = self.render_metrics.cell_size.width as f32;
        let text_cols = ((width - 22.0) / cell_width).floor().max(6.0) as usize;
        let mut painted = 0usize;
        for (index, line) in action_lines.iter().take(max_actions).enumerate() {
            let button_y = y + index as f32 * (button_height + button_gap);
            if button_y + button_height > y + height {
                break;
            }
            let active = last_action_id == line.action_id.as_deref();
            paint_lcars_action_button(
                self,
                layers,
                x,
                button_y,
                width,
                button_height,
                text_cols,
                &line.text,
                lcars_action_text_color(index),
                lcars.action_fill(index),
                lcars_action_fill_byte(index),
                active,
                line.hotkey,
                lcars,
            )?;
            if let Some(action_id) = &line.action_id {
                self.ui_items.push(UIItem {
                    x: x.max(0.0) as usize,
                    y: button_y.max(0.0) as usize,
                    width: width.ceil().max(1.0) as usize,
                    height: button_height.ceil().max(1.0) as usize,
                    item_type: UIItemType::OwtLcarsAction {
                        interface_id: interface_id.to_string(),
                        action_id: action_id.clone(),
                    },
                });
            }
            painted += 1;
        }
        Ok(painted)
    }

    fn owt_lcars_top_panel_height_for(
        &self,
        available_height: f32,
        mode: LcarsStructuralMode,
        structural: bool,
    ) -> Option<f32> {
        let cell_height = self.render_metrics.cell_size.height as f32;
        if available_height < cell_height * 7.0 {
            return None;
        }
        if structural && matches!(mode, LcarsStructuralMode::TheLcarsControlPanel) {
            return Some(
                (cell_height * LCARS_THELCARS_PANEL_ROW_HEIGHT)
                    .max(LCARS_THELCARS_PANEL_MIN_HEIGHT)
                    .min(available_height)
                    .min(LCARS_THELCARS_PANEL_MAX_HEIGHT),
            );
        }
        if structural && matches!(mode, LcarsStructuralMode::DockedSurface) {
            return Some(
                (cell_height * LCARS_DOCKED_SURFACE_PANEL_ROW_HEIGHT)
                    .max(LCARS_DOCKED_SURFACE_PANEL_MIN_HEIGHT)
                    .min(available_height)
                    .min(LCARS_DOCKED_SURFACE_PANEL_MAX_HEIGHT),
            );
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

    fn owt_lcars_side_panel_width_for(
        &self,
        available_width: f32,
        mode: LcarsStructuralMode,
    ) -> Option<f32> {
        let cell_width = self.render_metrics.cell_size.width as f32;
        if available_width < LCARS_MIN_TERMINAL_REMAINDER + (cell_width * 12.0) {
            return None;
        }
        let max_without_starving_terminal =
            (available_width - LCARS_MIN_TERMINAL_REMAINDER).max(cell_width * 12.0);
        let block_composition = matches!(mode, LcarsStructuralMode::BlockComposition);
        let thelcars = matches!(mode, LcarsStructuralMode::TheLcarsControlPanel);
        let (ratio, min_width, max_width) = if block_composition {
            (
                0.30,
                LCARS_BLOCK_SIDE_PANEL_MIN_WIDTH,
                LCARS_BLOCK_SIDE_PANEL_MAX_WIDTH,
            )
        } else if thelcars {
            (
                0.28,
                LCARS_THELCARS_SIDE_PANEL_MIN_WIDTH,
                LCARS_THELCARS_SIDE_PANEL_MAX_WIDTH,
            )
        } else {
            (0.24, LCARS_SIDE_PANEL_MIN_WIDTH, LCARS_SIDE_PANEL_MAX_WIDTH)
        };
        if max_without_starving_terminal < min_width {
            return None;
        }
        Some(
            (available_width * ratio)
                .max(min_width)
                .min(max_width)
                .min(max_without_starving_terminal),
        )
    }

    fn owt_lcars_bottom_panel_height_for(
        &self,
        available_height: f32,
        mode: LcarsStructuralMode,
    ) -> Option<f32> {
        let cell_height = self.render_metrics.cell_size.height as f32;
        if available_height < cell_height * 10.0 {
            return None;
        }
        let block_composition = matches!(mode, LcarsStructuralMode::BlockComposition);
        let thelcars = matches!(mode, LcarsStructuralMode::TheLcarsControlPanel);
        let (row_height, min_height, max_height) = if block_composition {
            (
                LCARS_BLOCK_BOTTOM_PANEL_ROW_HEIGHT,
                LCARS_BLOCK_BOTTOM_PANEL_MIN_HEIGHT,
                LCARS_BLOCK_BOTTOM_PANEL_MAX_HEIGHT,
            )
        } else if thelcars {
            (
                LCARS_THELCARS_BOTTOM_PANEL_ROW_HEIGHT,
                LCARS_THELCARS_BOTTOM_PANEL_MIN_HEIGHT,
                LCARS_THELCARS_BOTTOM_PANEL_MAX_HEIGHT,
            )
        } else {
            (
                LCARS_BOTTOM_PANEL_ROW_HEIGHT,
                LCARS_BOTTOM_PANEL_MIN_HEIGHT,
                LCARS_BOTTOM_PANEL_MAX_HEIGHT,
            )
        };
        Some(
            (cell_height * row_height)
                .max(min_height)
                .min(available_height)
                .min(max_height),
        )
    }

    pub(crate) fn paint_owt_panel_text(
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
    builder_highlight: Option<LcarsBuilderHighlight>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct LcarsBuilderHighlight {
    mode: String,
    label: String,
}

impl PanelLine {
    fn semantic(kind: PanelLineKind, text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            action_id: None,
            kind,
            hotkey: None,
            progress: None,
            builder_highlight: None,
        }
    }

    fn action(text: impl Into<String>, action_id: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            action_id: Some(action_id.into()),
            kind: PanelLineKind::Button,
            hotkey: None,
            progress: None,
            builder_highlight: None,
        }
    }

    fn with_progress(mut self, progress: Option<f32>) -> Self {
        self.progress = progress;
        self
    }
}

struct LcarsTableData {
    interface_id: String,
    title: String,
    provenance: Option<String>,
    sort_label: Option<String>,
    focus_label: Option<String>,
    group_detail: Option<String>,
    focused_detail: Option<String>,
    cell_detail: Option<String>,
    cell_focus: Option<LcarsTableCellFocus>,
    columns: Vec<String>,
    rows: Vec<LcarsTableRow>,
    group_summaries: Vec<LcarsTableGroupSummary>,
    overflow_columns: usize,
    overflow_rows: usize,
}

#[derive(Clone, Debug, PartialEq)]
struct LcarsTableCellFocus {
    column_index: usize,
    label: String,
    detail: String,
}

#[derive(Clone, Debug, PartialEq)]
struct LcarsTableGroupSummary {
    group: String,
    row_count: usize,
    action_count: usize,
    focused: bool,
    provenance_count: usize,
    severity: Option<String>,
}

#[derive(Clone)]
struct LcarsTableRow {
    cells: Vec<String>,
    severity: Option<String>,
    cell_severity: Vec<Option<String>>,
    group: Option<String>,
    provenance: Option<String>,
    cell_provenance: Vec<Option<String>>,
    action_id: Option<String>,
    focused: bool,
}

#[derive(Clone, Debug, PartialEq)]
struct LcarsTableRowActionHitbox {
    action_id: String,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
}

#[derive(Clone, Debug, PartialEq)]
struct LcarsTableCellHitbox {
    column: String,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
}

#[derive(Clone, Copy)]
struct TableSortSpec {
    column_index: usize,
    descending: bool,
}

struct LcarsPrimitiveLegendRow {
    number: usize,
    label: &'static str,
    kind: PanelLineKind,
    text: String,
    present: bool,
}

#[derive(Clone, Debug)]
struct LcarsBlockDataBox {
    label: String,
    value: String,
}

#[derive(Clone, Debug)]
struct LcarsBlockCompositionData {
    title: String,
    boxes: Vec<LcarsBlockDataBox>,
    graphics_title: String,
    graphics_detail: String,
    has_graphics: bool,
    command_bank_title: String,
}

#[derive(Clone, Debug)]
struct TheLcarsDemoMetadata {
    template_status: String,
    themes: String,
    palette_roles: String,
    frame_metrics: String,
    bar_rhythm: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TheLcarsCockpitDensity {
    Compact,
    Regular,
    Wide,
    Strip,
}

#[derive(Clone, Copy, Debug)]
struct TheLcarsCockpitGrid {
    placement: LcarsSurfacePlacement,
    density: TheLcarsCockpitDensity,
    panel: LcarsSceneRect,
    header: LcarsSceneRect,
    rail: LcarsSceneRect,
    scope_tab: LcarsSceneRect,
    theme_strip: Option<LcarsSceneRect>,
    primary_bay: LcarsSceneRect,
    secondary_bay: Option<LcarsSceneRect>,
    chart_strip: Option<LcarsSceneRect>,
    command_stack: Option<LcarsSceneRect>,
    terminal_workspace: Option<LcarsSceneRect>,
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum LcarsStructuralMode {
    Composition,
    PrimitiveLegend,
    DockedSurface,
    BlockComposition,
    TheLcarsControlPanel,
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

#[derive(Clone, Copy, Debug)]
struct LcarsSceneRect {
    x: f32,
    y: f32,
    width: f32,
    height: f32,
}

impl LcarsSceneRect {
    fn bottom(self) -> f32 {
        self.y + self.height
    }

    fn right(self) -> f32 {
        self.x + self.width
    }
}

#[derive(Clone, Copy, Debug)]
struct LcarsCommandBankScene {
    frame: LcarsSceneRect,
    label: LcarsSceneRect,
    button_top: f32,
    button_width: f32,
    button_height: f32,
    button_gap: f32,
    rows: usize,
}

#[derive(Clone, Copy, Debug)]
struct LcarsStructuralScene {
    mode: LcarsStructuralMode,
    plan: LcarsStructuralPlan,
    signal_field: LcarsSceneRect,
    scope_tab: LcarsSceneRect,
    content_bay: LcarsSceneRect,
    content_label: LcarsSceneRect,
    status_tab: LcarsSceneRect,
    command_bank: Option<LcarsCommandBankScene>,
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
        let title_and_header = cell_height * 3.65;
        let row_height = (cell_height * 1.46).clamp(23.0, 31.0) + 6.0;
        ((self.detail_height - title_and_header) / row_height)
            .floor()
            .max(2.0) as usize
    }
}

fn compute_thelcars_cockpit_grid(
    panel: LcarsSceneRect,
    placement: LcarsSurfacePlacement,
    cell_width: f32,
    cell_height: f32,
    action_count: usize,
    signal_count: usize,
) -> Option<TheLcarsCockpitGrid> {
    if panel.width < cell_width * 24.0 || panel.height < cell_height * 7.0 {
        return None;
    }

    match placement.layout {
        LcarsPanelLayout::Left | LcarsPanelLayout::Right => compute_thelcars_side_cockpit_grid(
            panel,
            placement,
            cell_width,
            cell_height,
            action_count,
        ),
        LcarsPanelLayout::Bottom => compute_thelcars_bottom_cockpit_grid(
            panel,
            placement,
            cell_width,
            cell_height,
            action_count,
        ),
        LcarsPanelLayout::Top | LcarsPanelLayout::Overlay => compute_thelcars_top_cockpit_grid(
            panel,
            placement,
            cell_width,
            cell_height,
            action_count,
            signal_count,
        ),
    }
}

fn compute_thelcars_top_cockpit_grid(
    panel: LcarsSceneRect,
    placement: LcarsSurfacePlacement,
    cell_width: f32,
    cell_height: f32,
    action_count: usize,
    signal_count: usize,
) -> Option<TheLcarsCockpitGrid> {
    let density = if panel.width >= 1320.0 && panel.height >= 430.0 {
        TheLcarsCockpitDensity::Wide
    } else if panel.width < 860.0 || panel.height < 360.0 {
        TheLcarsCockpitDensity::Compact
    } else {
        TheLcarsCockpitDensity::Regular
    };
    let header_height = (panel.height * 0.15)
        .clamp(58.0, 76.0)
        .min(panel.height * 0.24);
    let header = LcarsSceneRect {
        x: panel.x + 10.0,
        y: panel.y + 8.0,
        width: (panel.width - 20.0).max(cell_width * 24.0),
        height: header_height,
    };
    let rail_width = (panel.width * 0.12)
        .clamp(142.0, 178.0)
        .min(panel.width * 0.30);
    let body_top = header.bottom() + 16.0;
    let body_bottom = panel.bottom() - 28.0;
    if body_bottom - body_top < cell_height * 7.0 {
        return None;
    }
    let rail = LcarsSceneRect {
        x: panel.x + 10.0,
        y: body_top,
        width: rail_width,
        height: (body_bottom - body_top).max(cell_height * 7.0),
    };
    let command_visible =
        action_count > 0 && panel.width >= cell_width * 82.0 && body_bottom - body_top >= 220.0;
    let command_width = if command_visible {
        (panel.width * 0.23)
            .clamp(270.0, 380.0)
            .min(panel.width * 0.31)
    } else {
        0.0
    };
    let command_left = if command_visible {
        (panel.right() - command_width - 12.0).max(panel.x + panel.width * 0.62)
    } else {
        panel.right() - 12.0
    };
    let center_left = rail.right() + 22.0;
    let center_right = if command_visible {
        command_left - 18.0
    } else {
        panel.right() - 18.0
    };
    let center_width = (center_right - center_left).max(cell_width * 28.0);
    if center_width < cell_width * 18.0 {
        return None;
    }

    let scope_height = (cell_height + 9.0).clamp(25.0, 34.0);
    let scope_tab = LcarsSceneRect {
        x: center_left,
        y: body_top,
        width: center_width.min(cell_width * 70.0),
        height: scope_height,
    };
    let theme_height = (cell_height + 9.0).clamp(24.0, 32.0);
    let theme_strip = LcarsSceneRect {
        x: center_left,
        y: scope_tab.bottom() + 8.0,
        width: center_width,
        height: theme_height,
    };
    let bay_top = theme_strip.bottom() + 18.0;
    let chart_height = if matches!(density, TheLcarsCockpitDensity::Compact) {
        0.0
    } else {
        (cell_height * 4.2).clamp(76.0, 104.0)
    };
    let chart_gap = if chart_height > 0.0 { 20.0 } else { 0.0 };
    let available_bay_height = body_bottom - bay_top - chart_height - chart_gap;
    let bay_height = available_bay_height
        .clamp(cell_height * 6.0, 236.0)
        .max(cell_height * 4.0);
    let bay_gap = 18.0;
    let secondary_visible = signal_count > 4 && center_width >= 600.0;
    let left_bay_width = if secondary_visible {
        (center_width * 0.52).clamp(300.0, (center_width - bay_gap - 230.0).max(300.0))
    } else {
        center_width
    };
    let right_bay_width = center_width - left_bay_width - bay_gap;
    let secondary_bay = (secondary_visible && right_bay_width >= 210.0).then_some(LcarsSceneRect {
        x: center_left + left_bay_width + bay_gap,
        y: bay_top,
        width: right_bay_width,
        height: bay_height,
    });
    let primary_bay = LcarsSceneRect {
        x: center_left,
        y: bay_top,
        width: if secondary_bay.is_some() {
            left_bay_width
        } else {
            center_width
        },
        height: bay_height,
    };
    let chart_top = primary_bay.bottom() + 16.0;
    let chart_strip = (chart_height > 0.0 && chart_top + chart_height <= body_bottom + 2.0)
        .then_some(LcarsSceneRect {
            x: center_left,
            y: chart_top,
            width: center_width,
            height: chart_height,
        });
    let command_stack = command_visible.then_some(LcarsSceneRect {
        x: command_left,
        y: body_top,
        width: command_width,
        height: (body_bottom - body_top).max(220.0),
    });
    let workspace_top = chart_strip
        .map(LcarsSceneRect::bottom)
        .unwrap_or_else(|| primary_bay.bottom())
        + 12.0;
    let terminal_workspace =
        (workspace_top + cell_height * 2.0 < body_bottom).then_some(LcarsSceneRect {
            x: center_left,
            y: workspace_top,
            width: center_width,
            height: body_bottom - workspace_top,
        });

    Some(TheLcarsCockpitGrid {
        placement,
        density,
        panel,
        header,
        rail,
        scope_tab,
        theme_strip: Some(theme_strip),
        primary_bay,
        secondary_bay,
        chart_strip,
        command_stack,
        terminal_workspace,
    })
}

fn compute_thelcars_side_cockpit_grid(
    panel: LcarsSceneRect,
    placement: LcarsSurfacePlacement,
    cell_width: f32,
    cell_height: f32,
    action_count: usize,
) -> Option<TheLcarsCockpitGrid> {
    let density = if panel.width < 410.0 {
        TheLcarsCockpitDensity::Compact
    } else {
        TheLcarsCockpitDensity::Regular
    };
    let header_height = (cell_height + 18.0).clamp(34.0, 46.0);
    let header = LcarsSceneRect {
        x: panel.x + 10.0,
        y: panel.y + 10.0,
        width: (panel.width - 20.0).max(cell_width * 18.0),
        height: header_height,
    };
    let rail_width = (panel.width * 0.20).clamp(58.0, 84.0);
    let rail_x = if placement.layout == LcarsPanelLayout::Left {
        panel.x + 8.0
    } else {
        panel.right() - rail_width - 8.0
    };
    let content_left = if placement.layout == LcarsPanelLayout::Left {
        rail_x + rail_width + 12.0
    } else {
        panel.x + 12.0
    };
    let content_right = if placement.layout == LcarsPanelLayout::Left {
        panel.right() - 12.0
    } else {
        rail_x - 12.0
    };
    let content_width = (content_right - content_left).max(cell_width * 16.0);
    if content_width < cell_width * 14.0 {
        return None;
    }
    let body_top = header.bottom() + 12.0;
    let body_bottom = panel.bottom() - 16.0;
    if body_bottom - body_top < cell_height * 7.0 {
        return None;
    }
    let rail = LcarsSceneRect {
        x: rail_x,
        y: body_top,
        width: rail_width,
        height: body_bottom - body_top,
    };
    let scope_height = (cell_height + 8.0).clamp(24.0, 32.0);
    let scope_tab = LcarsSceneRect {
        x: content_left,
        y: body_top,
        width: content_width,
        height: scope_height,
    };
    let theme_strip =
        if content_width >= cell_width * 22.0 && body_bottom - scope_tab.bottom() > 180.0 {
            Some(LcarsSceneRect {
                x: content_left,
                y: scope_tab.bottom() + 8.0,
                width: content_width,
                height: (cell_height + 7.0).clamp(23.0, 30.0),
            })
        } else {
            None
        };
    let command_height =
        if action_count > 0 && content_width >= 220.0 && body_bottom - body_top > 250.0 {
            (cell_height * 8.4).clamp(132.0, 190.0)
        } else {
            0.0
        };
    let command_stack = (command_height > 0.0).then_some(LcarsSceneRect {
        x: content_left,
        y: body_bottom - command_height,
        width: content_width,
        height: command_height,
    });
    let primary_top = theme_strip
        .map(LcarsSceneRect::bottom)
        .unwrap_or_else(|| scope_tab.bottom())
        + 12.0;
    let primary_bottom = command_stack
        .map(|rect| rect.y - 12.0)
        .unwrap_or(body_bottom);
    let primary_bay = LcarsSceneRect {
        x: content_left,
        y: primary_top,
        width: content_width,
        height: (primary_bottom - primary_top).max(cell_height * 4.0),
    };

    Some(TheLcarsCockpitGrid {
        placement,
        density,
        panel,
        header,
        rail,
        scope_tab,
        theme_strip,
        primary_bay,
        secondary_bay: None,
        chart_strip: None,
        command_stack,
        terminal_workspace: None,
    })
}

fn compute_thelcars_bottom_cockpit_grid(
    panel: LcarsSceneRect,
    placement: LcarsSurfacePlacement,
    cell_width: f32,
    cell_height: f32,
    action_count: usize,
) -> Option<TheLcarsCockpitGrid> {
    let density = TheLcarsCockpitDensity::Strip;
    let header_height = (cell_height + 16.0).clamp(32.0, 44.0);
    let header = LcarsSceneRect {
        x: panel.x + 10.0,
        y: panel.y + 8.0,
        width: (panel.width - 20.0).max(cell_width * 24.0),
        height: header_height,
    };
    let rail_width = (panel.width * 0.10).clamp(96.0, 146.0);
    let body_top = header.bottom() + 10.0;
    let body_bottom = panel.bottom() - 14.0;
    if body_bottom - body_top < cell_height * 4.0 {
        return None;
    }
    let rail = LcarsSceneRect {
        x: panel.x + 10.0,
        y: body_top,
        width: rail_width,
        height: body_bottom - body_top,
    };
    let command_visible = action_count > 0 && panel.width >= cell_width * 78.0;
    let command_width = if command_visible {
        (panel.width * 0.24).clamp(250.0, 360.0)
    } else {
        0.0
    };
    let command_stack = command_visible.then_some(LcarsSceneRect {
        x: panel.right() - command_width - 12.0,
        y: body_top,
        width: command_width,
        height: body_bottom - body_top,
    });
    let content_left = rail.right() + 14.0;
    let content_right = command_stack
        .map(|rect| rect.x - 16.0)
        .unwrap_or(panel.right() - 12.0);
    let content_width = (content_right - content_left).max(cell_width * 18.0);
    if content_width < cell_width * 14.0 {
        return None;
    }
    let scope_height = (cell_height + 8.0).clamp(24.0, 30.0);
    let scope_tab = LcarsSceneRect {
        x: content_left,
        y: body_top,
        width: content_width.min(cell_width * 54.0),
        height: scope_height,
    };
    let primary_bay = LcarsSceneRect {
        x: content_left,
        y: scope_tab.bottom() + 10.0,
        width: content_width,
        height: (body_bottom - scope_tab.bottom() - 10.0).max(cell_height * 3.0),
    };

    Some(TheLcarsCockpitGrid {
        placement,
        density,
        panel,
        header,
        rail,
        scope_tab,
        theme_strip: None,
        primary_bay,
        secondary_bay: None,
        chart_strip: None,
        command_stack,
        terminal_workspace: None,
    })
}

fn compute_lcars_structural_scene(
    content_left: f32,
    content_right: f32,
    panel_top: f32,
    panel_bottom: f32,
    window_bottom: f32,
    cell_width: f32,
    cell_height: f32,
    has_table: bool,
    has_data_cascade: bool,
    action_count: usize,
    mode: LcarsStructuralMode,
) -> LcarsStructuralScene {
    let content_width = (content_right - content_left).max(cell_width * 34.0);
    let primitive_legend = matches!(mode, LcarsStructuralMode::PrimitiveLegend);
    let sleek = matches!(mode, LcarsStructuralMode::DockedSurface);
    let signal_top = panel_top
        + if primitive_legend || sleek {
            82.0
        } else {
            90.0
        };
    let content_bay_y = panel_bottom
        - if primitive_legend {
            42.0
        } else if sleek {
            48.0
        } else {
            52.0
        };
    let signal_limit = content_bay_y - 8.0;
    let plan = plan_lcars_structural_console(
        content_left,
        content_right,
        signal_top,
        signal_limit,
        cell_width,
        cell_height,
        has_table,
        has_data_cascade,
        action_count,
        mode,
    );
    let protected_scope_width = if sleek && plan.command_visible {
        clamp_ordered(
            plan.command_left - content_left - 28.0,
            cell_width * 24.0,
            content_width * 0.62,
        )
    } else {
        clamp_ordered(
            plan.signal_width * 0.96,
            cell_width * 30.0,
            content_width * 0.56,
        )
    };
    let label_height = if sleek {
        (cell_height + 14.0).clamp(30.0, 40.0)
    } else {
        (cell_height + 10.0).clamp(24.0, 32.0)
    };
    let bay_tab_height = if sleek {
        (cell_height + 10.0).clamp(28.0, 36.0)
    } else {
        (cell_height + 6.0).clamp(22.0, 30.0)
    };
    let content_label_width =
        clamp_ordered(content_width * 0.38, cell_width * 13.0, cell_width * 22.0);
    let status_tab_width = if sleek {
        (cell_width * 13.0)
            .max(122.0)
            .min((content_width * 0.24).max(122.0))
    } else {
        (cell_width * 18.0)
            .max(138.0)
            .min((content_width * 0.30).max(138.0))
    };
    let status_tab_x = if sleek && plan.command_visible {
        clamp_ordered(
            plan.command_left + plan.command_width - status_tab_width,
            content_left + 18.0,
            content_right - status_tab_width - 18.0,
        )
    } else {
        (content_right - status_tab_width - 18.0).max(content_left + 18.0)
    };
    let content_bay_height = if sleek {
        (window_bottom - content_bay_y - 14.0).clamp(170.0, 560.0)
    } else {
        (window_bottom - content_bay_y - 8.0).clamp(92.0, 220.0)
    };
    let content_bay_width = if sleek && plan.command_visible {
        let command_margin = if matches!(
            lcars_structural_breakpoint(content_width, cell_width),
            LcarsStructuralBreakpoint::Compact
        ) {
            18.0
        } else {
            24.0
        };
        (plan.command_left - content_left - command_margin)
            .max(content_width * 0.46)
            .min(content_width)
    } else {
        content_width
    };

    let button_height = if sleek {
        (cell_height * 2.24).clamp(42.0, 52.0)
    } else {
        (cell_height * 2.15).clamp(36.0, 46.0)
    };
    let button_gap = 12.0;
    let columns = if plan.two_action_columns { 2 } else { 1 };
    let button_width = if plan.two_action_columns {
        ((plan.command_width - button_gap) * 0.5).max(132.0)
    } else {
        plan.command_width
    };
    let visible_actions = action_count.min(plan.action_slots);
    let rows = if plan.command_visible {
        (visible_actions + columns - 1) / columns
    } else {
        0
    };
    let command_bank = plan.command_visible.then(|| {
        let frame_x = plan.command_left - 16.0;
        let frame_y = signal_top + 2.0;
        let frame_height = (14.0
            + (rows.max(1) as f32 * button_height)
            + (rows.saturating_sub(1) as f32 * button_gap)
            + 12.0)
            .min((content_bay_y - frame_y - 14.0).max(button_height + 16.0));
        let label_width = plan
            .command_width
            .min(cell_width * 24.0)
            .max(cell_width * 16.0);
        LcarsCommandBankScene {
            frame: LcarsSceneRect {
                x: frame_x,
                y: frame_y,
                width: (plan.command_width + 18.0).max(1.0),
                height: frame_height,
            },
            label: LcarsSceneRect {
                x: plan.command_left,
                y: panel_top + 58.0,
                width: label_width,
                height: label_height,
            },
            button_top: frame_y + 9.0,
            button_width,
            button_height,
            button_gap,
            rows,
        }
    });

    LcarsStructuralScene {
        mode,
        plan,
        signal_field: LcarsSceneRect {
            x: content_left,
            y: signal_top,
            width: content_width,
            height: (signal_limit - signal_top).max(cell_height * 2.0),
        },
        scope_tab: LcarsSceneRect {
            x: content_left,
            y: panel_top + 58.0,
            width: protected_scope_width,
            height: label_height,
        },
        content_bay: LcarsSceneRect {
            x: content_left,
            y: content_bay_y,
            width: content_bay_width,
            height: content_bay_height,
        },
        content_label: LcarsSceneRect {
            x: content_left + 18.0,
            y: content_bay_y + 7.0,
            width: content_label_width,
            height: bay_tab_height,
        },
        status_tab: LcarsSceneRect {
            x: status_tab_x,
            y: content_bay_y + 7.0,
            width: status_tab_width,
            height: bay_tab_height,
        },
        command_bank,
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

fn is_structural_chrome_signal(kind: PanelLineKind) -> bool {
    matches!(
        kind,
        PanelLineKind::Frame
            | PanelLineKind::Elbow
            | PanelLineKind::SideRail
            | PanelLineKind::ContentBay
            | PanelLineKind::Bar
            | PanelLineKind::BarRun
            | PanelLineKind::CommandGrid
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
    mode: LcarsStructuralMode,
) -> LcarsStructuralPlan {
    let content_width = (content_right - content_left).max(cell_width * 34.0);
    let breakpoint = lcars_structural_breakpoint(content_width, cell_width);
    let primitive_legend = matches!(mode, LcarsStructuralMode::PrimitiveLegend);
    let sleek = matches!(mode, LcarsStructuralMode::DockedSurface);
    let semantic_blocks = matches!(mode, LcarsStructuralMode::BlockComposition);
    let command_visible = action_count > 0;
    let command_width = if command_visible {
        let (min_cells, min_px, max_px, width_ratio, max_ratio): (f32, f32, f32, f32, f32) =
            if primitive_legend {
                (16.0, 170.0, 260.0, 0.15, 0.18)
            } else if sleek {
                match breakpoint {
                    LcarsStructuralBreakpoint::Compact => (28.0, 285.0, 315.0, 0.42, 0.55),
                    LcarsStructuralBreakpoint::Regular => (30.0, 320.0, 370.0, 0.28, 0.36),
                    LcarsStructuralBreakpoint::Wide => (32.0, 340.0, 395.0, 0.22, 0.28),
                }
            } else {
                match breakpoint {
                    LcarsStructuralBreakpoint::Compact => (20.0, 210.0, 270.0, 0.30, 0.34),
                    LcarsStructuralBreakpoint::Regular => (26.0, 260.0, 340.0, 0.22, 0.28),
                    LcarsStructuralBreakpoint::Wide => (28.0, 290.0, 380.0, 0.18, 0.24),
                }
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
    let command_left = if sleek && command_visible {
        let right_dock = content_right - command_width;
        let inset = match breakpoint {
            LcarsStructuralBreakpoint::Compact => 0.0,
            LcarsStructuralBreakpoint::Regular => content_width * 0.035,
            LcarsStructuralBreakpoint::Wide => content_width * 0.055,
        };
        clamp_ordered(
            right_dock - inset,
            content_left + cell_width * 18.0,
            right_dock,
        )
    } else {
        content_right - command_width
    };
    let pre_command_width = if command_visible {
        (command_left - content_left - 18.0).max(cell_width * 16.0)
    } else {
        (content_right - content_left).max(cell_width * 16.0)
    };
    let wants_detail =
        !primitive_legend && !sleek && !semantic_blocks && (has_table || has_data_cascade);
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
                LcarsStructuralBreakpoint::Regular => 540.0,
                LcarsStructuralBreakpoint::Wide => 620.0,
            };
            let detail_ratio = match (breakpoint, has_table) {
                (LcarsStructuralBreakpoint::Compact, true) => 0.50,
                (LcarsStructuralBreakpoint::Compact, false) => 0.46,
                (LcarsStructuralBreakpoint::Regular, true) => 0.42,
                (LcarsStructuralBreakpoint::Regular, false) => 0.46,
                (LcarsStructuralBreakpoint::Wide, true) => 0.38,
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
                LcarsStructuralBreakpoint::Compact if primitive_legend => 12,
                LcarsStructuralBreakpoint::Regular if primitive_legend => 14,
                LcarsStructuralBreakpoint::Wide if primitive_legend => 16,
                LcarsStructuralBreakpoint::Compact if sleek => 2,
                LcarsStructuralBreakpoint::Regular if sleek => 3,
                LcarsStructuralBreakpoint::Wide if sleek => 3,
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

    let button_height = if sleek {
        (cell_height * 2.24).clamp(42.0, 52.0)
    } else {
        (cell_height * 2.15).clamp(36.0, 46.0)
    };
    let button_gap = 12.0;
    let action_rows = ((signal_limit - (signal_top + 2.0)) / (button_height + button_gap))
        .floor()
        .max(0.0) as usize;
    let two_action_columns = false;
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
        if let Some(line) = items
            .iter_mut()
            .find(|line| line.action_id.as_deref() == Some(action.id.as_str()))
        {
            line.text = action.label.clone();
        } else if !items
            .iter()
            .any(|line| line.action_id.as_deref() == Some(action.id.as_str()))
        {
            items.push(PanelLine::action(action.label.clone(), action.id.clone()));
        }
    }
    items
}

fn prioritized_side_panel_signal_lines(items: &[PanelLine], limit: usize) -> Vec<&PanelLine> {
    let mut selected = items
        .iter()
        .enumerate()
        .filter(|(_, line)| line.action_id.is_none())
        .collect::<Vec<_>>();
    selected.sort_by_key(|(index, line)| (side_panel_line_priority(line.kind), *index));
    selected.truncate(limit);
    selected.sort_by_key(|(index, _)| *index);
    selected.into_iter().map(|(_, line)| line).collect()
}

fn side_panel_line_priority(kind: PanelLineKind) -> u8 {
    match kind {
        PanelLineKind::Metric => 0,
        PanelLineKind::Status => 1,
        PanelLineKind::Table => 2,
        PanelLineKind::DataCascade => 3,
        PanelLineKind::Progress => 4,
        PanelLineKind::Badge | PanelLineKind::List | PanelLineKind::Text => 5,
        PanelLineKind::ContentBay | PanelLineKind::Section => 6,
        PanelLineKind::CommandGrid => 7,
        PanelLineKind::Frame
        | PanelLineKind::SideRail
        | PanelLineKind::Bar
        | PanelLineKind::BarRun
        | PanelLineKind::Elbow
        | PanelLineKind::Image
        | PanelLineKind::Button => 8,
    }
}

fn assign_lcars_hotkeys(items: &mut [PanelLine]) {
    let mut slot = 1usize;
    for line in items.iter_mut().filter(|line| line.action_id.is_some()) {
        if slot > LCARS_KEY_ACTION_LIMIT {
            break;
        }
        line.hotkey = Some(slot);
        slot += 1;
    }
}

fn lcars_keyboard_action_slots(document: &InterfaceDocument) -> Vec<String> {
    lcars_keyboard_action_slots_for_page(document, 0)
}

fn lcars_keyboard_action_slots_for_page(document: &InterfaceDocument, page: usize) -> Vec<String> {
    collect_panel_items(document)
        .into_iter()
        .filter_map(|line| line.action_id)
        .skip(page.saturating_mul(LCARS_KEY_ACTION_LIMIT))
        .take(LCARS_KEY_ACTION_LIMIT)
        .collect()
}

fn lcars_action_page_count(document: &InterfaceDocument) -> usize {
    let action_count = collect_panel_items(document)
        .into_iter()
        .filter(|line| line.action_id.is_some())
        .count();
    action_count.max(1).div_ceil(LCARS_KEY_ACTION_LIMIT)
}

fn lcars_action_strip_mode(document: &InterfaceDocument) -> bool {
    document_property_value(
        document,
        &[
            "profile",
            "layout_profile",
            "information_shape",
            "surface_kind",
            "widget_kind",
            "shape",
        ],
    )
    .map(|value| {
        let normalized = value.trim().to_ascii_lowercase().replace(['-', ' '], "_");
        matches!(
            normalized.as_str(),
            "action_strip"
                | "button_strip"
                | "simple_buttons"
                | "folder_buttons"
                | "folder_opener"
                | "folder_open_strip"
        )
    })
    .unwrap_or(false)
}

fn lcars_action_strip_vertical_rail(placement: LcarsSurfacePlacement) -> bool {
    matches!(
        placement.layout,
        LcarsPanelLayout::Left | LcarsPanelLayout::Right
    )
}

fn lcars_corner_button_mode(document: &InterfaceDocument) -> bool {
    document_property_value(
        document,
        &[
            "profile",
            "layout_profile",
            "information_shape",
            "surface_kind",
            "widget_kind",
            "shape",
        ],
    )
    .map(|value| {
        let normalized = value.trim().to_ascii_lowercase().replace(['-', ' '], "_");
        matches!(
            normalized.as_str(),
            "corner_button"
                | "cycle_render_shortcut"
                | "floating_button"
                | "single_button"
                | "shortcut_button"
                | "action_button"
        )
    })
    .unwrap_or(false)
}

fn lcars_explicit_terminal_overlay_allowed(document: &InterfaceDocument) -> bool {
    document_property_value(
        document,
        &[
            "allow_terminal_overlay",
            "allow_prompt_overlay",
            "terminal_overlay",
            "transient_overlay",
            "hud_overlay",
        ],
    )
    .and_then(parse_lcars_bool)
    .unwrap_or(false)
}

fn lcars_surface_visible(document: &InterfaceDocument) -> bool {
    if let Some(hidden) = document_property_value(document, &["hidden"]).and_then(parse_lcars_bool)
    {
        if hidden {
            return false;
        }
    }
    if let Some(display) = document_property_value(document, &["display"]) {
        let normalized = display.trim().to_ascii_lowercase().replace(['-', ' '], "_");
        if matches!(
            normalized.as_str(),
            "none" | "hidden" | "hide" | "off" | "disabled" | "inactive"
        ) {
            return false;
        }
    }
    for key in ["visible", "active", "enabled"] {
        if let Some(false) = document_property_value(document, &[key]).and_then(parse_lcars_bool) {
            return false;
        }
    }
    true
}

fn lcars_surface_placement(document: &InterfaceDocument) -> LcarsSurfacePlacement {
    let mut placement = document_property_value(document, &["layout", "mode", "profile"])
        .and_then(parse_lcars_panel_layout)
        .map(LcarsSurfacePlacement::from_layout)
        .unwrap_or_else(|| LcarsSurfacePlacement::from_layout(LcarsPanelLayout::Top));

    if let Some(origin) = document_property_value(
        document,
        &[
            "placement",
            "dock",
            "docking",
            "origin",
            "anchor",
            "panel_origin",
            "surface_origin",
        ],
    )
    .and_then(parse_lcars_surface_origin)
    {
        placement = LcarsSurfacePlacement::from_origin(origin);
    }

    if let Some(reservation) =
        document_property_value(document, &["reservation", "reserve", "terminal_space"])
            .and_then(parse_lcars_surface_reservation)
    {
        placement.reservation = reservation;
    }

    if placement.layout == LcarsPanelLayout::Overlay {
        placement.origin = LcarsSurfaceOrigin::Overlay;
        placement.reservation = LcarsSurfaceReservation::Overlay;
    }

    if let Some(orientation) = document_property_value(document, &["orientation", "axis", "flow"])
        .and_then(parse_lcars_surface_orientation)
    {
        placement.orientation = orientation;
    }

    if lcars_corner_button_mode(document)
        && !placement.reserves_terminal_space()
        && !lcars_explicit_terminal_overlay_allowed(document)
    {
        if placement.layout == LcarsPanelLayout::Overlay {
            placement.layout = LcarsPanelLayout::Top;
            placement.origin = LcarsSurfaceOrigin::TopLeft;
            placement.orientation = LcarsSurfaceOrientation::Horizontal;
        }
        placement.reservation = LcarsSurfaceReservation::Reserved;
    }

    placement
}

pub(crate) fn describe_lcars_render_projection(
    document: &InterfaceDocument,
) -> LcarsRenderProjection {
    let visible = lcars_surface_visible(document);
    let placement = lcars_surface_placement(document);
    let corner_button = visible && lcars_corner_button_mode(document);
    let action_strip = visible && lcars_action_strip_mode(document);
    let structural = visible && has_structural_lcars_layout(document);
    let structural_mode = structural.then(|| lcars_structural_mode(document));
    let palette_profile = lcars_palette_profile(document);
    let expose_floating_geometry = placement.layout == LcarsPanelLayout::Overlay
        && placement.reservation == LcarsSurfaceReservation::Overlay;

    LcarsRenderProjection {
        visible,
        renderer_path: if !visible {
            "hidden"
        } else if corner_button {
            "corner_button"
        } else if action_strip {
            "action_strip"
        } else if structural {
            "structural_lcars"
        } else {
            "compact_lcars"
        },
        layout: lcars_panel_layout_name(placement.layout),
        origin: lcars_surface_origin_name(placement.origin),
        reservation: lcars_surface_reservation_name(placement.reservation),
        orientation: lcars_surface_orientation_name(placement.orientation),
        reserves_terminal_space: placement.reserves_terminal_space(),
        anchor: lcars_optional_document_property(
            document,
            &[
                "anchor",
                "placement_anchor",
                "sticky",
                "surface_anchor",
                "panel_anchor",
            ],
        ),
        z_order: lcars_optional_i32_document_property(document, &["z_order", "z", "layer"]),
        priority: lcars_optional_i32_document_property(
            document,
            &["priority", "layout_priority", "surface_priority"],
        ),
        min_terminal_cells: lcars_optional_document_property(
            document,
            &[
                "min_terminal_cells",
                "minimum_terminal_cells",
                "min_grid_cells",
            ],
        ),
        collapse_policy: lcars_optional_document_property(
            document,
            &["collapse_policy", "collapse", "overflow_policy"],
        ),
        floating_anchor: expose_floating_geometry
            .then(|| {
                lcars_optional_document_property(
                    document,
                    &["floating_anchor", "float_anchor", "widget_anchor"],
                )
            })
            .flatten(),
        floating_x: expose_floating_geometry
            .then(|| {
                lcars_optional_document_property(
                    document,
                    &["floating_x", "float_x", "widget_x", "surface_x"],
                )
            })
            .flatten(),
        floating_y: expose_floating_geometry
            .then(|| {
                lcars_optional_document_property(
                    document,
                    &["floating_y", "float_y", "widget_y", "surface_y"],
                )
            })
            .flatten(),
        floating_width: expose_floating_geometry
            .then(|| {
                lcars_optional_document_property(
                    document,
                    &[
                        "floating_width",
                        "float_width",
                        "widget_width",
                        "surface_width",
                    ],
                )
            })
            .flatten(),
        floating_height: expose_floating_geometry
            .then(|| {
                lcars_optional_document_property(
                    document,
                    &[
                        "floating_height",
                        "float_height",
                        "widget_height",
                        "surface_height",
                    ],
                )
            })
            .flatten(),
        structural_profile: structural_mode.map(lcars_structural_mode_name),
        requested_profile: lcars_requested_profile_hint(document),
        profile_family: lcars_optional_document_property(
            document,
            &["profile_family", "shape_family", "information_family"],
        ),
        palette_profile: (palette_profile != LcarsPaletteProfile::Classic)
            .then_some(lcars_palette_profile_name(palette_profile)),
        table_density: lcars_optional_document_property(
            document,
            &["table_density", "density", "matrix_density"],
        ),
        cohort_key: lcars_optional_document_property(
            document,
            &["cohort_key", "group_key", "group_by"],
        ),
        state_key: lcars_optional_document_property(document, &["state_key", "status_key"]),
        severity_key: lcars_optional_document_property(
            document,
            &["severity_key", "priority_key", "risk_key"],
        ),
        lifecycle_controls: lcars_optional_document_property(
            document,
            &["lifecycle_controls", "lifecycle_actions"],
        ),
        action_roles: lcars_optional_document_property(
            document,
            &["action_roles", "button_roles", "command_roles"],
        ),
        refresh_policy: lcars_optional_document_property(
            document,
            &["refresh_policy", "refresh_mode"],
        ),
        drilldown_policy: lcars_optional_document_property(
            document,
            &["drilldown_policy", "focus_policy"],
        ),
    }
}

fn lcars_optional_document_property(document: &InterfaceDocument, keys: &[&str]) -> Option<String> {
    document_property_value(document, keys)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
}

fn lcars_optional_i32_document_property(
    document: &InterfaceDocument,
    keys: &[&str],
) -> Option<i32> {
    document_property_value(document, keys)
        .map(str::trim)
        .and_then(|value| value.parse::<i32>().ok())
}

fn lcars_panel_layout_name(layout: LcarsPanelLayout) -> &'static str {
    match layout {
        LcarsPanelLayout::Top => "top",
        LcarsPanelLayout::Left => "left_rail",
        LcarsPanelLayout::Right => "right_rail",
        LcarsPanelLayout::Bottom => "bottom_strip",
        LcarsPanelLayout::Overlay => "overlay",
    }
}

fn lcars_surface_origin_name(origin: LcarsSurfaceOrigin) -> &'static str {
    match origin {
        LcarsSurfaceOrigin::TopLeft => "top_left",
        LcarsSurfaceOrigin::Top => "top",
        LcarsSurfaceOrigin::Left => "left",
        LcarsSurfaceOrigin::Right => "right",
        LcarsSurfaceOrigin::Bottom => "bottom",
        LcarsSurfaceOrigin::BottomRight => "bottom_right",
        LcarsSurfaceOrigin::Overlay => "overlay",
    }
}

fn lcars_surface_reservation_name(reservation: LcarsSurfaceReservation) -> &'static str {
    match reservation {
        LcarsSurfaceReservation::Reserved => "reserved",
        LcarsSurfaceReservation::Overlay => "overlay",
    }
}

fn lcars_surface_orientation_name(orientation: LcarsSurfaceOrientation) -> &'static str {
    match orientation {
        LcarsSurfaceOrientation::Horizontal => "horizontal",
        LcarsSurfaceOrientation::Vertical => "vertical",
        LcarsSurfaceOrientation::Auto => "auto",
    }
}

fn lcars_render_scene_summary(documents: &[InterfaceDocument]) -> LcarsRenderSceneSummary {
    let mut summary = LcarsRenderSceneSummary::default();
    let mut slots: BTreeMap<&'static str, LcarsRenderSceneSlotSummary> = BTreeMap::new();

    for document in documents
        .iter()
        .filter(|document| lcars_surface_visible(document))
    {
        let placement = lcars_surface_placement(document);
        let slot = lcars_panel_layout_name(placement.layout);
        let reserved = placement.reserves_terminal_space();
        summary.active_count += 1;
        if reserved {
            summary.reserved_count += 1;
        } else {
            summary.overlay_count += 1;
        }

        let entry = slots
            .entry(slot)
            .or_insert_with(|| LcarsRenderSceneSlotSummary {
                slot,
                ..LcarsRenderSceneSlotSummary::default()
            });
        entry.active_count += 1;
        if reserved {
            entry.reserved_count += 1;
        }
        entry.interface_ids.push(document.id.clone());
    }

    summary.slots = slots.into_values().collect();
    summary
        .slots
        .sort_by_key(|slot| lcars_render_scene_slot_rank(slot.slot));
    for slot in &summary.slots {
        if slot.reserved_count > 1 {
            summary.conflict_hints.push(format!(
                "{} has {} reserved surfaces; renderer uses max-edge reservation",
                slot.slot, slot.reserved_count
            ));
        }
    }
    summary
}

fn lcars_render_scene_badge_lines(summary: &LcarsRenderSceneSummary) -> Option<(String, String)> {
    if summary.active_count < 2 && summary.conflict_hints.is_empty() {
        return None;
    }

    let title = if summary.conflict_hints.is_empty() {
        "SCENE PLAN"
    } else {
        "SCENE CONFLICT"
    };
    let mut parts = vec![
        format!("{}A", summary.active_count),
        format!("{}R", summary.reserved_count),
    ];
    if summary.overlay_count > 0 {
        parts.push(format!("{}O", summary.overlay_count));
    }
    let slot_parts = summary
        .slots
        .iter()
        .map(lcars_render_scene_slot_label)
        .collect::<Vec<_>>();
    if !slot_parts.is_empty() {
        parts.push(slot_parts.join(" "));
    }
    if let Some(conflict_slot) = summary.slots.iter().find(|slot| slot.reserved_count > 1) {
        parts.push(format!(
            "{}{} CONFLICT",
            lcars_render_scene_slot_abbrev(conflict_slot.slot),
            conflict_slot.reserved_count
        ));
    }
    Some((title.to_string(), parts.join(" / ")))
}

fn lcars_render_scene_slot_label(slot: &LcarsRenderSceneSlotSummary) -> String {
    let abbrev = lcars_render_scene_slot_abbrev(slot.slot);
    if slot.reserved_count > 0 {
        format!("{abbrev}{}R", slot.reserved_count)
    } else {
        format!("{abbrev}{}O", slot.active_count)
    }
}

fn lcars_render_scene_slot_abbrev(slot: &str) -> &'static str {
    match slot {
        "top" => "T",
        "left_rail" => "L",
        "right_rail" => "R",
        "bottom_strip" => "B",
        "overlay" => "O",
        _ => "S",
    }
}

fn lcars_render_scene_slot_rank(slot: &str) -> usize {
    match slot {
        "top" => 0,
        "left_rail" => 1,
        "right_rail" => 2,
        "bottom_strip" => 3,
        "overlay" => 4,
        _ => 5,
    }
}

fn lcars_structural_mode_name(mode: LcarsStructuralMode) -> &'static str {
    match mode {
        LcarsStructuralMode::Composition => "composition",
        LcarsStructuralMode::PrimitiveLegend => "primitive_legend",
        LcarsStructuralMode::DockedSurface => "docked_surface",
        LcarsStructuralMode::BlockComposition => "block_composition",
        LcarsStructuralMode::TheLcarsControlPanel => "thelcars_control_panel",
    }
}

fn lcars_requested_profile_hint(document: &InterfaceDocument) -> Option<String> {
    document_property_value(
        document,
        &["profile", "layout_profile", "information_shape"],
    )
    .map(str::to_string)
    .or_else(|| {
        document.nodes.iter().find_map(|node| {
            [
                "profile",
                "layout_profile",
                "information_shape",
                "data_shape",
                "shape",
            ]
            .iter()
            .find_map(|key| node_property_value(node, key))
            .map(str::to_string)
        })
    })
}

fn lcars_palette_profile(document: &InterfaceDocument) -> LcarsPaletteProfile {
    let Some(value) = document_property_value(
        document,
        &[
            "palette_profile",
            "palette",
            "color_profile",
            "colour_profile",
            "lcars_palette",
            "theme_variant",
        ],
    ) else {
        return LcarsPaletteProfile::Classic;
    };
    let normalized = value.trim().to_ascii_lowercase().replace(['-', ' '], "_");
    match normalized.as_str() {
        "science"
        | "science_station"
        | "muted_science"
        | "muted_science_station"
        | "science_console"
        | "muted" => LcarsPaletteProfile::ScienceStation,
        "bright" | "bright_classic" | "classic_bright" | "classic_v24" | "lcars_v24" => {
            LcarsPaletteProfile::BrightClassic
        }
        "classic" | "default" => LcarsPaletteProfile::Classic,
        _ => LcarsPaletteProfile::Classic,
    }
}

fn lcars_palette_profile_name(profile: LcarsPaletteProfile) -> &'static str {
    match profile {
        LcarsPaletteProfile::Classic => "classic",
        LcarsPaletteProfile::BrightClassic => "bright_classic",
        LcarsPaletteProfile::ScienceStation => "science_station",
    }
}

fn has_structural_lcars_layout(document: &InterfaceDocument) -> bool {
    if lcars_corner_button_mode(document) {
        return false;
    }

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
                    | "docked_surface"
                    | "daily_shell"
                    | "lcars_shell"
                    | "operator_shell"
                    | "readable_shell"
                    | "terminal_shell"
                    | "owt_shell"
                    | "surface_dock"
                    | "console_surface"
                    | "lcars_dock"
                    | "sleek"
                    | "sleek_console"
                    | "clean_console"
                    | "low_noise"
                    | "thelcars"
                    | "thelcars_control_panel"
                    | "thelcars_private_demo"
                    | "lcars_website"
                    | "lcars_website_template"
                    | "primitive_legend"
                    | "primitive_coverage"
                    | "primitive_smoke"
                    | "primitive_checklist"
                    | "diagnostic_legend"
                    | "block_composition"
                    | "semantic_blocks"
                    | "labelled_box"
                    | "labeled_box"
                    | "control_block"
                    | "graphics_block"
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

fn lcars_structural_mode(document: &InterfaceDocument) -> LcarsStructuralMode {
    for node in &document.nodes {
        for key in [
            "profile",
            "layout_profile",
            "information_shape",
            "data_shape",
            "shape",
            "test",
            "diagnostic",
        ] {
            if let Some(value) = node_property_value(node, key) {
                let normalized = value.trim().to_ascii_lowercase().replace(['-', ' '], "_");
                if matches!(
                    normalized.as_str(),
                    "thelcars"
                        | "thelcars_control_panel"
                        | "thelcars_private_demo"
                        | "lcars_website"
                        | "lcars_website_template"
                        | "lcars_template"
                ) {
                    return LcarsStructuralMode::TheLcarsControlPanel;
                }
                if matches!(
                    normalized.as_str(),
                    "docked_surface"
                        | "daily_shell"
                        | "lcars_shell"
                        | "operator_shell"
                        | "readable_shell"
                        | "terminal_shell"
                        | "owt_shell"
                        | "surface_dock"
                        | "console_surface"
                        | "lcars_dock"
                        | "sleek"
                        | "sleek_console"
                        | "clean_console"
                        | "low_noise"
                ) {
                    return LcarsStructuralMode::DockedSurface;
                }
                if matches!(
                    normalized.as_str(),
                    "primitive_legend"
                        | "primitive_coverage"
                        | "primitive_smoke"
                        | "primitive_checklist"
                        | "legend"
                        | "diagnostic_legend"
                ) {
                    return LcarsStructuralMode::PrimitiveLegend;
                }
                if matches!(
                    normalized.as_str(),
                    "block_composition"
                        | "semantic_blocks"
                        | "labelled_box"
                        | "labeled_box"
                        | "control_block"
                        | "graphics_block"
                ) {
                    return LcarsStructuralMode::BlockComposition;
                }
            }
        }
    }
    LcarsStructuralMode::Composition
}

fn parse_lcars_panel_layout(value: &str) -> Option<LcarsPanelLayout> {
    let normalized = value.trim().to_ascii_lowercase().replace(['-', ' '], "_");
    match normalized.as_str() {
        "docked" | "docked_top" | "top" | "top_left" | "left_top" | "top_band" | "compact_top"
        | "full_dashboard" => Some(LcarsPanelLayout::Top),
        "left" | "left_rail" | "rail_left" | "docked_left" | "left_browser" | "browser_left" => {
            Some(LcarsPanelLayout::Left)
        }
        "right" | "right_rail" | "rail_right" | "side" | "side_rail" | "docked_right"
        | "right_browser" | "browser_right" => Some(LcarsPanelLayout::Right),
        "bottom" | "bottom_right" | "right_bottom" | "bottom_strip" | "status_strip"
        | "docked_bottom" | "alert_strip" => Some(LcarsPanelLayout::Bottom),
        "overlay" | "hud" | "heads_up" | "heads_up_display" | "free" | "undocked" | "widget"
        | "floating" => Some(LcarsPanelLayout::Overlay),
        _ => None,
    }
}

fn parse_lcars_surface_origin(value: &str) -> Option<LcarsSurfaceOrigin> {
    let normalized = value.trim().to_ascii_lowercase().replace(['-', ' '], "_");
    match normalized.as_str() {
        "top_left" | "left_top" | "tl" | "upper_left" | "northwest" | "north_west"
        | "corner_top_left" | "top_left_corner" | "elbow_top_left" => {
            Some(LcarsSurfaceOrigin::TopLeft)
        }
        "top" | "top_band" | "docked_top" | "upper" | "north" => Some(LcarsSurfaceOrigin::Top),
        "left" | "left_rail" | "rail_left" | "docked_left" | "west" => {
            Some(LcarsSurfaceOrigin::Left)
        }
        "right" | "right_rail" | "rail_right" | "docked_right" | "east" => {
            Some(LcarsSurfaceOrigin::Right)
        }
        "bottom" | "bottom_strip" | "docked_bottom" | "lower" | "south" => {
            Some(LcarsSurfaceOrigin::Bottom)
        }
        "bottom_right"
        | "right_bottom"
        | "br"
        | "lower_right"
        | "southeast"
        | "south_east"
        | "corner_bottom_right"
        | "bottom_right_corner" => Some(LcarsSurfaceOrigin::BottomRight),
        "overlay" | "hud" | "floating" | "free" | "undocked" | "widget" | "heads_up"
        | "heads_up_display" => Some(LcarsSurfaceOrigin::Overlay),
        _ => None,
    }
}

fn parse_lcars_surface_reservation(value: &str) -> Option<LcarsSurfaceReservation> {
    let normalized = value.trim().to_ascii_lowercase().replace(['-', ' '], "_");
    match normalized.as_str() {
        "reserve" | "reserved" | "true" | "yes" | "1" | "carve" | "carved" | "constrain"
        | "constrained" | "terminal_reserved" => Some(LcarsSurfaceReservation::Reserved),
        "overlay" | "floating" | "free" | "undocked" | "widget" | "hud" | "false" | "no" | "0"
        | "none" | "passthrough" => Some(LcarsSurfaceReservation::Overlay),
        _ => None,
    }
}

fn parse_lcars_surface_orientation(value: &str) -> Option<LcarsSurfaceOrientation> {
    let normalized = value.trim().to_ascii_lowercase().replace(['-', ' '], "_");
    match normalized.as_str() {
        "horizontal" | "h" | "row" | "rows" | "x" => Some(LcarsSurfaceOrientation::Horizontal),
        "vertical" | "v" | "column" | "columns" | "y" => Some(LcarsSurfaceOrientation::Vertical),
        "auto" | "adaptive" | "responsive" => Some(LcarsSurfaceOrientation::Auto),
        _ => None,
    }
}

fn parse_lcars_bool(value: &str) -> Option<bool> {
    let normalized = value.trim().to_ascii_lowercase().replace(['-', ' '], "_");
    match normalized.as_str() {
        "true" | "yes" | "1" | "on" | "show" | "shown" | "visible" | "active" | "enabled" => {
            Some(true)
        }
        "false" | "no" | "0" | "off" | "hide" | "hidden" | "none" | "inactive" | "disabled" => {
            Some(false)
        }
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

fn lcars_action_page_movement(key: &KeyCode) -> Option<isize> {
    match key {
        KeyCode::Char('[' | '{') => Some(-1),
        KeyCode::Char(']' | '}') => Some(1),
        _ => None,
    }
}

fn lcars_action_palette_key(key: &KeyCode) -> bool {
    matches!(key, KeyCode::Char('a' | 'A'))
}

fn lcars_table_focus_movement(
    key: &KeyCode,
    physical_key: Option<&PhysKeyCode>,
) -> Option<TableFocusMovement> {
    match physical_key {
        Some(PhysKeyCode::UpArrow) => return Some(TableFocusMovement::Previous),
        Some(PhysKeyCode::DownArrow) => return Some(TableFocusMovement::Next),
        Some(PhysKeyCode::LeftArrow) => return Some(TableFocusMovement::PreviousGroup),
        Some(PhysKeyCode::RightArrow) => return Some(TableFocusMovement::NextGroup),
        Some(PhysKeyCode::PageUp) => return Some(TableFocusMovement::First),
        Some(PhysKeyCode::PageDown) => return Some(TableFocusMovement::Last),
        _ => {}
    }

    match key {
        KeyCode::UpArrow => Some(TableFocusMovement::Previous),
        KeyCode::DownArrow => Some(TableFocusMovement::Next),
        KeyCode::LeftArrow => Some(TableFocusMovement::PreviousGroup),
        KeyCode::RightArrow => Some(TableFocusMovement::NextGroup),
        KeyCode::PageUp => Some(TableFocusMovement::First),
        KeyCode::PageDown => Some(TableFocusMovement::Last),
        _ => None,
    }
}

fn lcars_table_cell_focus_movement(
    key: &KeyCode,
    physical_key: Option<&PhysKeyCode>,
) -> Option<TableCellFocusMovement> {
    match physical_key {
        Some(PhysKeyCode::Comma) => return Some(TableCellFocusMovement::Previous),
        Some(PhysKeyCode::Period) => return Some(TableCellFocusMovement::Next),
        Some(PhysKeyCode::Home) => return Some(TableCellFocusMovement::First),
        Some(PhysKeyCode::End) => return Some(TableCellFocusMovement::Last),
        _ => {}
    }

    match key {
        KeyCode::Char(',' | '<') => Some(TableCellFocusMovement::Previous),
        KeyCode::Char('.' | '>') => Some(TableCellFocusMovement::Next),
        KeyCode::Home => Some(TableCellFocusMovement::First),
        KeyCode::End => Some(TableCellFocusMovement::Last),
        _ => None,
    }
}

fn lcars_table_activation_key(key: &KeyCode, physical_key: Option<&PhysKeyCode>) -> bool {
    matches!(
        physical_key,
        Some(PhysKeyCode::Return) | Some(PhysKeyCode::KeypadEnter)
    ) || matches!(key, KeyCode::Char('\r') | KeyCode::Char('\n'))
}

fn lcars_table_focus_mode_key(key: &KeyCode, physical_key: Option<&PhysKeyCode>) -> bool {
    matches!(physical_key, Some(PhysKeyCode::Space)) || matches!(key, KeyCode::Char(' '))
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

fn lcars_root_property_value<'a>(
    document: &'a InterfaceDocument,
    keys: &[&str],
) -> Option<&'a str> {
    let root = document.nodes.first()?;
    keys.iter()
        .find_map(|key| root.properties.get(*key).map(String::as_str))
}

fn lcars_float_property(document: &InterfaceDocument, keys: &[&str]) -> Option<f32> {
    lcars_root_property_value(document, keys).and_then(parse_lcars_float)
}

fn parse_lcars_float(value: &str) -> Option<f32> {
    let value = value.trim().trim_end_matches("px").trim();
    value.parse::<f32>().ok().filter(|value| value.is_finite())
}

fn document_property_value<'a>(document: &'a InterfaceDocument, keys: &[&str]) -> Option<&'a str> {
    for node in &document.nodes {
        for key in keys {
            if let Some(value) = node_property_value(node, key) {
                return Some(value);
            }
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
    let line = match &node.kind {
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
            node_text_with_provenance(
                format!(
                    "{}: {}",
                    node.label.as_deref().unwrap_or("METRIC"),
                    node_value_text(node, &["value", "state", "status"], "ONLINE")
                ),
                node,
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
        UiNodeKind::Table => node_summary_text(node, "TABLE").map(|text| {
            PanelLine::semantic(PanelLineKind::Table, node_text_with_provenance(text, node))
        }),
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
    };
    line.map(|line| apply_builder_highlight_to_line(node, line))
}

fn apply_builder_highlight_to_line(node: &UiNode, mut line: PanelLine) -> PanelLine {
    let Some(mode) = node
        .properties
        .get("owt_builder_highlight")
        .filter(|value| !value.trim().is_empty())
    else {
        return line;
    };
    let label = node
        .properties
        .get("owt_builder_highlight_label")
        .map(String::as_str)
        .filter(|value| !value.trim().is_empty())
        .unwrap_or("BUILD");
    line.builder_highlight = Some(LcarsBuilderHighlight {
        mode: mode.trim().to_ascii_uppercase(),
        label: label.trim().to_ascii_uppercase(),
    });
    line
}

fn node_text_with_provenance(text: String, node: &UiNode) -> String {
    match node_provenance_label(node) {
        Some(provenance) => format!("{text} [{provenance}]"),
        None => text,
    }
}

fn node_provenance_label(node: &UiNode) -> Option<String> {
    if let Some(provenance) = &node.provenance {
        return fact_provenance_label(provenance);
    }

    let state = node
        .properties
        .get("fact_state")
        .or_else(|| node.properties.get("provenance_state"))
        .or_else(|| {
            node.properties
                .get("state")
                .filter(|value| parse_fact_state(value).is_some())
        })
        .and_then(|value| parse_fact_state(value));
    let source = node
        .properties
        .get("source")
        .or_else(|| node.properties.get("provenance"))
        .cloned();
    let collected_at = node.properties.get("collected_at").cloned();
    let host = node.properties.get("host").cloned();
    let command = node.properties.get("command").cloned();
    let confidence = node.properties.get("confidence").cloned();
    let error = node.properties.get("error").cloned();

    if source.is_none()
        && collected_at.is_none()
        && host.is_none()
        && command.is_none()
        && confidence.is_none()
        && error.is_none()
        && state.is_none()
    {
        return None;
    }

    fact_provenance_label(&FactProvenance {
        source,
        collected_at,
        host,
        command,
        confidence,
        error,
        state,
    })
}

fn fact_provenance_label(provenance: &FactProvenance) -> Option<String> {
    if provenance
        .source
        .as_deref()
        .unwrap_or_default()
        .trim()
        .is_empty()
        && provenance
            .collected_at
            .as_deref()
            .unwrap_or_default()
            .trim()
            .is_empty()
        && provenance
            .host
            .as_deref()
            .unwrap_or_default()
            .trim()
            .is_empty()
        && provenance
            .command
            .as_deref()
            .unwrap_or_default()
            .trim()
            .is_empty()
        && provenance
            .confidence
            .as_deref()
            .unwrap_or_default()
            .trim()
            .is_empty()
        && provenance
            .error
            .as_deref()
            .unwrap_or_default()
            .trim()
            .is_empty()
        && provenance.state.is_none()
    {
        return None;
    }

    let mut parts = vec![fact_state_label(provenance.state.as_ref()).to_string()];
    if let Some(error) = provenance
        .error
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    {
        parts.push(compact_provenance_value(error, 28));
    } else if let Some(source) = provenance
        .source
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    {
        parts.push(compact_provenance_value(source, 28));
    } else if let Some(command) = provenance
        .command
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    {
        parts.push(compact_provenance_value(command, 28));
    } else if let Some(host) = provenance
        .host
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    {
        parts.push(compact_provenance_value(host, 20));
    }

    if let Some(collected_at) = provenance
        .collected_at
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    {
        parts.push(compact_provenance_timestamp(collected_at));
    }

    if let Some(confidence) = provenance
        .confidence
        .as_deref()
        .filter(|value| !value.trim().is_empty() && !value.eq_ignore_ascii_case("high"))
    {
        parts.push(format!("CONF {}", compact_provenance_value(confidence, 10)));
    }

    Some(parts.join(" "))
}

fn fact_state_label(state: Option<&FactState>) -> &'static str {
    match state {
        Some(FactState::Inferred) => "INF",
        Some(FactState::Stale) => "STALE",
        Some(FactState::Failed) => "FAIL",
        Some(FactState::Observed) | None => "OBS",
    }
}

fn parse_fact_state(value: &str) -> Option<FactState> {
    match normalize_lcars_token(value).as_str() {
        "observed" => Some(FactState::Observed),
        "inferred" => Some(FactState::Inferred),
        "stale" => Some(FactState::Stale),
        "failed" => Some(FactState::Failed),
        _ => None,
    }
}

fn compact_provenance_value(value: &str, max_chars: usize) -> String {
    let trimmed = value.trim();
    if trimmed.chars().count() <= max_chars {
        return trimmed.to_string();
    }
    let prefix_len = max_chars.saturating_sub(3).max(1);
    let prefix = trimmed.chars().take(prefix_len).collect::<String>();
    format!("{prefix}...")
}

fn compact_provenance_timestamp(value: &str) -> String {
    let normalized = value.trim().trim_end_matches('Z').replace('T', " ");
    if normalized.chars().count() <= 16 {
        normalized
    } else {
        normalized.chars().take(16).collect()
    }
}

fn lcars_primitive_legend_rows(document: &InterfaceDocument) -> Vec<LcarsPrimitiveLegendRow> {
    (1..=LCARS_PRIMITIVE_LEGEND_COUNT)
        .map(|number| {
            let label = primitive_legend_label(number);
            let kind = primitive_legend_line_kind(number);
            let node = first_node_matching(document, |node| {
                primitive_legend_kind_matches(number, &node.kind)
            });
            let present = node.is_some();
            let summary = node
                .and_then(|node| node_summary_text(node, label))
                .unwrap_or_else(|| "MISSING".to_string());
            LcarsPrimitiveLegendRow {
                number,
                label,
                kind,
                text: summary.to_uppercase(),
                present,
            }
        })
        .collect()
}

fn lcars_block_composition_data(document: &InterfaceDocument) -> LcarsBlockCompositionData {
    let container = first_node_matching(document, |node| {
        matches!(
            node.kind,
            UiNodeKind::Group | UiNodeKind::Region | UiNodeKind::Frame | UiNodeKind::ContentBay
        ) && node_has_any_property_value(
            node,
            &[
                "role",
                "profile",
                "layout_profile",
                "shape",
                "information_shape",
            ],
            &[
                "labelled_box",
                "labeled_box",
                "data_box",
                "block_composition",
                "control_cluster",
                "labelled_data",
                "labeled_data",
            ],
        )
    })
    .or_else(|| {
        first_node_matching(document, |node| {
            matches!(
                node.kind,
                UiNodeKind::Group | UiNodeKind::Region | UiNodeKind::ContentBay
            )
        })
    });

    let title = container
        .and_then(|node| node_summary_text(node, "CONTROL BLOCK"))
        .unwrap_or_else(|| document.title.clone())
        .to_uppercase();
    let mut labels = Vec::new();
    let mut boxes = Vec::new();
    if let Some(container) = container {
        collect_block_composition_fields(container, &mut labels, &mut boxes);
    }
    if boxes.is_empty() {
        for node in &document.nodes {
            collect_block_composition_fields(node, &mut labels, &mut boxes);
            if boxes.len() >= 4 {
                break;
            }
        }
    }

    for (index, box_item) in boxes.iter_mut().enumerate() {
        if let Some(label) = labels.get(index).filter(|label| !label.trim().is_empty()) {
            box_item.label = label.to_uppercase();
        }
    }
    if boxes.is_empty() {
        boxes.push(LcarsBlockDataBox {
            label: "DATA A".to_string(),
            value: "READY".to_string(),
        });
        boxes.push(LcarsBlockDataBox {
            label: "DATA B".to_string(),
            value: "STANDBY".to_string(),
        });
    }
    boxes.truncate(4);

    let graphics_node = first_node_matching(document, |node| {
        node.kind == UiNodeKind::Image
            || node_has_any_property_value(
                node,
                &[
                    "role",
                    "profile",
                    "layout_profile",
                    "shape",
                    "information_shape",
                ],
                &[
                    "graphics_panel",
                    "graphic_panel",
                    "scan_panel",
                    "plot",
                    "viewport",
                ],
            )
    });
    let has_graphics = graphics_node.is_some();
    let graphics_title = graphics_node
        .and_then(|node| node_summary_text(node, "GRAPHICS PANEL"))
        .unwrap_or_default()
        .to_uppercase();
    let graphics_detail = graphics_node
        .map(|node| node_value_text(node, &["detail", "value", "state", "status"], "ZOOM READY"))
        .unwrap_or_default()
        .to_uppercase();
    let command_bank_title =
        first_node_matching(document, |node| node.kind == UiNodeKind::CommandGrid)
            .and_then(|node| node_summary_text(node, "COMMAND BANK"))
            .unwrap_or_else(|| "COMMAND BANK".to_string())
            .to_uppercase();

    LcarsBlockCompositionData {
        title,
        boxes,
        graphics_title,
        graphics_detail,
        has_graphics,
        command_bank_title,
    }
}

fn collect_block_composition_fields(
    node: &UiNode,
    labels: &mut Vec<String>,
    boxes: &mut Vec<LcarsBlockDataBox>,
) {
    if boxes.len() < 4 && node.kind == UiNodeKind::DataCascade {
        boxes.push(LcarsBlockDataBox {
            label: node
                .label
                .clone()
                .or_else(|| node.properties.get("label").cloned())
                .unwrap_or_else(|| format!("DATA {}", boxes.len() + 1))
                .to_uppercase(),
            value: node_property_or_text(node, &["value", "state", "status"], "READY")
                .to_uppercase(),
        });
    } else if boxes.len() < 4 && node.kind == UiNodeKind::Metric {
        boxes.push(LcarsBlockDataBox {
            label: node
                .label
                .clone()
                .or_else(|| node.properties.get("label").cloned())
                .unwrap_or_else(|| format!("DATA {}", boxes.len() + 1))
                .to_uppercase(),
            value: node_value_text(node, &["value", "state", "status"], "READY").to_uppercase(),
        });
    } else if labels.len() < 4
        && node.kind == UiNodeKind::Text
        && node_has_any_property_value(node, &["role", "usage", "kind"], &["label", "caption"])
    {
        labels.push(
            node_value_text(
                node,
                &["value", "title"],
                node.label.as_deref().unwrap_or("LABEL"),
            )
            .to_uppercase(),
        );
    }

    for child in &node.children {
        if boxes.len() >= 4 && labels.len() >= 4 {
            break;
        }
        collect_block_composition_fields(child, labels, boxes);
    }
}

fn node_property_or_text(node: &UiNode, keys: &[&str], fallback: &str) -> String {
    for key in keys {
        if let Some(value) = node
            .properties
            .get(*key)
            .filter(|value| !value.trim().is_empty())
        {
            return value.to_string();
        }
    }
    node.text
        .as_deref()
        .and_then(|text| text.lines().find(|line| !line.trim().is_empty()))
        .map(str::to_string)
        .unwrap_or_else(|| fallback.to_string())
}

fn node_has_any_property_value(node: &UiNode, keys: &[&str], values: &[&str]) -> bool {
    keys.iter().any(|key| {
        let direct_value = if *key == "role" {
            node.role.as_ref().or_else(|| node.properties.get(*key))
        } else {
            node.properties.get(*key)
        };
        direct_value
            .map(|value| normalize_lcars_token(value))
            .map(|value| {
                values
                    .iter()
                    .any(|expected| value == normalize_lcars_token(expected))
            })
            .unwrap_or(false)
    })
}

fn normalize_lcars_token(value: &str) -> String {
    value.trim().to_ascii_lowercase().replace(['-', ' '], "_")
}

fn primitive_legend_kind_matches(index: usize, kind: &UiNodeKind) -> bool {
    matches!(
        (index, kind),
        (1, UiNodeKind::Panel)
            | (2, UiNodeKind::Region)
            | (3, UiNodeKind::Group)
            | (4, UiNodeKind::Frame)
            | (5, UiNodeKind::SideRail)
            | (6, UiNodeKind::ContentBay)
            | (7, UiNodeKind::Text)
            | (8, UiNodeKind::Bar)
            | (9, UiNodeKind::BarRun)
            | (10, UiNodeKind::Elbow)
            | (11, UiNodeKind::CommandGrid)
            | (12, UiNodeKind::DataCascade)
            | (13, UiNodeKind::Button)
            | (14, UiNodeKind::Badge)
            | (15, UiNodeKind::List)
            | (16, UiNodeKind::Table)
            | (17, UiNodeKind::Metric)
            | (18, UiNodeKind::Progress)
            | (19, UiNodeKind::Image)
            | (20, UiNodeKind::Spacer)
    )
}

fn primitive_legend_line_kind(index: usize) -> PanelLineKind {
    match index {
        1 | 2 | 3 | 20 => PanelLineKind::Section,
        4 => PanelLineKind::Frame,
        5 => PanelLineKind::SideRail,
        6 => PanelLineKind::ContentBay,
        7 => PanelLineKind::Text,
        8 => PanelLineKind::Bar,
        9 => PanelLineKind::BarRun,
        10 => PanelLineKind::Elbow,
        11 => PanelLineKind::CommandGrid,
        12 => PanelLineKind::DataCascade,
        13 => PanelLineKind::Button,
        14 => PanelLineKind::Badge,
        15 => PanelLineKind::List,
        16 => PanelLineKind::Table,
        17 => PanelLineKind::Metric,
        18 => PanelLineKind::Progress,
        19 => PanelLineKind::Image,
        _ => PanelLineKind::Text,
    }
}

fn primitive_legend_label(index: usize) -> &'static str {
    match index {
        1 => "PANEL",
        2 => "REGION",
        3 => "GROUP",
        4 => "FRAME",
        5 => "SIDE_RAIL",
        6 => "CONTENT_BAY",
        7 => "TEXT",
        8 => "BAR",
        9 => "BAR_RUN",
        10 => "ELBOW",
        11 => "COMMAND_GRID",
        12 => "DATA_CASCADE",
        13 => "BUTTON",
        14 => "BADGE",
        15 => "LIST",
        16 => "TABLE",
        17 => "METRIC",
        18 => "PROGRESS",
        19 => "IMAGE",
        20 => "SPACER",
        _ => "UNKNOWN",
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
    let sort_spec = table_sort_spec(node, &columns);
    let mut rows = parse_table_rows(node, &columns);
    if let Some(sort_spec) = sort_spec {
        rows.sort_by(|left, right| compare_table_rows(left, right, sort_spec));
    }
    let focused_group = table_focus_group_target(node);
    let group_focus = table_group_focus_target(node);
    let derived_group_summaries =
        table_group_summaries(&rows, focused_group.as_deref().or(group_focus.as_deref()));
    let group_summaries = merge_table_group_summaries(
        explicit_table_group_summaries(
            document,
            node,
            focused_group.as_deref().or(group_focus.as_deref()),
        ),
        derived_group_summaries,
    );
    let group_detail = table_group_detail(
        &group_summaries,
        &rows,
        &columns,
        group_focus.as_deref().or(focused_group.as_deref()),
    );
    let total_rows = rows.len();
    let focus_label = group_focus.and_then(|group| {
        let group_token = normalize_lcars_token(&group);
        let mut focused_rows = rows
            .iter()
            .filter(|row| {
                row.group.as_deref().map(normalize_lcars_token).as_deref()
                    == Some(group_token.as_str())
            })
            .cloned()
            .collect::<Vec<_>>();
        if focused_rows.is_empty() {
            return None;
        }
        let hidden_rows = total_rows.saturating_sub(focused_rows.len());
        let label = if hidden_rows > 0 {
            format!(
                "FOCUS {} {}R +{} OUT",
                group,
                focused_rows.len(),
                hidden_rows
            )
        } else {
            format!("FOCUS {} {}R", group, focused_rows.len())
        };
        rows.clear();
        rows.append(&mut focused_rows);
        Some(label)
    });
    let overflow_rows = rows.len().saturating_sub(max_rows);
    let focused_detail = table_focused_row_detail(&rows, &columns);
    let cell_detail = table_focused_cell_detail(&rows, &columns);
    let cell_focus = table_focused_cell_focus(node, &rows, &columns);
    rows.truncate(max_rows);
    if rows.is_empty() && columns.is_empty() {
        return None;
    }

    Some(LcarsTableData {
        interface_id: document.id.clone(),
        title: node
            .label
            .clone()
            .or_else(|| node.properties.get("title").cloned())
            .unwrap_or_else(|| "TABLE BAY".to_string()),
        provenance: node_provenance_label(node),
        sort_label: table_sort_label(node, &columns, sort_spec),
        focus_label,
        group_detail,
        focused_detail,
        cell_detail,
        cell_focus,
        columns,
        rows,
        group_summaries,
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

fn parse_table_rows(node: &UiNode, columns: &[String]) -> Vec<LcarsTableRow> {
    let mut rows = Vec::new();
    let expected_columns = columns.len().max(1);
    let group_index = table_group_index(node, columns);
    let focus_target = table_focus_target(node);
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
                cell_severity: vec![None; expected_columns],
                group: table_row_group_from_cells(&cells, group_index),
                provenance: None,
                cell_provenance: vec![None; expected_columns],
                action_id: None,
                focused: table_row_matches_focus(&cells, None, None, &[], focus_target.as_deref()),
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
            cell_severity: parse_table_cell_metadata(
                child,
                &[
                    "cell_severity",
                    "cell_severities",
                    "severity_cells",
                    "cell_states",
                ],
                expected_columns,
            ),
            group: child
                .properties
                .get("group")
                .or_else(|| child.properties.get("row_group"))
                .cloned()
                .or_else(|| table_row_group_from_cells(&cells, group_index)),
            provenance: node_provenance_label(child),
            cell_provenance: parse_table_cell_metadata(
                child,
                &["cell_provenance", "cell_provenances", "provenance_cells"],
                expected_columns,
            ),
            action_id: child
                .action_id
                .clone()
                .or_else(|| child.properties.get("action_id").cloned())
                .or_else(|| child.properties.get("drilldown_action_id").cloned())
                .or_else(|| child.properties.get("focus_action_id").cloned()),
            focused: bool_property(child, &["focused", "selected", "focus"])
                || table_row_matches_focus(
                    &cells,
                    Some(child.id.as_str()),
                    child.action_id.as_deref(),
                    &[
                        child.properties.get("focus_key").map(String::as_str),
                        child.properties.get("row_key").map(String::as_str),
                    ],
                    focus_target.as_deref(),
                ),
            cells,
        });
    }
    rows
}

fn table_focused_row_detail(rows: &[LcarsTableRow], columns: &[String]) -> Option<String> {
    let row = rows.iter().find(|row| row.focused)?;
    let mut parts = Vec::new();

    if let Some(group) = row
        .group
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    {
        parts.push(format!("GROUP {}", group.trim()));
    }
    if let Some(severity) = row
        .severity
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    {
        parts.push(format!("STATE {}", severity.trim()));
    }
    if let Some(action_id) = row
        .action_id
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    {
        parts.push(format!("DRILL {}", action_id.trim()));
    }

    for (index, cell) in row.cells.iter().enumerate().take(3) {
        let cell = cell.trim();
        if cell.is_empty() {
            continue;
        }
        let column = columns
            .get(index)
            .map(String::as_str)
            .unwrap_or("CELL")
            .trim();
        if column.is_empty() {
            parts.push(cell.to_string());
        } else {
            parts.push(format!("{} {}", column.to_ascii_uppercase(), cell));
        }
    }

    if let Some(provenance) = row
        .provenance
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    {
        parts.push(format!("PROV {}", provenance.trim()));
    }

    let cell_marks = row
        .cell_severity
        .iter()
        .filter(|value| value.is_some())
        .count()
        + row
            .cell_provenance
            .iter()
            .filter(|value| value.is_some())
            .count();
    if cell_marks > 0 {
        parts.push(format!("CELL {}M", cell_marks));
    }

    if parts.is_empty() {
        None
    } else {
        Some(parts.join(" | "))
    }
}

fn table_focused_cell_detail(rows: &[LcarsTableRow], columns: &[String]) -> Option<String> {
    let row = rows.iter().find(|row| row.focused)?;
    let mut parts = Vec::new();

    for index in 0..row.cells.len().max(columns.len()) {
        let severity = row
            .cell_severity
            .get(index)
            .and_then(|value| value.as_deref())
            .filter(|value| !value.trim().is_empty());
        let provenance = row
            .cell_provenance
            .get(index)
            .and_then(|value| value.as_deref())
            .filter(|value| !value.trim().is_empty());
        if severity.is_none() && provenance.is_none() {
            continue;
        }
        let column = columns
            .get(index)
            .map(String::as_str)
            .unwrap_or("CELL")
            .trim();
        let column = if column.is_empty() { "CELL" } else { column };
        let mut marks = Vec::new();
        if let Some(severity) = severity {
            marks.push(format!("S:{}", severity.trim()));
        }
        if let Some(provenance) = provenance {
            marks.push(format!("P:{}", provenance.trim()));
        }
        parts.push(format!(
            "{} {}",
            column.to_ascii_uppercase(),
            marks.join("/")
        ));
        if parts.len() >= 4 {
            break;
        }
    }

    if parts.is_empty() {
        None
    } else {
        Some(parts.join(" | "))
    }
}

fn table_focused_cell_focus(
    node: &UiNode,
    rows: &[LcarsTableRow],
    columns: &[String],
) -> Option<LcarsTableCellFocus> {
    let row = rows.iter().find(|row| row.focused)?;
    let column_index = table_cell_focus_index(node, row, columns)?;
    let column = columns
        .get(column_index)
        .map(String::as_str)
        .unwrap_or("CELL")
        .trim();
    let column = if column.is_empty() { "CELL" } else { column };
    let value = row
        .cells
        .get(column_index)
        .map(String::as_str)
        .unwrap_or("")
        .trim();
    let severity = row
        .cell_severity
        .get(column_index)
        .and_then(|value| value.as_deref())
        .filter(|value| !value.trim().is_empty())
        .or(row.severity.as_deref());
    let provenance = row
        .cell_provenance
        .get(column_index)
        .and_then(|value| value.as_deref())
        .filter(|value| !value.trim().is_empty())
        .or(row.provenance.as_deref());

    let mut parts = vec![format!("{} {}", column.to_ascii_uppercase(), value)];
    if let Some(severity) = severity.filter(|value| !value.trim().is_empty()) {
        parts.push(format!("SEV {}", severity.trim()));
    }
    if let Some(provenance) = provenance.filter(|value| !value.trim().is_empty()) {
        parts.push(format!("PROV {}", provenance.trim()));
    }
    if let Some(group) = row
        .group
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    {
        parts.push(format!("GROUP {}", group.trim()));
    }
    if let Some(row_key) = row
        .cells
        .first()
        .map(String::as_str)
        .filter(|value| !value.trim().is_empty())
    {
        let key_label = columns
            .first()
            .map(String::as_str)
            .filter(|value| !value.trim().is_empty())
            .unwrap_or("ROW");
        parts.push(format!(
            "{} {}",
            key_label.to_ascii_uppercase(),
            row_key.trim()
        ));
    }
    if let Some(action_id) = row
        .action_id
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    {
        parts.push(format!("DRILL {}", action_id.trim()));
    }

    Some(LcarsTableCellFocus {
        column_index,
        label: format!("CELL {}", column.to_ascii_uppercase()),
        detail: parts.join(" | "),
    })
}

fn table_cell_focus_index(node: &UiNode, row: &LcarsTableRow, columns: &[String]) -> Option<usize> {
    if let Some(index) = node
        .properties
        .get("focused_cell")
        .or_else(|| node.properties.get("focus_cell"))
        .or_else(|| node.properties.get("selected_cell"))
        .or_else(|| node.properties.get("focused_column"))
        .or_else(|| node.properties.get("focus_column"))
        .or_else(|| node.properties.get("selected_column"))
        .and_then(|value| table_column_index(columns, value))
    {
        return Some(index);
    }

    let marked_index = row
        .cell_severity
        .iter()
        .enumerate()
        .filter_map(|(index, severity)| {
            severity
                .as_deref()
                .map(|value| (index, table_severity_rank(value)))
        })
        .max_by_key(|(_, rank)| *rank)
        .map(|(index, _)| index)
        .or_else(|| row.cell_provenance.iter().position(Option::is_some));
    marked_index
}

fn table_group_index(node: &UiNode, columns: &[String]) -> Option<usize> {
    node.properties
        .get("group_by")
        .or_else(|| node.properties.get("group_column"))
        .or_else(|| node.properties.get("group"))
        .and_then(|value| table_column_index(columns, value))
}

fn parse_table_cell_metadata(
    node: &UiNode,
    keys: &[&str],
    expected_columns: usize,
) -> Vec<Option<String>> {
    let mut values = keys
        .iter()
        .find_map(|key| node.properties.get(*key))
        .map(|value| split_table_cells(value))
        .unwrap_or_default()
        .into_iter()
        .map(|value| {
            let value = value.trim().to_string();
            (!value.is_empty() && !matches!(normalize_lcars_token(&value).as_str(), "none" | "-"))
                .then_some(value)
        })
        .collect::<Vec<_>>();
    let expected_columns = expected_columns.max(1);
    values.truncate(expected_columns);
    while values.len() < expected_columns {
        values.push(None);
    }
    values
}

fn table_sort_spec(node: &UiNode, columns: &[String]) -> Option<TableSortSpec> {
    let column_index = node
        .properties
        .get("sort_by")
        .or_else(|| node.properties.get("sort_column"))
        .or_else(|| node.properties.get("sort"))
        .and_then(|value| table_column_index(columns, value))?;
    let descending = node
        .properties
        .get("sort_direction")
        .or_else(|| node.properties.get("sort_dir"))
        .or_else(|| node.properties.get("direction"))
        .map(|value| {
            matches!(
                normalize_lcars_token(value).as_str(),
                "desc" | "descending" | "down" | "reverse"
            )
        })
        .unwrap_or(false);
    Some(TableSortSpec {
        column_index,
        descending,
    })
}

fn table_sort_label(
    node: &UiNode,
    columns: &[String],
    sort_spec: Option<TableSortSpec>,
) -> Option<String> {
    let sort_spec = sort_spec?;
    let column = columns
        .get(sort_spec.column_index)
        .map(String::as_str)
        .unwrap_or("VALUE");
    let direction = if sort_spec.descending { "DESC" } else { "ASC" };
    let label = node
        .properties
        .get("sort_label")
        .cloned()
        .unwrap_or_else(|| format!("SORT {} {}", column.to_uppercase(), direction));
    Some(label)
}

fn table_group_summaries(
    rows: &[LcarsTableRow],
    focused_group: Option<&str>,
) -> Vec<LcarsTableGroupSummary> {
    let mut summaries: Vec<LcarsTableGroupSummary> = Vec::new();
    let focused_group = focused_group.map(normalize_lcars_token);
    for row in rows {
        let Some(group) = row
            .group
            .as_ref()
            .map(|value| value.trim())
            .filter(|value| !value.is_empty())
        else {
            continue;
        };
        let group_key = normalize_lcars_token(group);
        let summary_index = summaries
            .iter()
            .position(|summary| normalize_lcars_token(&summary.group) == group_key)
            .unwrap_or_else(|| {
                summaries.push(LcarsTableGroupSummary {
                    group: group.to_string(),
                    row_count: 0,
                    action_count: 0,
                    focused: false,
                    provenance_count: 0,
                    severity: None,
                });
                summaries.len() - 1
            });
        let summary = &mut summaries[summary_index];
        summary.row_count += 1;
        if row.action_id.is_some() {
            summary.action_count += 1;
        }
        summary.focused |= row.focused;
        if row.provenance.is_some() {
            summary.provenance_count += 1;
        }
        if focused_group.as_deref() == Some(group_key.as_str()) {
            summary.focused = true;
        }
        summary.severity =
            table_worst_severity(summary.severity.as_deref(), row.severity.as_deref());
    }
    summaries
}

fn explicit_table_group_summaries(
    document: &InterfaceDocument,
    table: &UiNode,
    focused_group: Option<&str>,
) -> Vec<LcarsTableGroupSummary> {
    let summary_node_id = table.properties.get("cohort_summary_node");
    let source_table_id = table.id.as_str();
    let mut summaries = Vec::new();
    for node in document.nodes.iter().flat_map(flatten_node) {
        let role = node.role.as_deref().map(normalize_lcars_token);
        let source_table = node.properties.get("source_table").map(String::as_str);
        let matches_role = role.as_deref() == Some("cohort_summary");
        let matches_id = summary_node_id
            .map(|expected| node.id == *expected)
            .unwrap_or(false);
        let matches_source = source_table == Some(source_table_id);
        if !matches_role && !matches_id && !matches_source {
            continue;
        }
        for child in &node.children {
            let child_role = child.role.as_deref().map(normalize_lcars_token);
            if child_role.as_deref() != Some("cohort")
                && !child.properties.contains_key("cohort_value")
            {
                continue;
            }
            let Some(group) = child
                .properties
                .get("cohort_value")
                .or_else(|| child.properties.get("group"))
                .or_else(|| child.properties.get("row_group"))
                .or(child.label.as_ref())
                .map(|value| value.trim())
                .filter(|value| !value.is_empty())
            else {
                continue;
            };
            let group_key = normalize_lcars_token(group);
            let focused = focused_group.map(normalize_lcars_token).as_deref()
                == Some(group_key.as_str())
                || bool_property(child, &["focused", "selected", "focus"]);
            summaries.push(LcarsTableGroupSummary {
                group: group.to_string(),
                row_count: usize_property(child, &["row_count", "rows", "count"]).unwrap_or(0),
                action_count: usize_property(
                    child,
                    &["drilldown_count", "action_count", "actions"],
                )
                .unwrap_or(0),
                focused,
                provenance_count: usize_property(child, &["provenance_count", "provenance"])
                    .unwrap_or_else(|| usize::from(child.provenance.is_some())),
                severity: child
                    .properties
                    .get("severity")
                    .or_else(|| child.properties.get("state"))
                    .or_else(|| child.properties.get("status"))
                    .filter(|value| !value.trim().is_empty())
                    .cloned(),
            });
        }
    }
    summaries
}

fn merge_table_group_summaries(
    mut explicit: Vec<LcarsTableGroupSummary>,
    derived: Vec<LcarsTableGroupSummary>,
) -> Vec<LcarsTableGroupSummary> {
    if explicit.is_empty() {
        return derived;
    }
    for derived_summary in derived {
        let derived_key = normalize_lcars_token(&derived_summary.group);
        if let Some(summary) = explicit
            .iter_mut()
            .find(|summary| normalize_lcars_token(&summary.group) == derived_key)
        {
            if summary.row_count == 0 {
                summary.row_count = derived_summary.row_count;
            }
            if summary.action_count == 0 {
                summary.action_count = derived_summary.action_count;
            }
            summary.focused |= derived_summary.focused;
            if summary.provenance_count == 0 {
                summary.provenance_count = derived_summary.provenance_count;
            }
            summary.severity = table_worst_severity(
                summary.severity.as_deref(),
                derived_summary.severity.as_deref(),
            );
        } else {
            explicit.push(derived_summary);
        }
    }
    explicit
}

fn flatten_node(node: &UiNode) -> Vec<&UiNode> {
    let mut nodes = vec![node];
    for child in &node.children {
        nodes.extend(flatten_node(child));
    }
    nodes
}

fn table_group_summary_for<'a>(
    summaries: &'a [LcarsTableGroupSummary],
    group: &str,
) -> Option<&'a LcarsTableGroupSummary> {
    let group = normalize_lcars_token(group);
    summaries
        .iter()
        .find(|summary| normalize_lcars_token(&summary.group) == group)
}

fn table_group_summary_label(summary: &LcarsTableGroupSummary) -> String {
    let mut parts = vec![
        format!("GRP {}", summary.group),
        format!("{}R", summary.row_count),
    ];
    if summary.action_count > 0 {
        parts.push(format!("{}D", summary.action_count));
    }
    if summary.focused {
        parts.push("FOCUS".to_string());
    }
    if summary.provenance_count > 0 {
        parts.push(format!("{}P", summary.provenance_count));
    }
    if let Some(severity) = summary
        .severity
        .as_deref()
        .and_then(table_group_severity_label)
    {
        parts.push(severity.to_string());
    }
    parts.join(" ")
}

fn table_group_detail(
    summaries: &[LcarsTableGroupSummary],
    rows: &[LcarsTableRow],
    columns: &[String],
    preferred_group: Option<&str>,
) -> Option<String> {
    let summary = preferred_group
        .and_then(|group| table_group_summary_for(summaries, group))
        .or_else(|| summaries.iter().find(|summary| summary.focused))
        .or_else(|| (summaries.len() == 1).then(|| &summaries[0]))?;
    let group_key = normalize_lcars_token(&summary.group);
    let group_rows = rows
        .iter()
        .filter(|row| {
            row.group.as_deref().map(normalize_lcars_token).as_deref() == Some(group_key.as_str())
        })
        .collect::<Vec<_>>();
    let mut parts = vec![
        format!("COHORT {}", summary.group.trim()),
        format!("ROWS {}", summary.row_count),
    ];
    if summary.action_count > 0 {
        parts.push(format!("DRILL {}", summary.action_count));
    }
    if summary.provenance_count > 0 {
        parts.push(format!("PROV {}", summary.provenance_count));
    }
    let cell_mark_count = group_rows
        .iter()
        .map(|row| {
            row.cell_severity
                .iter()
                .filter(|value| value.is_some())
                .count()
                + row
                    .cell_provenance
                    .iter()
                    .filter(|value| value.is_some())
                    .count()
        })
        .sum::<usize>();
    if cell_mark_count > 0 {
        parts.push(format!("CELL {}", cell_mark_count));
    }
    if let Some(severity) = summary
        .severity
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    {
        let severity = table_group_severity_label(severity)
            .unwrap_or_else(|| severity.trim())
            .to_ascii_uppercase();
        parts.push(format!("STATE {severity}"));
    }
    if summary.focused {
        parts.push("FOCUS".to_string());
    }
    let representatives = group_rows
        .iter()
        .filter_map(|row| {
            row.cells
                .first()
                .or_else(|| row.cells.iter().find(|cell| !cell.trim().is_empty()))
        })
        .map(|cell| cell.trim())
        .filter(|cell| !cell.is_empty())
        .take(2)
        .collect::<Vec<_>>();
    if !representatives.is_empty() {
        let representative_label = columns
            .first()
            .map(|column| column.trim())
            .filter(|column| !column.is_empty())
            .unwrap_or("ROW")
            .to_ascii_uppercase();
        parts.push(format!(
            "{} {}",
            representative_label,
            representatives.join(",")
        ));
    }
    Some(parts.join(" | "))
}

fn table_worst_severity(left: Option<&str>, right: Option<&str>) -> Option<String> {
    match (left, right) {
        (None, None) => None,
        (Some(value), None) | (None, Some(value)) => Some(value.to_string()),
        (Some(left), Some(right)) => {
            if table_severity_rank(right) > table_severity_rank(left) {
                Some(right.to_string())
            } else {
                Some(left.to_string())
            }
        }
    }
}

fn table_group_severity_label(severity: &str) -> Option<&'static str> {
    match table_severity_rank(severity) {
        3 => Some("ERR"),
        2 => Some("WARN"),
        1 => Some("OK"),
        _ => None,
    }
}

fn table_severity_rank(severity: &str) -> u8 {
    match normalize_lcars_token(severity).as_str() {
        "error" | "blocked" | "critical" => 3,
        "warning" | "stale" | "legacy" => 2,
        "success" | "ok" | "ready" | "current" => 1,
        _ => 0,
    }
}

fn table_column_index(columns: &[String], value: &str) -> Option<usize> {
    let value = value.trim();
    if value.is_empty() || matches!(normalize_lcars_token(value).as_str(), "none" | "off") {
        return None;
    }
    if let Ok(index) = value.parse::<usize>() {
        if index == 0 {
            return None;
        }
        return (index - 1 < columns.len()).then_some(index - 1);
    }
    let normalized = normalize_lcars_token(value);
    columns
        .iter()
        .position(|column| normalize_lcars_token(column) == normalized)
}

fn usize_property(node: &UiNode, keys: &[&str]) -> Option<usize> {
    keys.iter()
        .find_map(|key| node.properties.get(*key))
        .and_then(|value| value.trim().parse::<usize>().ok())
}

fn table_focus_target(node: &UiNode) -> Option<String> {
    node.properties
        .get("focused_row")
        .or_else(|| node.properties.get("focus_row"))
        .or_else(|| node.properties.get("selected_row"))
        .filter(|value| !value.trim().is_empty())
        .cloned()
}

fn table_focus_group_target(node: &UiNode) -> Option<String> {
    node.properties
        .get("focused_group")
        .or_else(|| node.properties.get("focus_group"))
        .or_else(|| node.properties.get("selected_group"))
        .filter(|value| !value.trim().is_empty())
        .cloned()
}

fn table_group_focus_target(node: &UiNode) -> Option<String> {
    node.properties
        .get("focus_mode")
        .filter(|value| matches!(normalize_lcars_token(value).as_str(), "group" | "cohort"))
        .and_then(|_| {
            node.properties
                .get("expanded_group")
                .or_else(|| node.properties.get("focused_group"))
                .or_else(|| node.properties.get("focus_group"))
                .or_else(|| node.properties.get("selected_group"))
        })
        .filter(|value| !value.trim().is_empty())
        .cloned()
}

fn table_row_group_from_cells(cells: &[String], group_index: Option<usize>) -> Option<String> {
    group_index
        .and_then(|index| cells.get(index))
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn table_row_matches_focus(
    cells: &[String],
    row_id: Option<&str>,
    action_id: Option<&str>,
    extra_identifiers: &[Option<&str>],
    focus_target: Option<&str>,
) -> bool {
    let Some(focus_target) = focus_target
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return false;
    };
    let focus_target = normalize_lcars_token(focus_target);
    row_id
        .map(|value| normalize_lcars_token(value) == focus_target)
        .unwrap_or(false)
        || action_id
            .map(|value| normalize_lcars_token(value) == focus_target)
            .unwrap_or(false)
        || extra_identifiers
            .iter()
            .flatten()
            .any(|value| normalize_lcars_token(value) == focus_target)
        || cells
            .first()
            .map(|value| normalize_lcars_token(value) == focus_target)
            .unwrap_or(false)
}

fn bool_property(node: &UiNode, keys: &[&str]) -> bool {
    keys.iter().any(|key| {
        node.properties
            .get(*key)
            .map(|value| {
                matches!(
                    normalize_lcars_token(value).as_str(),
                    "1" | "true" | "yes" | "on"
                )
            })
            .unwrap_or(false)
    })
}

fn compare_table_rows(
    left: &LcarsTableRow,
    right: &LcarsTableRow,
    sort_spec: TableSortSpec,
) -> std::cmp::Ordering {
    let left_value = left
        .cells
        .get(sort_spec.column_index)
        .map(|value| value.to_ascii_lowercase())
        .unwrap_or_default();
    let right_value = right
        .cells
        .get(sort_spec.column_index)
        .map(|value| value.to_ascii_lowercase())
        .unwrap_or_default();
    if sort_spec.descending {
        right_value.cmp(&left_value)
    } else {
        left_value.cmp(&right_value)
    }
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
        PanelLineKind::Status => RgbColor::new_8bpc(207, 79, 79),
        PanelLineKind::Frame => RgbColor::new_8bpc(255, 149, 96),
        PanelLineKind::Section => RgbColor::new_8bpc(255, 204, 112),
        PanelLineKind::Metric => RgbColor::new_8bpc(153, 204, 255),
        PanelLineKind::Badge => RgbColor::new_8bpc(255, 149, 96),
        PanelLineKind::Progress => RgbColor::new_8bpc(153, 204, 255),
        PanelLineKind::Bar | PanelLineKind::BarRun => RgbColor::new_8bpc(255, 204, 112),
        PanelLineKind::Elbow => RgbColor::new_8bpc(197, 143, 255),
        PanelLineKind::SideRail | PanelLineKind::ContentBay => RgbColor::new_8bpc(153, 204, 255),
        PanelLineKind::CommandGrid => RgbColor::new_8bpc(255, 149, 96),
        PanelLineKind::DataCascade => RgbColor::new_8bpc(197, 143, 255),
        PanelLineKind::List | PanelLineKind::Table => RgbColor::new_8bpc(153, 204, 255),
        PanelLineKind::Image => RgbColor::new_8bpc(137, 148, 255),
        PanelLineKind::Button | PanelLineKind::Text => match index {
            0 => RgbColor::new_8bpc(153, 204, 255),
            1 => RgbColor::new_8bpc(255, 204, 112),
            2 => RgbColor::new_8bpc(197, 143, 255),
            _ => RgbColor::new_8bpc(255, 149, 96),
        },
    }
}

fn lcars_action_text_color(index: usize) -> RgbColor {
    match index % 4 {
        0 => RgbColor::new_8bpc(255, 149, 96),
        1 => RgbColor::new_8bpc(197, 143, 255),
        2 => RgbColor::new_8bpc(153, 204, 255),
        _ => RgbColor::new_8bpc(255, 204, 112),
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
        if fill_byte == LCARS_BYTE_PEACH {
            LCARS_BYTE_AMBER
        } else {
            LCARS_BYTE_PEACH
        }
    } else {
        fill_byte
    }
}

fn lcars_primitive_legend_fill_byte(row: &LcarsPrimitiveLegendRow) -> LcarsByteColor {
    if !row.present {
        return LCARS_BYTE_RED;
    }
    match row.kind {
        PanelLineKind::Frame
        | PanelLineKind::Section
        | PanelLineKind::Bar
        | PanelLineKind::BarRun
        | PanelLineKind::Elbow
        | PanelLineKind::SideRail
        | PanelLineKind::ContentBay => LCARS_BYTE_ORANGE,
        PanelLineKind::CommandGrid | PanelLineKind::Button => LCARS_BYTE_VIOLET,
        PanelLineKind::Badge | PanelLineKind::Status => LCARS_BYTE_PEACH,
        PanelLineKind::DataCascade
        | PanelLineKind::List
        | PanelLineKind::Table
        | PanelLineKind::Metric
        | PanelLineKind::Progress
        | PanelLineKind::Image
        | PanelLineKind::Text => LCARS_BYTE_BLUE,
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct LcarsDepthMetrics {
    shadow_offset: f32,
    shadow_alpha: f32,
    top_edge_alpha: f32,
    bottom_edge_alpha: f32,
    inner_edge_alpha: f32,
}

fn lcars_depth_metrics(
    width: f32,
    height: f32,
    active: bool,
    hovered: bool,
) -> Option<LcarsDepthMetrics> {
    if width <= 1.0 || height <= 1.0 {
        return None;
    }

    let scale = (height / 40.0).clamp(0.70, 1.20);
    let (shadow_offset, shadow_alpha, top_edge_alpha, bottom_edge_alpha, inner_edge_alpha) =
        if active {
            (1.0 * scale, 0.18, 0.07, 0.34, 0.28)
        } else if hovered {
            (3.5 * scale, 0.24, 0.28, 0.24, 0.18)
        } else {
            (2.5 * scale, 0.18, 0.20, 0.18, 0.13)
        };

    Some(LcarsDepthMetrics {
        shadow_offset,
        shadow_alpha,
        top_edge_alpha,
        bottom_edge_alpha,
        inner_edge_alpha,
    })
}

fn lcars_byte_color(fill: LcarsByteColor) -> LinearRgba {
    color(fill.red, fill.green, fill.blue)
}

fn paint_lcars_cast_shadow(
    window: &mut crate::TermWindow,
    layers: &mut TripleLayerQuadAllocator,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    active: bool,
    hovered: bool,
) -> anyhow::Result<()> {
    let Some(metrics) = lcars_depth_metrics(width, height, active, hovered) else {
        return Ok(());
    };
    let shadow = with_alpha(color(0, 0, 0), metrics.shadow_alpha);
    let soft_shadow = with_alpha(color(0, 0, 0), metrics.shadow_alpha * 0.46);
    let offset = metrics.shadow_offset;
    window.filled_rectangle(
        layers,
        0,
        rect(x + offset * 0.45, y + offset * 0.45, width, height),
        soft_shadow,
    )?;
    window.filled_rectangle(
        layers,
        0,
        rect(x + offset, y + offset, width, height),
        shadow,
    )?;
    Ok(())
}

fn paint_lcars_raised_slab_edges(
    window: &mut crate::TermWindow,
    layers: &mut TripleLayerQuadAllocator,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    fill_byte: LcarsByteColor,
    active: bool,
    hovered: bool,
) -> anyhow::Result<()> {
    let Some(metrics) = lcars_depth_metrics(width, height, active, hovered) else {
        return Ok(());
    };
    let edge = (height * 0.07).clamp(2.0, 4.0).min(height * 0.35);
    let side_edge = (height * 0.05).clamp(1.0, 3.0).min(width * 0.20);
    let highlight = with_alpha(color(255, 255, 255), metrics.top_edge_alpha);
    let fill_highlight = with_alpha(lcars_byte_color(fill_byte), metrics.top_edge_alpha + 0.08);
    let shade = with_alpha(color(0, 0, 0), metrics.bottom_edge_alpha);

    if active {
        window.filled_rectangle(layers, 0, rect(x, y, width, edge), shade)?;
        window.filled_rectangle(layers, 0, rect(x, y, side_edge, height), shade)?;
        window.filled_rectangle(
            layers,
            0,
            rect(x + side_edge, y + height - edge, width - side_edge, edge),
            fill_highlight,
        )?;
        window.filled_rectangle(
            layers,
            0,
            rect(x + width - side_edge, y + edge, side_edge, height - edge),
            fill_highlight,
        )?;
    } else {
        window.filled_rectangle(layers, 0, rect(x, y, width, edge), highlight)?;
        window.filled_rectangle(layers, 0, rect(x, y, side_edge, height), highlight)?;
        window.filled_rectangle(
            layers,
            0,
            rect(x + side_edge, y + height - edge, width - side_edge, edge),
            shade,
        )?;
        window.filled_rectangle(
            layers,
            0,
            rect(x + width - side_edge, y + edge, side_edge, height - edge),
            shade,
        )?;
    }
    Ok(())
}

fn paint_lcars_rect_slab(
    window: &mut crate::TermWindow,
    layers: &mut TripleLayerQuadAllocator,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    fill_byte: LcarsByteColor,
    active: bool,
    hovered: bool,
) -> anyhow::Result<()> {
    if width <= 0.0 || height <= 0.0 {
        return Ok(());
    }
    paint_lcars_cast_shadow(window, layers, x, y, width, height, active, hovered)?;
    window.filled_rectangle(
        layers,
        0,
        rect(x, y, width, height),
        lcars_byte_color(fill_byte),
    )?;
    paint_lcars_raised_slab_edges(
        window, layers, x, y, width, height, fill_byte, active, hovered,
    )
}

fn paint_lcars_recessed_bay_depth(
    window: &mut crate::TermWindow,
    layers: &mut TripleLayerQuadAllocator,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    accent_byte: LcarsByteColor,
) -> anyhow::Result<()> {
    if width <= 0.0 || height <= 0.0 {
        return Ok(());
    }

    let Some(metrics) = lcars_depth_metrics(width, height, false, false) else {
        return Ok(());
    };
    let edge = (height * 0.08).clamp(2.0, 4.0).min(height * 0.35);
    let side_edge = (height * 0.06).clamp(1.0, 3.0).min(width * 0.20);
    let accent = lcars_byte_color(accent_byte);
    window.filled_rectangle(layers, 0, rect(x, y, width, height), color(0, 0, 0))?;
    window.filled_rectangle(
        layers,
        0,
        rect(x, y, width, edge),
        with_alpha(accent, metrics.inner_edge_alpha * 0.55),
    )?;
    window.filled_rectangle(
        layers,
        0,
        rect(x, y, side_edge, height),
        with_alpha(accent, metrics.inner_edge_alpha * 0.40),
    )?;
    window.filled_rectangle(
        layers,
        0,
        rect(x + side_edge, y + height - edge, width - side_edge, edge),
        with_alpha(accent, metrics.inner_edge_alpha + 0.10),
    )?;
    window.filled_rectangle(
        layers,
        0,
        rect(x + width - side_edge, y + edge, side_edge, height - edge),
        with_alpha(accent, metrics.inner_edge_alpha + 0.06),
    )?;
    Ok(())
}

fn thelcars_demo_metadata(document: &InterfaceDocument) -> TheLcarsDemoMetadata {
    TheLcarsDemoMetadata {
        template_status: thelcars_document_property(
            document,
            &["private_template_status", "template_status"],
            "not configured",
        )
        .to_uppercase(),
        themes: thelcars_document_property(
            document,
            &["private_theme_variants", "theme_variants", "themes"],
            "none detected",
        ),
        palette_roles: thelcars_document_property(
            document,
            &["private_palette_roles", "palette_roles"],
            "none detected",
        ),
        frame_metrics: thelcars_document_property(
            document,
            &["private_frame_metrics", "frame_metrics"],
            "none detected",
        ),
        bar_rhythm: thelcars_document_property(
            document,
            &["private_bar_rhythm", "bar_rhythm"],
            "none detected",
        ),
    }
}

fn thelcars_document_property(
    document: &InterfaceDocument,
    keys: &[&str],
    fallback: &str,
) -> String {
    document_property_value(document, keys)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(fallback)
        .to_string()
}

fn thelcars_theme_labels(themes: &str) -> Vec<String> {
    let mut labels = themes
        .split(|ch| ch == ',' || ch == ';' || ch == '|')
        .map(str::trim)
        .filter(|label| !label.is_empty())
        .filter(|label| !label.eq_ignore_ascii_case("none detected"))
        .map(str::to_uppercase)
        .collect::<Vec<_>>();

    if labels.is_empty() {
        labels.push("PRIVATE TEMPLATE OFFLINE".to_string());
    }
    labels
}

fn thelcars_metric_count(value: &str) -> usize {
    let normalized = value.trim();
    if normalized.is_empty() || normalized.eq_ignore_ascii_case("none detected") {
        return 0;
    }
    normalized
        .split(|ch| ch == ';' || ch == '|')
        .map(str::trim)
        .filter(|token| !token.is_empty())
        .filter(|token| !token.eq_ignore_ascii_case("none detected"))
        .count()
}

fn thelcars_adaptive_layout_label(metadata: &TheLcarsDemoMetadata) -> &'static str {
    let themes = metadata.themes.to_uppercase();
    if themes.contains("ULTRA") {
        "ULTRA TO ADAPTIVE"
    } else if themes.contains("PADD") {
        "PADD RESPONSIVE"
    } else {
        "ADAPTIVE"
    }
}

fn thelcars_cockpit_detail_text(metadata: &TheLcarsDemoMetadata) -> String {
    let theme_count = thelcars_theme_labels(&metadata.themes).len().max(1);
    let palette_count = thelcars_metric_count(&metadata.palette_roles);
    let frame_count = thelcars_metric_count(&metadata.frame_metrics);
    let bar_count = thelcars_metric_count(&metadata.bar_rhythm);
    let has_reference_metrics = [
        &metadata.palette_roles,
        &metadata.frame_metrics,
        &metadata.bar_rhythm,
    ]
    .iter()
    .any(|value| !value.eq_ignore_ascii_case("none detected"));
    let readiness = if metadata.template_status.contains("DETECTED")
        || metadata.template_status.contains("READY")
        || metadata.template_status.contains("LIVE")
    {
        "REFERENCE READY"
    } else {
        "ORIGINAL GRID"
    };
    let geometry = if has_reference_metrics {
        "GEOMETRY SAMPLED"
    } else {
        "GEOMETRY NATIVE"
    };
    let layout = thelcars_adaptive_layout_label(metadata);
    format!(
        "NATIVE COCKPIT | THEMES {theme_count:02} | PAL {palette_count:02} | FRAME {frame_count:02} | BAR {bar_count:02} | {readiness} | {geometry} | {layout}"
    )
}

fn thelcars_navigation_labels(items: &[PanelLine]) -> Vec<String> {
    let mut labels = items
        .iter()
        .filter(|line| line.action_id.is_none())
        .filter(|line| {
            matches!(
                line.kind,
                PanelLineKind::Frame
                    | PanelLineKind::SideRail
                    | PanelLineKind::BarRun
                    | PanelLineKind::Elbow
                    | PanelLineKind::ContentBay
                    | PanelLineKind::DataCascade
                    | PanelLineKind::CommandGrid
            )
        })
        .map(|line| line.text.to_uppercase())
        .collect::<Vec<_>>();

    if labels.is_empty() {
        labels = items
            .iter()
            .filter(|line| line.action_id.is_none())
            .take(5)
            .map(|line| line.text.to_uppercase())
            .collect();
    }
    if labels.is_empty() {
        labels.push("LIVE BUILDER".to_string());
        labels.push("STRUCTURE".to_string());
        labels.push("CONTENT BAY".to_string());
    }
    labels
}

fn thelcars_signal_lines(items: &[PanelLine], limit: usize) -> Vec<&PanelLine> {
    let mut selected = items
        .iter()
        .enumerate()
        .filter(|(_, line)| line.action_id.is_none())
        .filter(|(_, line)| !is_thelcars_chrome_only_line(line.kind))
        .collect::<Vec<_>>();
    selected.sort_by_key(|(index, line)| (thelcars_signal_priority(line.kind), *index));
    selected.truncate(limit);
    selected.sort_by_key(|(index, _)| *index);
    selected.into_iter().map(|(_, line)| line).collect()
}

fn is_thelcars_chrome_only_line(kind: PanelLineKind) -> bool {
    matches!(
        kind,
        PanelLineKind::Frame
            | PanelLineKind::SideRail
            | PanelLineKind::Bar
            | PanelLineKind::BarRun
            | PanelLineKind::Elbow
            | PanelLineKind::CommandGrid
    )
}

fn thelcars_signal_priority(kind: PanelLineKind) -> u8 {
    match kind {
        PanelLineKind::Badge | PanelLineKind::Status => 0,
        PanelLineKind::Metric | PanelLineKind::Progress => 1,
        PanelLineKind::Table | PanelLineKind::DataCascade => 2,
        PanelLineKind::ContentBay | PanelLineKind::Section => 3,
        PanelLineKind::List | PanelLineKind::Text => 4,
        _ => 5,
    }
}

fn lcars_builder_highlight_byte(line: &PanelLine, index: usize) -> Option<LcarsByteColor> {
    let highlight = line.builder_highlight.as_ref()?;
    let mode = highlight.mode.as_str();
    Some(
        if mode.contains("ACK") || mode.contains("READY") || mode.contains("COMPLETE") {
            LCARS_BYTE_AMBER
        } else if mode.contains("PULSE") || mode.contains("FOCUS") {
            LCARS_BYTE_CYAN
        } else {
            match index % 3 {
                0 => LCARS_BYTE_PEACH,
                1 => LCARS_BYTE_VIOLET,
                _ => LCARS_BYTE_BLUE,
            }
        },
    )
}

fn paint_lcars_builder_highlight_outline(
    window: &mut crate::TermWindow,
    layers: &mut TripleLayerQuadAllocator,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    line: &PanelLine,
    index: usize,
    cell_width: f32,
) -> anyhow::Result<()> {
    let Some(fill_byte) = lcars_builder_highlight_byte(line, index) else {
        return Ok(());
    };
    let fill = with_alpha(color(fill_byte.red, fill_byte.green, fill_byte.blue), 0.86);
    window.filled_rectangle(
        layers,
        0,
        rect(x - 8.0, y - 6.0, width + 16.0, height + 12.0),
        with_alpha(color(fill_byte.red, fill_byte.green, fill_byte.blue), 0.12),
    )?;
    window.filled_rectangle(layers, 0, rect(x - 4.0, y - 3.0, 5.0, height + 6.0), fill)?;
    window.filled_rectangle(layers, 0, rect(x, y - 3.0, width * 0.58, 3.0), fill)?;
    window.filled_rectangle(
        layers,
        0,
        rect(x + width * 0.42, y + height, width * 0.42, 3.0),
        fill,
    )?;
    if let Some(highlight) = &line.builder_highlight {
        let label = format!("{} {}", highlight.label, highlight.mode);
        let cols = ((width * 0.24) / cell_width).floor().clamp(6.0, 18.0) as usize;
        window.paint_owt_panel_text(
            layers,
            x + width - (cols as f32 * cell_width) - 8.0,
            y + 2.0,
            cols,
            &fit_text_ellipsis(&label, cols),
            RgbColor::new_8bpc(fill_byte.red, fill_byte.green, fill_byte.blue),
            true,
        )?;
    }
    Ok(())
}

fn paint_lcars_thelcars_compact_header(
    window: &mut crate::TermWindow,
    layers: &mut TripleLayerQuadAllocator,
    header: LcarsSceneRect,
    title: &str,
    cell_width: f32,
    cell_height: f32,
    palette: LcarsPalette,
) -> anyhow::Result<()> {
    if header.width <= 0.0 || header.height <= 0.0 {
        return Ok(());
    }

    window.filled_rectangle(
        layers,
        0,
        rect(header.x, header.y, header.width, header.height),
        palette.black,
    )?;
    let cap_width = (header.width * 0.24)
        .clamp(62.0, 132.0)
        .min(header.width * 0.42);
    let lcars_width = (header.width * 0.16).clamp(54.0, 82.0);
    let label_width = (header.width - cap_width - lcars_width - 12.0).max(cell_width * 8.0);
    paint_lcars_rect_slab(
        window,
        layers,
        header.x,
        header.y,
        cap_width,
        header.height,
        LCARS_BYTE_PEACH,
        false,
        false,
    )?;
    paint_lcars_recessed_bay_depth(
        window,
        layers,
        header.x + cap_width + 6.0,
        header.y + 4.0,
        label_width,
        header.height - 8.0,
        LCARS_BYTE_BLUE,
    )?;
    let label_cols = ((label_width - 16.0) / cell_width).floor().max(4.0) as usize;
    window.paint_owt_panel_text(
        layers,
        header.x + cap_width + 14.0,
        header.y + ((header.height - cell_height) * 0.5).max(0.0),
        label_cols,
        &fit_text_ellipsis(title, label_cols),
        RgbColor::new_8bpc(210, 225, 255),
        true,
    )?;
    paint_lcars_recessed_bay_depth(
        window,
        layers,
        header.right() - lcars_width,
        header.y,
        lcars_width,
        header.height,
        LCARS_BYTE_RED,
    )?;
    let lcars_cols = ((lcars_width - 10.0) / cell_width).floor().max(3.0) as usize;
    window.paint_owt_panel_text(
        layers,
        header.right() - lcars_width + 6.0,
        header.y + ((header.height - cell_height) * 0.5).max(0.0),
        lcars_cols,
        &fit_text_ellipsis("LCARS", lcars_cols),
        RgbColor::new_8bpc(229, 64, 67),
        true,
    )
}

fn paint_lcars_thelcars_cockpit_backbone(
    window: &mut crate::TermWindow,
    layers: &mut TripleLayerQuadAllocator,
    grid: &TheLcarsCockpitGrid,
) -> anyhow::Result<()> {
    let rail = grid.rail;
    let header = grid.header;
    let primary = grid.primary_bay;
    let panel = grid.panel;
    let rail_to_primary = (primary.x - rail.right() - 6.0).max(1.0);

    paint_lcars_rect_slab(
        window,
        layers,
        rail.x + rail.width * 0.70,
        header.bottom() - 3.0,
        rail.width * 0.20,
        (rail.height * 0.42).max(12.0),
        LCARS_BYTE_ORANGE,
        false,
        false,
    )?;
    if rail_to_primary > 8.0 {
        paint_lcars_rect_slab(
            window,
            layers,
            rail.right() + 4.0,
            grid.scope_tab.y + grid.scope_tab.height * 0.50,
            rail_to_primary,
            5.0,
            LCARS_BYTE_VIOLET,
            false,
            false,
        )?;
    }
    paint_lcars_rect_slab(
        window,
        layers,
        primary.x,
        primary.y - 9.0,
        primary
            .width
            .min((panel.right() - primary.x - 18.0).max(1.0)),
        5.0,
        LCARS_BYTE_BLUE,
        false,
        false,
    )?;
    if let Some(command) = grid.command_stack {
        let bridge_width = (command.x - primary.right() - 12.0).max(1.0);
        if bridge_width > 10.0 {
            paint_lcars_rect_slab(
                window,
                layers,
                primary.right() + 6.0,
                command.y + 38.0,
                bridge_width,
                5.0,
                LCARS_BYTE_PEACH,
                false,
                false,
            )?;
        }
        paint_lcars_rect_slab(
            window,
            layers,
            command.x,
            command.y - 8.0,
            command.width * 0.72,
            8.0,
            LCARS_BYTE_VIOLET,
            false,
            false,
        )?;
    }
    if matches!(grid.density, TheLcarsCockpitDensity::Strip) {
        paint_lcars_rect_slab(
            window,
            layers,
            panel.x + 12.0,
            panel.bottom() - 8.0,
            panel.width * 0.30,
            5.0,
            LCARS_BYTE_BLUE,
            false,
            false,
        )?;
    }
    Ok(())
}

fn paint_lcars_thelcars_compact_rail(
    window: &mut crate::TermWindow,
    layers: &mut TripleLayerQuadAllocator,
    rail: LcarsSceneRect,
    items: &[PanelLine],
    palette: LcarsPalette,
) -> anyhow::Result<()> {
    if rail.width <= 0.0 || rail.height <= 0.0 {
        return Ok(());
    }

    window.filled_rectangle(
        layers,
        0,
        rect(rail.x, rail.y, rail.width, rail.height),
        palette.black,
    )?;
    let labels = thelcars_navigation_labels(items);
    let segments = labels.len().clamp(3, 6);
    let gap = 4.0;
    let segment_h = ((rail.height - gap * (segments.saturating_sub(1) as f32)) / segments as f32)
        .clamp(16.0, 32.0);
    for index in 0..segments {
        let y = rail.y + index as f32 * (segment_h + gap);
        if y + segment_h > rail.bottom() {
            break;
        }
        let fill = match index % 4 {
            0 => LCARS_BYTE_VIOLET,
            1 => LCARS_BYTE_BLUE,
            2 => LCARS_BYTE_ORANGE,
            _ => LCARS_BYTE_PEACH,
        };
        paint_lcars_rect_slab(
            window, layers, rail.x, y, rail.width, segment_h, fill, false, false,
        )?;
    }
    Ok(())
}

fn paint_lcars_thelcars_workspace_frame(
    window: &mut crate::TermWindow,
    layers: &mut TripleLayerQuadAllocator,
    workspace: LcarsSceneRect,
    cell_width: f32,
    palette: LcarsPalette,
) -> anyhow::Result<()> {
    if workspace.width < 80.0 || workspace.height < 36.0 {
        return Ok(());
    }

    paint_lcars_recessed_bay_depth(
        window,
        layers,
        workspace.x,
        workspace.y,
        workspace.width,
        workspace.height,
        LCARS_BYTE_BLUE,
    )?;
    paint_lcars_rect_slab(
        window,
        layers,
        workspace.x,
        workspace.y,
        workspace.width * 0.26,
        5.0,
        LCARS_BYTE_BLUE,
        false,
        false,
    )?;
    paint_lcars_rect_slab(
        window,
        layers,
        workspace.x + workspace.width * 0.34,
        workspace.y,
        workspace.width * 0.18,
        5.0,
        LCARS_BYTE_VIOLET,
        false,
        false,
    )?;
    let label_cols = ((workspace.width * 0.24) / cell_width).floor().max(4.0) as usize;
    window.paint_owt_panel_text(
        layers,
        workspace.x + workspace.width - (label_cols as f32 * cell_width) - 10.0,
        workspace.y + 8.0,
        label_cols,
        &fit_text_ellipsis("TERMINAL WORKSPACE", label_cols),
        RgbColor::new_8bpc(145, 158, 255),
        true,
    )?;
    window.filled_rectangle(
        layers,
        0,
        rect(
            workspace.x + 8.0,
            workspace.bottom() - 7.0,
            workspace.width * 0.18,
            4.0,
        ),
        palette.violet,
    )?;
    Ok(())
}

fn paint_lcars_thelcars_header(
    window: &mut crate::TermWindow,
    layers: &mut TripleLayerQuadAllocator,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    rail_width: f32,
    title: &str,
    palette: LcarsPalette,
) -> anyhow::Result<()> {
    if width <= 0.0 || height <= 0.0 {
        return Ok(());
    }

    let cell_width = window.render_metrics.cell_size.width as f32;
    let cell_height = window.render_metrics.cell_size.height as f32;
    let peach_soft = LcarsByteColor::rgb(255, 171, 154);
    let left_cap_max = (width * 0.25).max(80.0);
    let left_cap_width = rail_width.min(left_cap_max).max(left_cap_max.min(120.0));
    let toolbar_left = x + left_cap_width + 10.0;
    let toolbar_height = (height * 0.48).clamp(24.0, 34.0);
    let lower_y = y + toolbar_height + 8.0;
    let lower_height = (height - toolbar_height - 10.0).max(12.0);

    window.filled_rectangle(layers, 0, rect(x, y, width, height), palette.black)?;
    paint_lcars_cast_shadow(window, layers, x, y, left_cap_width, height, false, false)?;
    paint_lcars_generated_bitmap(window, layers, x, y, left_cap_width, height, |raster| {
        raster.fill_rounded_rect(
            0.0,
            0.0,
            left_cap_width,
            height,
            (height * 0.52).min(left_cap_width * 0.45),
            peach_soft,
        );
        raster.fill_rect(
            0.0,
            height * 0.62,
            left_cap_width,
            height * 0.38,
            LCARS_BYTE_BLACK,
        );
    })?;
    paint_lcars_raised_slab_edges(
        window,
        layers,
        x,
        y,
        left_cap_width,
        height,
        peach_soft,
        false,
        false,
    )?;
    window.filled_rectangle(
        layers,
        0,
        rect(x + 12.0, y + height - 7.0, left_cap_width * 0.45, 5.0),
        palette.red,
    )?;

    let cap_cols = ((left_cap_width - 36.0) / cell_width).floor().max(3.0) as usize;
    window.paint_owt_panel_text(
        layers,
        x + 36.0,
        y + ((toolbar_height - cell_height) * 0.5).max(0.0),
        cap_cols,
        &fit_text_ellipsis(title, cap_cols),
        RgbColor::new_8bpc(0, 0, 0),
        true,
    )?;

    let toolbar_min = cell_width * 18.0;
    let toolbar_max = (width - left_cap_width - 130.0).max(toolbar_min);
    let toolbar_width = (width * 0.46).max(toolbar_min).min(toolbar_max);
    paint_lcars_rect_slab(
        window,
        layers,
        toolbar_left,
        y + 4.0,
        toolbar_width,
        toolbar_height,
        LCARS_BYTE_RED,
        false,
        false,
    )?;
    let tool_labels = ["<", "HOME", "EDIT", "NEXT"];
    let mut tool_x = toolbar_left + 12.0;
    for label in tool_labels {
        let label_cols = label.chars().count().max(1);
        window.paint_owt_panel_text(
            layers,
            tool_x,
            y + 8.0,
            label_cols,
            label,
            RgbColor::new_8bpc(255, 255, 255),
            true,
        )?;
        tool_x += cell_width * (label.chars().count() as f32 + 2.0);
    }

    let black_strip_left = toolbar_left + toolbar_width + 8.0;
    let black_strip_width = (x + width - black_strip_left - 94.0).max(cell_width * 12.0);
    paint_lcars_recessed_bay_depth(
        window,
        layers,
        black_strip_left,
        y + 4.0,
        black_strip_width,
        toolbar_height,
        LCARS_BYTE_BLUE,
    )?;
    for (index, label) in [
        "SECTIONS",
        "GRID",
        "BASIC CARDS",
        "ADV CARDS",
        "STACKS",
        "PANEL",
    ]
    .iter()
    .enumerate()
    {
        let label_x = black_strip_left + 10.0 + index as f32 * cell_width * 8.0;
        if label_x + cell_width * 5.0 >= black_strip_left + black_strip_width {
            break;
        }
        let cols = label.chars().count().min(9);
        window.paint_owt_panel_text(
            layers,
            label_x,
            y + 8.0,
            cols,
            &fit_text_ellipsis(label, cols),
            RgbColor::new_8bpc(210, 210, 210),
            true,
        )?;
    }

    let lcars_label_width = (cell_width * 10.0).clamp(72.0, 104.0);
    paint_lcars_recessed_bay_depth(
        window,
        layers,
        x + width - lcars_label_width,
        y,
        lcars_label_width,
        toolbar_height + 8.0,
        LCARS_BYTE_RED,
    )?;
    window.paint_owt_panel_text(
        layers,
        x + width - lcars_label_width + 6.0,
        y + 5.0,
        8,
        "LCARS",
        RgbColor::new_8bpc(229, 64, 67),
        true,
    )?;

    let bands = [
        (
            x,
            lower_y,
            rail_width * 0.23,
            lower_height,
            LCARS_BYTE_PEACH,
        ),
        (
            x + rail_width * 0.28,
            lower_y,
            rail_width * 0.16,
            lower_height,
            LCARS_BYTE_VIOLET,
        ),
        (
            x + rail_width * 0.48,
            lower_y,
            rail_width * 0.38,
            lower_height,
            LCARS_BYTE_BLUE,
        ),
        (
            x + rail_width + 22.0,
            lower_y,
            width * 0.38,
            lower_height * 0.46,
            LCARS_BYTE_BLUE,
        ),
        (
            x + rail_width + width * 0.40,
            lower_y,
            width * 0.12,
            lower_height * 0.46,
            LCARS_BYTE_PEACH,
        ),
        (
            x + width * 0.62,
            lower_y,
            width * 0.18,
            lower_height * 0.46,
            LCARS_BYTE_VIOLET,
        ),
        (
            x + width * 0.82,
            lower_y,
            width * 0.15,
            lower_height * 0.46,
            LCARS_BYTE_BLUE,
        ),
    ];
    for (bar_x, bar_y, bar_width, bar_height, fill_byte) in bands {
        if bar_x < x + width && bar_width > 2.0 {
            paint_lcars_rect_slab(
                window, layers, bar_x, bar_y, bar_width, bar_height, fill_byte, false, false,
            )?;
        }
    }

    Ok(())
}

fn paint_lcars_thelcars_theme_strip(
    window: &mut crate::TermWindow,
    layers: &mut TripleLayerQuadAllocator,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    metadata: &TheLcarsDemoMetadata,
    cell_width: f32,
    cell_height: f32,
    palette: LcarsPalette,
) -> anyhow::Result<()> {
    if width <= 0.0 || height <= 0.0 {
        return Ok(());
    }

    window.filled_rectangle(layers, 0, rect(x, y, width, height), palette.black)?;

    let label_width = (width * 0.23).clamp(118.0, 184.0).min(width * 0.44);
    let label_fill_byte = if metadata.template_status == "DETECTED" {
        LCARS_BYTE_AMBER
    } else {
        LCARS_BYTE_RED
    };
    paint_lcars_cast_shadow(window, layers, x, y, label_width, height, false, false)?;
    paint_lcars_left_cap_bar(window, layers, x, y, label_width, height, label_fill_byte)?;
    paint_lcars_raised_slab_edges(
        window,
        layers,
        x,
        y,
        label_width,
        height,
        label_fill_byte,
        false,
        false,
    )?;
    let label_cols = ((label_width - 18.0) / cell_width).floor().max(4.0) as usize;
    window.paint_owt_panel_text(
        layers,
        x + 12.0,
        y + ((height - cell_height) * 0.5).max(0.0),
        label_cols,
        &fit_text_ellipsis("INCLUDED THEMES", label_cols),
        RgbColor::new_8bpc(0, 0, 0),
        true,
    )?;

    let labels = thelcars_theme_labels(&metadata.themes);
    let visible = labels.len().min(4).max(1);
    let gap = 6.0;
    let detail_width = (width * 0.28).clamp(120.0, 230.0).min(width * 0.35);
    let chips_left = x + label_width + gap;
    let chips_right = (x + width - detail_width - gap).max(chips_left + 70.0);
    let chips_width = (chips_right - chips_left).max(70.0);
    let chip_width = ((chips_width - gap * (visible.saturating_sub(1)) as f32) / visible as f32)
        .clamp(58.0, 138.0);
    for (index, label) in labels.iter().take(visible).enumerate() {
        let chip_x = chips_left + index as f32 * (chip_width + gap);
        if chip_x + chip_width > x + width - detail_width - gap {
            break;
        }
        let fill = match index % 4 {
            0 => LCARS_BYTE_ORANGE,
            1 => LCARS_BYTE_BLUE,
            2 => LCARS_BYTE_VIOLET,
            _ => LCARS_BYTE_PEACH,
        };
        paint_lcars_cast_shadow(window, layers, chip_x, y, chip_width, height, false, false)?;
        paint_lcars_right_cap_bar(window, layers, chip_x, y, chip_width, height, fill)?;
        paint_lcars_raised_slab_edges(
            window, layers, chip_x, y, chip_width, height, fill, false, false,
        )?;
        let cols = ((chip_width - 16.0) / cell_width).floor().max(3.0) as usize;
        window.paint_owt_panel_text(
            layers,
            chip_x + 8.0,
            y + ((height - cell_height) * 0.5).max(0.0),
            cols,
            &fit_text_ellipsis(label, cols),
            RgbColor::new_8bpc(0, 0, 0),
            true,
        )?;
    }

    let detail_x = (x + width - detail_width).max(chips_left + 72.0);
    paint_lcars_recessed_bay_depth(
        window,
        layers,
        detail_x,
        y,
        detail_width,
        height,
        LCARS_BYTE_BLUE,
    )?;
    window.filled_rectangle(
        layers,
        0,
        rect(detail_x, y, detail_width * 0.34, 4.0),
        palette.blue,
    )?;
    window.filled_rectangle(
        layers,
        0,
        rect(detail_x + detail_width * 0.38, y, detail_width * 0.22, 4.0),
        palette.violet,
    )?;
    window.filled_rectangle(
        layers,
        0,
        rect(detail_x + detail_width * 0.66, y, detail_width * 0.24, 4.0),
        palette.peach,
    )?;
    let detail = thelcars_cockpit_detail_text(metadata);
    let detail_cols = ((detail_width - 10.0) / cell_width).floor().max(4.0) as usize;
    window.paint_owt_panel_text(
        layers,
        detail_x + 6.0,
        y + ((height - cell_height) * 0.5).max(0.0),
        detail_cols,
        &fit_text_ellipsis(&detail, detail_cols),
        RgbColor::new_8bpc(153, 204, 255),
        true,
    )?;

    Ok(())
}

fn paint_lcars_thelcars_nav_rail(
    window: &mut crate::TermWindow,
    layers: &mut TripleLayerQuadAllocator,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    items: &[PanelLine],
    palette: LcarsPalette,
) -> anyhow::Result<()> {
    if width <= 0.0 || height <= 0.0 {
        return Ok(());
    }

    let cell_width = window.render_metrics.cell_size.width as f32;
    let cell_height = window.render_metrics.cell_size.height as f32;
    let icon_width = (width * 0.24).clamp(28.0, 42.0);
    let row_gap = 3.0;
    let row_height = ((height - row_gap * 12.0) / 12.0).clamp(23.0, 34.0);
    let label_cols = ((width - icon_width - 12.0) / cell_width).floor().max(4.0) as usize;
    let labels = thelcars_navigation_labels(items);
    let mut rows = labels
        .iter()
        .take(8)
        .enumerate()
        .map(|(index, label)| {
            let fill = match index % 4 {
                0 => LCARS_BYTE_VIOLET,
                1 => LCARS_BYTE_BLUE,
                2 => LCARS_BYTE_ORANGE,
                _ => LCARS_BYTE_PEACH,
            };
            (label.as_str(), fill, index == 0)
        })
        .collect::<Vec<_>>();
    if rows.len() < 3 {
        rows.push(("CONTENT BAY", LCARS_BYTE_BLUE, rows.is_empty()));
        rows.push(("COMMAND STACK", LCARS_BYTE_VIOLET, false));
    }

    window.filled_rectangle(layers, 0, rect(x, y, width, height), palette.black)?;
    for (index, (label, fill_byte, active)) in rows.iter().enumerate() {
        let row_y = y + index as f32 * (row_height + row_gap);
        paint_lcars_rect_slab(
            window, layers, x, row_y, icon_width, row_height, *fill_byte, false, false,
        )?;
        if *active {
            paint_lcars_recessed_bay_depth(
                window,
                layers,
                x + icon_width + 4.0,
                row_y,
                width - icon_width - 4.0,
                row_height,
                *fill_byte,
            )?;
            window.filled_rectangle(
                layers,
                0,
                rect(x + icon_width + 4.0, row_y, width - icon_width - 4.0, 3.0),
                palette.red,
            )?;
            window.filled_rectangle(
                layers,
                0,
                rect(
                    x + icon_width + 4.0,
                    row_y + row_height - 3.0,
                    width - icon_width - 4.0,
                    3.0,
                ),
                palette.red,
            )?;
        } else {
            paint_lcars_rect_slab(
                window,
                layers,
                x + icon_width + 4.0,
                row_y,
                width - icon_width - 4.0,
                row_height,
                if index % 3 == 2 {
                    LCARS_BYTE_BLUE
                } else {
                    LCARS_BYTE_VIOLET
                },
                false,
                false,
            )?;
        }

        let icon = if *active { "*" } else { "+" };
        window.paint_owt_panel_text(
            layers,
            x + (icon_width - cell_width) * 0.5,
            row_y + ((row_height - cell_height) * 0.5).max(0.0),
            1,
            icon,
            if *active {
                RgbColor::new_8bpc(229, 64, 67)
            } else {
                RgbColor::new_8bpc(60, 62, 180)
            },
            true,
        )?;
        window.paint_owt_panel_text(
            layers,
            x + icon_width + 10.0,
            row_y + ((row_height - cell_height) * 0.5).max(0.0),
            label_cols,
            &fit_text_ellipsis(label, label_cols),
            if *active {
                RgbColor::new_8bpc(229, 64, 67)
            } else {
                RgbColor::new_8bpc(0, 0, 0)
            },
            true,
        )?;
    }

    let footer_rows = [
        ("SETTINGS", LCARS_BYTE_VIOLET),
        ("NOTIFICATIONS", LCARS_BYTE_VIOLET),
        ("CHIEF ENGINEER", LCARS_BYTE_BLUE),
    ];
    let footer_top = y + height - (row_height + row_gap) * footer_rows.len() as f32;
    for (index, (label, fill_byte)) in footer_rows.iter().enumerate() {
        let row_y = footer_top + index as f32 * (row_height + row_gap);
        paint_lcars_rect_slab(
            window, layers, x, row_y, icon_width, row_height, *fill_byte, false, false,
        )?;
        paint_lcars_rect_slab(
            window,
            layers,
            x + icon_width + 4.0,
            row_y,
            width - icon_width - 4.0,
            row_height,
            *fill_byte,
            false,
            false,
        )?;
        window.paint_owt_panel_text(
            layers,
            x + icon_width + 10.0,
            row_y + ((row_height - cell_height) * 0.5).max(0.0),
            label_cols,
            &fit_text_ellipsis(label, label_cols),
            RgbColor::new_8bpc(0, 0, 0),
            true,
        )?;
    }

    Ok(())
}

fn paint_lcars_thelcars_panel_title(
    window: &mut crate::TermWindow,
    layers: &mut TripleLayerQuadAllocator,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    label: &str,
    fill_byte: LcarsByteColor,
    _palette: LcarsPalette,
) -> anyhow::Result<()> {
    let cell_width = window.render_metrics.cell_size.width as f32;
    paint_lcars_cast_shadow(window, layers, x, y, width, height, false, false)?;
    paint_lcars_right_cap_bar(window, layers, x, y, width, height, fill_byte)?;
    paint_lcars_raised_slab_edges(window, layers, x, y, width, height, fill_byte, false, false)?;
    paint_lcars_recessed_bay_depth(
        window,
        layers,
        x + 14.0,
        y + 5.0,
        width - 28.0,
        height - 10.0,
        fill_byte,
    )?;
    let cols = ((width - 44.0) / cell_width).floor().max(4.0) as usize;
    window.paint_owt_panel_text(
        layers,
        x + 22.0,
        y + 7.0,
        cols,
        &fit_text_ellipsis(label, cols),
        RgbColor::new_8bpc(230, 230, 230),
        true,
    )
}

fn paint_lcars_thelcars_signal_bay(
    window: &mut crate::TermWindow,
    layers: &mut TripleLayerQuadAllocator,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    title: &str,
    rows: &[&PanelLine],
    cell_width: f32,
    cell_height: f32,
    palette: LcarsPalette,
) -> anyhow::Result<()> {
    if width <= 0.0 || height <= 0.0 {
        return Ok(());
    }

    let title_h = (cell_height + 11.0).clamp(27.0, 36.0);
    let side_slab_width = (width * 0.12).clamp(30.0, 72.0).min(width * 0.22);
    let content_width = (width - side_slab_width - 10.0).max(cell_width * 12.0);
    window.filled_rectangle(layers, 0, rect(x, y, width, height), palette.black)?;
    paint_lcars_thelcars_panel_title(
        window,
        layers,
        x,
        y,
        content_width,
        title_h,
        title,
        LCARS_BYTE_BLUE,
        palette,
    )?;

    let slab_x = x + width - side_slab_width;
    let slab_top = y + title_h + 8.0;
    let slab_h = (height - title_h - 16.0).max(1.0);
    paint_lcars_rect_slab(
        window,
        layers,
        slab_x,
        slab_top,
        side_slab_width,
        slab_h * 0.52,
        LCARS_BYTE_RED,
        false,
        false,
    )?;
    paint_lcars_rect_slab(
        window,
        layers,
        slab_x,
        slab_top + slab_h * 0.58,
        side_slab_width,
        slab_h * 0.24,
        LCARS_BYTE_PEACH,
        false,
        false,
    )?;
    paint_lcars_rect_slab(
        window,
        layers,
        slab_x,
        slab_top + slab_h * 0.87,
        side_slab_width,
        slab_h * 0.13,
        LCARS_BYTE_VIOLET,
        false,
        false,
    )?;

    let row_top = y + title_h + 16.0;
    let row_h = (cell_height * 1.54).clamp(28.0, 40.0);
    let row_gap = 8.0;
    let row_width = (content_width - 8.0).max(1.0);
    let max_rows = ((height - title_h - 28.0) / (row_h + row_gap))
        .floor()
        .max(1.0) as usize;
    let visible_rows = rows.iter().copied().take(max_rows).collect::<Vec<_>>();

    if visible_rows.is_empty() {
        let fill = LCARS_BYTE_RED;
        paint_lcars_cast_shadow(window, layers, x, row_top, row_width, row_h, false, false)?;
        paint_lcars_left_cap_bar(window, layers, x, row_top, row_width, row_h, fill)?;
        paint_lcars_raised_slab_edges(
            window, layers, x, row_top, row_width, row_h, fill, false, false,
        )?;
        paint_lcars_recessed_bay_depth(
            window,
            layers,
            x + 18.0,
            row_top + 5.0,
            row_width - 36.0,
            row_h - 10.0,
            fill,
        )?;
        let cols = ((row_width - 44.0) / cell_width).floor().max(4.0) as usize;
        window.paint_owt_panel_text(
            layers,
            x + 26.0,
            row_top + ((row_h - cell_height) * 0.5).max(0.0),
            cols,
            "AWAITING SEMANTIC SIGNALS",
            RgbColor::new_8bpc(255, 149, 96),
            true,
        )?;
        return Ok(());
    }

    for (index, line) in visible_rows.iter().enumerate() {
        let row_y = row_top + index as f32 * (row_h + row_gap);
        let fill_byte = match line.kind {
            PanelLineKind::Badge | PanelLineKind::Status => LCARS_BYTE_AMBER,
            PanelLineKind::Metric | PanelLineKind::Progress => LCARS_BYTE_BLUE,
            PanelLineKind::Table | PanelLineKind::DataCascade => LCARS_BYTE_VIOLET,
            PanelLineKind::ContentBay | PanelLineKind::Section => LCARS_BYTE_PEACH,
            _ => match index % 4 {
                0 => LCARS_BYTE_BLUE,
                1 => LCARS_BYTE_VIOLET,
                2 => LCARS_BYTE_ORANGE,
                _ => LCARS_BYTE_PEACH,
            },
        };
        paint_lcars_cast_shadow(window, layers, x, row_y, row_width, row_h, false, false)?;
        paint_lcars_left_cap_bar(window, layers, x, row_y, row_width, row_h, fill_byte)?;
        paint_lcars_raised_slab_edges(
            window, layers, x, row_y, row_width, row_h, fill_byte, false, false,
        )?;
        paint_lcars_recessed_bay_depth(
            window,
            layers,
            x + 18.0,
            row_y + 5.0,
            row_width - 36.0,
            row_h - 10.0,
            fill_byte,
        )?;
        let marker_w = (row_width * 0.16).clamp(42.0, 86.0);
        window.filled_rectangle(
            layers,
            0,
            rect(x + row_width - marker_w - 14.0, row_y + 8.0, marker_w, 3.0),
            color(fill_byte.red, fill_byte.green, fill_byte.blue),
        )?;
        window.filled_rectangle(
            layers,
            0,
            rect(
                x + row_width - marker_w * 0.74 - 14.0,
                row_y + row_h - 11.0,
                marker_w * 0.74,
                3.0,
            ),
            with_alpha(color(fill_byte.red, fill_byte.green, fill_byte.blue), 0.76),
        )?;
        paint_lcars_builder_highlight_outline(
            window,
            layers,
            x + 16.0,
            row_y + 5.0,
            row_width - 32.0,
            row_h - 10.0,
            line,
            index,
            cell_width,
        )?;
        if line.kind == PanelLineKind::Progress {
            paint_progress_rail(
                window,
                layers,
                x + row_width - marker_w - 18.0,
                row_y + row_h * 0.50,
                marker_w,
                5.0,
                line.progress,
                palette.dim_blue,
                palette.amber,
            )?;
        }
        let cols = ((row_width - marker_w - 42.0) / cell_width)
            .floor()
            .max(4.0) as usize;
        window.paint_owt_panel_text(
            layers,
            x + 26.0,
            row_y + ((row_h - cell_height) * 0.5).max(0.0),
            cols,
            &fit_text_ellipsis(&line.text.to_uppercase(), cols),
            lcars_signal_text_color(line.kind, index),
            true,
        )?;
    }

    let hidden = rows.len().saturating_sub(visible_rows.len());
    if hidden > 0 {
        let footer_y = y + height - title_h;
        paint_lcars_right_cap_bar(
            window,
            layers,
            x + width * 0.42,
            footer_y,
            width * 0.48,
            title_h,
            LCARS_BYTE_ORANGE,
        )?;
        let text = format!("+{hidden} SIGNALS");
        let cols = ((width * 0.34) / cell_width).floor().max(4.0) as usize;
        window.paint_owt_panel_text(
            layers,
            x + width * 0.48,
            footer_y + 7.0,
            cols,
            &fit_text_ellipsis(&text, cols),
            RgbColor::new_8bpc(0, 0, 0),
            true,
        )?;
    }

    Ok(())
}

fn paint_lcars_thelcars_mini_chart(
    window: &mut crate::TermWindow,
    layers: &mut TripleLayerQuadAllocator,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    color_byte: LcarsByteColor,
    _palette: LcarsPalette,
) -> anyhow::Result<()> {
    if width <= 0.0 || height <= 0.0 {
        return Ok(());
    }

    let grid = color(54, 58, 70);
    paint_lcars_recessed_bay_depth(window, layers, x, y, width, height, color_byte)?;
    for col in 0..8 {
        let gx = x + col as f32 * width / 8.0;
        window.filled_rectangle(layers, 0, rect(gx, y, 1.0, height), grid)?;
    }
    for row in 0..4 {
        let gy = y + row as f32 * height / 4.0;
        window.filled_rectangle(layers, 0, rect(x, gy, width, 1.0), grid)?;
    }
    let fill = color(color_byte.red, color_byte.green, color_byte.blue);
    for index in 0..18 {
        let phase = ((index * 7) % 13) as f32 / 13.0;
        let bar_h = (height * (0.12 + phase * 0.68)).clamp(4.0, height - 4.0);
        let bar_w = (width / 27.0).clamp(2.0, 6.0);
        let bx = x + width * 0.48 + index as f32 * (bar_w + 2.0);
        if bx + bar_w > x + width - 2.0 {
            break;
        }
        window.filled_rectangle(layers, 0, rect(bx, y + height - bar_h, bar_w, bar_h), fill)?;
    }
    Ok(())
}

fn paint_lcars_thelcars_chart_strip(
    window: &mut crate::TermWindow,
    layers: &mut TripleLayerQuadAllocator,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    metadata: &TheLcarsDemoMetadata,
    cell_width: f32,
    cell_height: f32,
    palette: LcarsPalette,
) -> anyhow::Result<()> {
    if width <= 0.0 || height <= 0.0 {
        return Ok(());
    }

    let gap = 12.0;
    let chart_width = (width - gap * 2.0) / 3.0;
    let charts = [
        (
            LCARS_BYTE_ORANGE,
            format!("PAL {:02}", thelcars_metric_count(&metadata.palette_roles)),
        ),
        (
            LCARS_BYTE_RED,
            format!(
                "FRAME {:02}",
                thelcars_metric_count(&metadata.frame_metrics)
            ),
        ),
        (
            LCARS_BYTE_BLUE,
            format!("BAR {:02}", thelcars_metric_count(&metadata.bar_rhythm)),
        ),
    ];
    for (index, (color_byte, label)) in charts.iter().enumerate() {
        let chart_x = x + index as f32 * (chart_width + gap);
        paint_lcars_thelcars_mini_chart(
            window,
            layers,
            chart_x,
            y,
            chart_width,
            height,
            *color_byte,
            palette,
        )?;
        let label_cols = ((chart_width - 18.0) / cell_width).floor().max(4.0) as usize;
        window.paint_owt_panel_text(
            layers,
            chart_x + 10.0,
            y + 8.0,
            label_cols,
            &fit_text_ellipsis(label, label_cols),
            RgbColor::new_8bpc(color_byte.red, color_byte.green, color_byte.blue),
            true,
        )?;
        let layout = thelcars_adaptive_layout_label(metadata);
        let layout_cols = ((chart_width - 18.0) / cell_width).floor().max(4.0) as usize;
        if index == 2 && height > cell_height * 2.4 {
            window.paint_owt_panel_text(
                layers,
                chart_x + 10.0,
                y + height - cell_height - 8.0,
                layout_cols,
                &fit_text_ellipsis(layout, layout_cols),
                RgbColor::new_8bpc(153, 204, 255),
                true,
            )?;
        }
    }
    Ok(())
}

fn paint_lcars_thelcars_command_stack(
    window: &mut crate::TermWindow,
    layers: &mut TripleLayerQuadAllocator,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    actions: &[&PanelLine],
    interface_id: &str,
    last_action_id: Option<&str>,
    cell_width: f32,
    cell_height: f32,
    palette: LcarsPalette,
) -> anyhow::Result<()> {
    if width <= 0.0 || height <= 0.0 {
        return Ok(());
    }

    let title_h = (cell_height + 9.0).clamp(26.0, 34.0);
    window.filled_rectangle(layers, 0, rect(x, y, width, height), palette.black)?;
    paint_lcars_thelcars_panel_title(
        window,
        layers,
        x,
        y,
        width,
        title_h,
        "COMMAND STACK",
        LCARS_BYTE_VIOLET,
        palette,
    )?;
    paint_lcars_rect_slab(
        window,
        layers,
        x,
        y + title_h + 7.0,
        width * 0.82,
        5.0,
        LCARS_BYTE_VIOLET,
        false,
        false,
    )?;
    paint_lcars_rect_slab(
        window,
        layers,
        x + width * 0.72,
        y + title_h + 15.0,
        width * 0.22,
        4.0,
        LCARS_BYTE_PEACH,
        false,
        false,
    )?;

    let button_h = (cell_height * 1.62).clamp(32.0, 44.0);
    let button_gap = 10.0;
    let mut button_y = y + title_h + 16.0;
    let text_cols = ((width - 74.0) / cell_width).floor().max(4.0) as usize;
    let bottom_guard = y + height - title_h - 18.0;
    let mut painted = 0usize;
    for (index, line) in actions.iter().copied().take(5).enumerate() {
        if button_y + button_h > bottom_guard {
            break;
        }
        let active = last_action_id == line.action_id.as_deref();
        paint_lcars_action_button(
            window,
            layers,
            x,
            button_y,
            width,
            button_h,
            text_cols,
            &line.text,
            lcars_action_text_color(index),
            palette.action_fill(index),
            lcars_action_fill_byte(index),
            active,
            line.hotkey,
            palette,
        )?;
        if let Some(action_id) = &line.action_id {
            window.ui_items.push(UIItem {
                x: x.max(0.0) as usize,
                y: button_y.max(0.0) as usize,
                width: width.ceil().max(1.0) as usize,
                height: button_h.ceil().max(1.0) as usize,
                item_type: UIItemType::OwtLcarsAction {
                    interface_id: interface_id.to_string(),
                    action_id: action_id.clone(),
                },
            });
        }
        painted += 1;
        button_y += button_h + button_gap;
    }

    if painted == 0 {
        paint_lcars_rect_slab(
            window,
            layers,
            x + 22.0,
            button_y,
            width - 44.0,
            button_h,
            LCARS_BYTE_RED,
            false,
            false,
        )?;
        window.paint_owt_panel_text(
            layers,
            x + 34.0,
            button_y + 9.0,
            text_cols,
            "NO ACTIONS",
            RgbColor::new_8bpc(0, 0, 0),
            true,
        )?;
    }

    let bridge_y = (button_y + 4.0).min(y + height - title_h - 14.0);
    if bridge_y > y + title_h + 20.0 {
        paint_lcars_rect_slab(
            window,
            layers,
            x + 18.0,
            bridge_y,
            width * 0.38,
            4.0,
            LCARS_BYTE_BLUE,
            false,
            false,
        )?;
        paint_lcars_rect_slab(
            window,
            layers,
            x + width * 0.48,
            bridge_y + 7.0,
            width * 0.34,
            4.0,
            LCARS_BYTE_AMBER,
            false,
            false,
        )?;
    }

    let key_min = (cell_width * 12.0).min((width - 18.0).max(1.0));
    let key_width = (width * 0.64).max(key_min).min((width - 18.0).max(1.0));
    let key_x = x + width - key_width;
    let key_y = y + height - title_h;
    paint_lcars_thelcars_panel_title(
        window,
        layers,
        key_x,
        key_y,
        key_width,
        title_h,
        "KEY READY",
        LCARS_BYTE_BLUE,
        palette,
    )?;
    Ok(())
}

fn paint_lcars_primitive_legend(
    window: &mut crate::TermWindow,
    layers: &mut TripleLayerQuadAllocator,
    scene: &LcarsStructuralScene,
    document: &InterfaceDocument,
    cell_width: f32,
    cell_height: f32,
    palette: LcarsPalette,
) -> anyhow::Result<()> {
    let rows = lcars_primitive_legend_rows(document);
    let columns = if scene.plan.signal_width >= cell_width * 34.0 {
        2
    } else {
        1
    };
    let rows_per_column = (rows.len() + columns - 1) / columns;
    let column_gap = 14.0;
    let area_x = scene.signal_field.x;
    let area_y = scene.signal_field.y;
    let area_width =
        (scene.plan.signal_width - column_gap * (columns.saturating_sub(1) as f32)).max(1.0);
    let column_width = area_width / columns as f32;
    let area_bottom = (scene.content_bay.y - 12.0).max(area_y + cell_height * 4.0);
    let area_height = (area_bottom - area_y).max(cell_height * 4.0);
    let row_step =
        (area_height / rows_per_column.max(1) as f32).clamp(cell_height * 0.88, cell_height * 1.20);
    let row_height = (row_step - 4.0)
        .max(cell_height * 0.72)
        .min(cell_height * 1.08);

    for (index, row) in rows.iter().enumerate() {
        let column = index / rows_per_column;
        let row_index = index % rows_per_column;
        let row_x = area_x + (column as f32 * (column_width + column_gap));
        let row_y = area_y + (row_index as f32 * row_step);
        let fill_byte = lcars_primitive_legend_fill_byte(row);
        let fill = if row.present {
            palette.signal_fill(index)
        } else {
            palette.red
        };
        let tab_width = (cell_width * 4.3)
            .clamp(34.0, 50.0)
            .min(column_width * 0.24);
        let cap_width = (row_height * 0.78).clamp(14.0, 22.0);
        let bay_x = row_x + tab_width + 7.0;
        let bay_width = (column_width - tab_width - cap_width - 12.0).max(cell_width * 8.0);
        let text_cols = (bay_width / cell_width).floor().max(4.0) as usize;

        window.filled_rectangle(
            layers,
            0,
            rect(row_x, row_y, column_width, row_height),
            palette.black,
        )?;
        paint_lcars_left_cap_bar(
            window, layers, row_x, row_y, tab_width, row_height, fill_byte,
        )?;
        window.filled_rectangle(
            layers,
            0,
            rect(row_x + tab_width - 3.0, row_y, 8.0, row_height),
            palette.black,
        )?;
        window.filled_rectangle(
            layers,
            0,
            rect(bay_x, row_y + 3.0, bay_width, row_height - 6.0),
            palette.black,
        )?;
        paint_lcars_right_cap_bar(
            window,
            layers,
            row_x + column_width - cap_width,
            row_y,
            cap_width,
            row_height,
            fill_byte,
        )?;
        window.filled_rectangle(layers, 0, rect(bay_x, row_y, bay_width * 0.26, 3.0), fill)?;
        window.filled_rectangle(
            layers,
            0,
            rect(
                bay_x + bay_width * 0.58,
                row_y + row_height - 3.0,
                bay_width * 0.24,
                2.0,
            ),
            fill,
        )?;

        let number_text = format!("{:02}", row.number);
        let number_cols = (tab_width / cell_width).floor().max(1.0) as usize;
        window.paint_owt_panel_text(
            layers,
            row_x + 6.0,
            row_y + ((row_height - cell_height) * 0.5).max(0.0),
            number_cols,
            &number_text,
            RgbColor::new_8bpc(0, 0, 0),
            true,
        )?;
        window.paint_owt_panel_text(
            layers,
            bay_x + 10.0,
            row_y + ((row_height - cell_height) * 0.5).max(0.0),
            text_cols.saturating_sub(1).max(4),
            &fit_text_ellipsis(
                &format!("{:<13} {}", row.label, row.text),
                text_cols.saturating_sub(1).max(4),
            ),
            lcars_signal_text_color(row.kind, index),
            false,
        )?;
    }
    Ok(())
}

fn paint_lcars_interface_switcher_spine(
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
        rect(x - 5.0, y - 4.0, width + 10.0, height + 8.0),
        palette.black,
    )?;
    let header_h = 34.0_f32.min(height * 0.16).max(28.0);
    paint_lcars_left_cap_bar(window, layers, x, y, width, header_h, LCARS_BYTE_ORANGE)?;
    window.filled_rectangle(
        layers,
        0,
        rect(x + 13.0, y + 6.0, width - 22.0, header_h - 12.0),
        palette.black,
    )?;
    window.paint_owt_panel_text(
        layers,
        x + 17.0,
        y + 8.0,
        ((width - 26.0) / window.render_metrics.cell_size.width as f32)
            .floor()
            .max(3.0) as usize,
        "LCARS",
        RgbColor::new_8bpc(255, 204, 112),
        true,
    )?;

    let labels = ["LOAD", "DOCK", "SHOW", "AUX"];
    let gap = 9.0;
    let slot_top = y + header_h + 14.0;
    let slot_h = ((height - header_h - 20.0 - gap * (labels.len().saturating_sub(1) as f32))
        / labels.len() as f32)
        .clamp(28.0, 48.0);
    for (index, label) in labels.iter().enumerate() {
        let slot_y = slot_top + index as f32 * (slot_h + gap);
        if slot_y + slot_h > y + height {
            break;
        }
        let fill_byte = match index {
            0 => LCARS_BYTE_PEACH,
            1 => LCARS_BYTE_VIOLET,
            2 => LCARS_BYTE_BLUE,
            _ => LCARS_BYTE_AMBER,
        };
        paint_lcars_left_cap_bar(window, layers, x, slot_y, width, slot_h, fill_byte)?;
        window.filled_rectangle(
            layers,
            0,
            rect(x + 10.0, slot_y + 5.0, width - 18.0, slot_h - 10.0),
            palette.black,
        )?;
        let number = format!("{:02}", index + 1);
        window.paint_owt_panel_text(
            layers,
            x + 13.0,
            slot_y + ((slot_h - window.render_metrics.cell_size.height as f32) * 0.5).max(0.0),
            2,
            &number,
            RgbColor::new_8bpc(255, 204, 112),
            true,
        )?;
        let label_cols = ((width - 40.0) / window.render_metrics.cell_size.width as f32)
            .floor()
            .max(2.0) as usize;
        window.paint_owt_panel_text(
            layers,
            x + 36.0,
            slot_y + ((slot_h - window.render_metrics.cell_size.height as f32) * 0.5).max(0.0),
            label_cols,
            &fit_text_ellipsis(label, label_cols),
            RgbColor::new_8bpc(153, 204, 255),
            true,
        )?;
    }
    window.filled_rectangle(
        layers,
        0,
        rect(x, y + height - 4.0, width * 0.68, 3.0),
        palette.peach,
    )?;
    Ok(())
}

fn paint_lcars_interface_switcher_strip(
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
        rect(x - 4.0, y - 4.0, width + 8.0, height + 8.0),
        palette.black,
    )?;
    let labels = ["LOAD", "DOCK", "SHOW"];
    let gap = 8.0;
    let label_w = ((width - gap * (labels.len().saturating_sub(1) as f32)) / labels.len() as f32)
        .clamp(42.0, 76.0);
    for (index, label) in labels.iter().enumerate() {
        let item_x = x + index as f32 * (label_w + gap);
        if item_x + label_w > x + width {
            break;
        }
        let fill_byte = match index {
            0 => LCARS_BYTE_ORANGE,
            1 => LCARS_BYTE_VIOLET,
            _ => LCARS_BYTE_BLUE,
        };
        paint_lcars_left_cap_bar(window, layers, item_x, y, label_w, height, fill_byte)?;
        window.filled_rectangle(
            layers,
            0,
            rect(item_x + 10.0, y + 8.0, label_w - 16.0, height - 16.0),
            palette.black,
        )?;
        let label_cols = ((label_w - 18.0) / window.render_metrics.cell_size.width as f32)
            .floor()
            .max(2.0) as usize;
        window.paint_owt_panel_text(
            layers,
            item_x + 13.0,
            y + ((height - window.render_metrics.cell_size.height as f32) * 0.5).max(0.0),
            label_cols,
            &fit_text_ellipsis(label, label_cols),
            RgbColor::new_8bpc(255, 204, 112),
            true,
        )?;
    }
    Ok(())
}

fn paint_lcars_surface_menu_slab(
    window: &mut crate::TermWindow,
    layers: &mut TripleLayerQuadAllocator,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    label: &str,
    palette: LcarsPalette,
) -> anyhow::Result<()> {
    paint_lcars_left_cap_bar(window, layers, x, y, width, height, LCARS_BYTE_AMBER)?;
    let cell_width = window.render_metrics.cell_size.width as f32;
    let cell_height = window.render_metrics.cell_size.height as f32;
    window.filled_rectangle(
        layers,
        0,
        rect(
            x + height * 0.78,
            y + 5.0,
            width - height - 14.0,
            height - 10.0,
        ),
        palette.black,
    )?;
    let cols = ((width - height - 20.0) / cell_width).floor().max(4.0) as usize;
    window.paint_owt_panel_text(
        layers,
        x + height * 0.82 + 8.0,
        y + ((height - cell_height) * 0.5).max(0.0),
        cols,
        &fit_text_ellipsis(label, cols),
        RgbColor::new_8bpc(255, 204, 112),
        true,
    )
}

fn paint_lcars_block_composition(
    window: &mut crate::TermWindow,
    layers: &mut TripleLayerQuadAllocator,
    scene: &LcarsStructuralScene,
    document: &InterfaceDocument,
    cell_width: f32,
    cell_height: f32,
    palette: LcarsPalette,
) -> anyhow::Result<()> {
    let data = lcars_block_composition_data(document);
    let area_x = scene.signal_field.x;
    let area_y = scene.signal_field.y;
    let area_width = (scene.plan.signal_width - 4.0).max(cell_width * 24.0);
    let area_bottom = (scene.content_bay.y - 12.0).max(area_y + cell_height * 8.0);
    let area_height = (area_bottom - area_y).max(cell_height * 8.0);
    if area_width < cell_width * 22.0 || area_height < cell_height * 7.0 {
        return Ok(());
    }

    let gap = 16.0;
    paint_lcars_block_composition_chassis(
        window,
        layers,
        area_x,
        area_y,
        area_width,
        area_height,
        palette,
    )?;

    let inner_x = area_x + 18.0;
    let inner_y = area_y + 18.0;
    let inner_width = (area_width - 34.0).max(cell_width * 20.0);
    let inner_height = (area_height - 32.0).max(cell_height * 6.0);
    let inline = area_width >= cell_width * 76.0 && area_height >= cell_height * 9.0;
    let (
        data_x,
        data_y,
        data_width,
        data_height,
        graphics_x,
        graphics_y,
        graphics_width,
        graphics_height,
    ) = if inline {
        let show_function_column = inner_width >= cell_width * 70.0;
        let function_width = if show_function_column {
            clamp_ordered(inner_width * 0.18, cell_width * 12.0, 220.0)
        } else {
            0.0
        };
        if show_function_column {
            paint_lcars_block_function_column(
                window,
                layers,
                inner_x,
                inner_y + 4.0,
                function_width,
                inner_height - 8.0,
                &data,
                cell_width,
                cell_height,
                palette,
            )?;
        }
        let block_x = inner_x + function_width + if show_function_column { gap } else { 0.0 };
        let block_width =
            (inner_width - function_width - if show_function_column { gap } else { 0.0 })
                .max(cell_width * 42.0);
        let min_graphics_width = (cell_width * 24.0).max(240.0);
        let data_width = clamp_ordered(block_width * 0.34, cell_width * 22.0, block_width * 0.44)
            .min((block_width - gap - min_graphics_width).max(cell_width * 18.0));
        (
            block_x,
            inner_y + 4.0,
            data_width,
            inner_height - 8.0,
            block_x + data_width + gap,
            inner_y + 4.0,
            (block_width - data_width - gap).max(min_graphics_width),
            inner_height - 8.0,
        )
    } else {
        let data_height = (inner_height * 0.48).max(cell_height * 5.0);
        (
            inner_x,
            inner_y,
            inner_width,
            data_height,
            inner_x,
            inner_y + data_height + gap,
            inner_width,
            (inner_height - data_height - gap).max(cell_height * 5.0),
        )
    };

    paint_lcars_labelled_data_block(
        window,
        layers,
        data_x,
        data_y,
        data_width,
        data_height,
        &data,
        cell_width,
        cell_height,
        palette,
    )?;
    paint_lcars_block_detail_bay(
        window,
        layers,
        document,
        graphics_x,
        graphics_y,
        graphics_width,
        graphics_height,
        &data,
        cell_width,
        cell_height,
        palette,
    )
}

fn paint_lcars_block_composition_chassis(
    window: &mut crate::TermWindow,
    layers: &mut TripleLayerQuadAllocator,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    palette: LcarsPalette,
) -> anyhow::Result<()> {
    window.filled_rectangle(layers, 0, rect(x, y, width, height), palette.black)?;
    paint_lcars_generated_bitmap(window, layers, x, y, width, height, |raster| {
        let top_h = 9.0;
        let rail_w = 18.0;
        raster.fill_rect(
            0.0,
            0.0,
            width * 0.58,
            top_h,
            LCARS_BYTE_BLUE.with_alpha(226),
        );
        raster.fill_rect(width * 0.60, 0.0, width * 0.12, top_h, LCARS_BYTE_PEACH);
        raster.fill_rect(
            width * 0.74,
            0.0,
            width * 0.16,
            top_h,
            LCARS_BYTE_VIOLET.with_alpha(226),
        );
        raster.fill_rect(
            0.0,
            0.0,
            rail_w,
            height * 0.70,
            LCARS_BYTE_BLUE.with_alpha(218),
        );
        raster.fill_rect(0.0, height * 0.74, rail_w, height * 0.18, LCARS_BYTE_PEACH);
        raster.fill_rect(
            width - rail_w,
            height * 0.12,
            rail_w,
            height * 0.52,
            LCARS_BYTE_BLUE.with_alpha(208),
        );
        raster.fill_rect(0.0, height - 7.0, width * 0.28, 6.0, LCARS_BYTE_PEACH);
        raster.fill_rect(
            width * 0.44,
            height - 6.0,
            width * 0.18,
            5.0,
            LCARS_BYTE_AMBER,
        );
        raster.fill_rect(
            width * 0.70,
            height - 5.0,
            width * 0.16,
            4.0,
            LCARS_BYTE_BLUE.with_alpha(204),
        );
    })
}

fn paint_lcars_block_function_column(
    window: &mut crate::TermWindow,
    layers: &mut TripleLayerQuadAllocator,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    data: &LcarsBlockCompositionData,
    cell_width: f32,
    cell_height: f32,
    palette: LcarsPalette,
) -> anyhow::Result<()> {
    let mut labels = data
        .boxes
        .iter()
        .take(2)
        .map(|item| item.label.clone())
        .collect::<Vec<_>>();
    if data.has_graphics {
        labels.push(data.graphics_title.clone());
    } else {
        labels.push(data.command_bank_title.clone());
    }
    while labels.len() < 3 {
        labels.push("CONTROL".to_string());
    }

    let row_gap = 10.0;
    let row_height = (cell_height * 2.05).clamp(34.0, 44.0);
    let row_count = labels.len().min(
        ((height + row_gap) / (row_height + row_gap))
            .floor()
            .max(1.0) as usize,
    );
    for (index, label) in labels.iter().take(row_count).enumerate() {
        let row_y = y + index as f32 * (row_height + row_gap);
        let fill_byte = match index {
            0 => LCARS_BYTE_ORANGE,
            1 => LCARS_BYTE_AMBER,
            _ => LCARS_BYTE_BLUE,
        };
        paint_lcars_left_cap_bar(window, layers, x, row_y, width, row_height, fill_byte)?;
        let cap_w = (row_height * 0.95).clamp(30.0, 42.0);
        window.filled_rectangle(
            layers,
            0,
            rect(
                x + cap_w,
                row_y + 5.0,
                width - cap_w - 8.0,
                row_height - 10.0,
            ),
            palette.black,
        )?;
        let number = format!("{:02}", index + 1);
        window.paint_owt_panel_text(
            layers,
            x + 7.0,
            row_y + ((row_height - cell_height) * 0.5).max(0.0),
            2,
            &number,
            RgbColor::new_8bpc(0, 0, 0),
            true,
        )?;
        let label_cols = ((width - cap_w - 16.0) / cell_width).floor().max(3.0) as usize;
        window.paint_owt_panel_text(
            layers,
            x + cap_w + 8.0,
            row_y + ((row_height - cell_height) * 0.5).max(0.0),
            label_cols,
            &fit_text_ellipsis(label, label_cols),
            RgbColor::new_8bpc(255, 204, 112),
            true,
        )?;
    }
    Ok(())
}

fn paint_lcars_labelled_data_block(
    window: &mut crate::TermWindow,
    layers: &mut TripleLayerQuadAllocator,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    data: &LcarsBlockCompositionData,
    cell_width: f32,
    cell_height: f32,
    palette: LcarsPalette,
) -> anyhow::Result<()> {
    window.filled_rectangle(layers, 0, rect(x, y, width, height), palette.black)?;
    paint_lcars_content_bay_frame(window, layers, x, y, width, height, false, palette)?;

    let label_h = (cell_height + 8.0).clamp(24.0, 31.0);
    let label_w = clamp_to_available(width * 0.56, cell_width * 15.0, width - 22.0);
    paint_lcars_embedded_label_tab(
        window,
        layers,
        x + 16.0,
        y + 10.0,
        label_w,
        label_h,
        LCARS_BYTE_AMBER,
        palette,
    )?;
    let title_cols = ((label_w - 34.0) / cell_width).floor().max(4.0) as usize;
    window.paint_owt_panel_text(
        layers,
        x + 38.0,
        y + 15.0,
        title_cols,
        &fit_text_ellipsis(&data.title, title_cols),
        RgbColor::new_8bpc(255, 204, 112),
        true,
    )?;

    let row_top = y + label_h + 28.0;
    let row_gap = 12.0;
    let rows = data.boxes.len().clamp(1, 4);
    let row_height = ((height - (row_top - y) - 16.0 - (row_gap * rows.saturating_sub(1) as f32))
        / rows as f32)
        .clamp(cell_height * 1.8, cell_height * 3.0);
    for (index, item) in data.boxes.iter().take(rows).enumerate() {
        let row_y = row_top + index as f32 * (row_height + row_gap);
        if row_y + row_height > y + height - 8.0 {
            break;
        }
        paint_lcars_data_box_row(
            window,
            layers,
            x + 24.0,
            row_y,
            width - 44.0,
            row_height,
            item,
            index,
            cell_width,
            palette,
        )?;
    }
    Ok(())
}

fn paint_lcars_data_box_row(
    window: &mut crate::TermWindow,
    layers: &mut TripleLayerQuadAllocator,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    item: &LcarsBlockDataBox,
    index: usize,
    cell_width: f32,
    palette: LcarsPalette,
) -> anyhow::Result<()> {
    let label_width = clamp_to_available(width * 0.34, cell_width * 8.0, width * 0.48);
    let value_x = x + label_width + 12.0;
    let value_width = (width - label_width - 12.0).max(cell_width * 10.0);
    let fill_byte = match index % 4 {
        0 => LCARS_BYTE_BLUE,
        1 => LCARS_BYTE_PEACH,
        2 => LCARS_BYTE_VIOLET,
        _ => LCARS_BYTE_AMBER,
    };
    let fill = color(fill_byte.red, fill_byte.green, fill_byte.blue);
    window.filled_rectangle(layers, 0, rect(x, y, width, height), palette.black)?;
    paint_lcars_left_cap_bar(window, layers, x, y, label_width, height, fill_byte)?;
    window.filled_rectangle(
        layers,
        0,
        rect(x + label_width - 4.0, y, 10.0, height),
        palette.black,
    )?;
    window.filled_rectangle(
        layers,
        0,
        rect(value_x, y + 5.0, value_width, height - 10.0),
        palette.black,
    )?;
    window.filled_rectangle(layers, 0, rect(value_x, y, value_width * 0.70, 4.0), fill)?;
    window.filled_rectangle(
        layers,
        0,
        rect(
            value_x + value_width * 0.38,
            y + height - 5.0,
            value_width * 0.42,
            3.0,
        ),
        fill,
    )?;
    let label_cols = (label_width / cell_width).floor().max(3.0) as usize;
    let value_cols = (value_width / cell_width).floor().max(4.0) as usize;
    window.paint_owt_panel_text(
        layers,
        x + 10.0,
        y + ((height - window.render_metrics.cell_size.height as f32) * 0.5).max(0.0),
        label_cols.saturating_sub(1).max(3),
        &fit_text_ellipsis(&item.label, label_cols.saturating_sub(1).max(3)),
        RgbColor::new_8bpc(0, 0, 0),
        true,
    )?;
    window.paint_owt_panel_text(
        layers,
        value_x + 12.0,
        y + ((height - window.render_metrics.cell_size.height as f32) * 0.5).max(0.0),
        value_cols.saturating_sub(2).max(4),
        &fit_text_ellipsis(&item.value, value_cols.saturating_sub(2).max(4)),
        RgbColor::new_8bpc(153, 204, 255),
        true,
    )?;
    Ok(())
}

fn paint_lcars_block_detail_bay(
    window: &mut crate::TermWindow,
    layers: &mut TripleLayerQuadAllocator,
    document: &InterfaceDocument,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    data: &LcarsBlockCompositionData,
    cell_width: f32,
    cell_height: f32,
    palette: LcarsPalette,
) -> anyhow::Result<()> {
    if data.has_graphics {
        return paint_lcars_graphics_block(
            window,
            layers,
            x,
            y,
            width,
            height,
            data,
            cell_width,
            cell_height,
            palette,
        );
    }

    let max_columns = if width >= cell_width * 92.0 {
        6
    } else if width >= cell_width * 70.0 {
        5
    } else if width >= cell_width * 50.0 {
        4
    } else {
        3
    };
    let max_rows = ((height - 72.0) / ((cell_height * 1.82).clamp(34.0, 42.0) + 7.0))
        .floor()
        .max(1.0) as usize;
    if let Some(table) = lcars_table_data(document, max_columns, max_rows) {
        return paint_lcars_table_bay(window, layers, x, y, width, height, &table, palette);
    }

    paint_lcars_data_cascade(window, layers, x, y, width, height, document, palette)
}

fn paint_lcars_graphics_block(
    window: &mut crate::TermWindow,
    layers: &mut TripleLayerQuadAllocator,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    data: &LcarsBlockCompositionData,
    cell_width: f32,
    cell_height: f32,
    palette: LcarsPalette,
) -> anyhow::Result<()> {
    window.filled_rectangle(layers, 0, rect(x, y, width, height), palette.black)?;
    paint_lcars_generated_bitmap(window, layers, x, y, width, height, |raster| {
        raster.fill_rect(0.0, 0.0, width, height, LCARS_BYTE_BLACK);
        raster.fill_rect(0.0, 0.0, width * 0.44, 7.0, LCARS_BYTE_BLUE);
        raster.fill_rect(width * 0.50, 0.0, width * 0.24, 6.0, LCARS_BYTE_VIOLET);
        raster.fill_rect(width * 0.80, 0.0, width * 0.14, 4.0, LCARS_BYTE_AMBER);
        raster.fill_rect(0.0, 0.0, 10.0, height * 0.35, LCARS_BYTE_BLUE);
        raster.fill_rect(0.0, height * 0.68, 10.0, height * 0.28, LCARS_BYTE_PEACH);
        raster.fill_rect(
            width - 12.0,
            height * 0.14,
            8.0,
            height * 0.62,
            LCARS_BYTE_BLUE,
        );
        let grid_left = 26.0;
        let grid_top = 44.0;
        let grid_width = (width - 58.0).max(20.0);
        let grid_height = (height - 68.0).max(20.0);
        for step in 0..=4 {
            let px = grid_left + grid_width * step as f32 / 4.0;
            raster.fill_rect(
                px,
                grid_top,
                2.0,
                grid_height,
                LCARS_BYTE_BLUE.with_alpha(104),
            );
        }
        for step in 0..=3 {
            let py = grid_top + grid_height * step as f32 / 3.0;
            raster.fill_rect(
                grid_left,
                py,
                grid_width,
                2.0,
                LCARS_BYTE_BLUE.with_alpha(98),
            );
        }
        raster.fill_rect(
            grid_left + grid_width * 0.15,
            grid_top + grid_height * 0.38,
            grid_width * 0.34,
            3.0,
            LCARS_BYTE_AMBER,
        );
        raster.fill_rect(
            grid_left + grid_width * 0.58,
            grid_top + grid_height * 0.58,
            grid_width * 0.25,
            3.0,
            LCARS_BYTE_VIOLET,
        );
        raster.fill_rect(
            grid_left + grid_width * 0.45,
            grid_top + grid_height * 0.22,
            4.0,
            grid_height * 0.62,
            LCARS_BYTE_BLUE,
        );
    })?;

    let label_h = (cell_height + 8.0).clamp(24.0, 31.0);
    let label_w = clamp_to_available(width * 0.62, cell_width * 14.0, width - 20.0);
    paint_lcars_embedded_label_tab(
        window,
        layers,
        x + 18.0,
        y + 10.0,
        label_w,
        label_h,
        LCARS_BYTE_BLUE,
        palette,
    )?;
    let label_cols = ((label_w - 34.0) / cell_width).floor().max(4.0) as usize;
    window.paint_owt_panel_text(
        layers,
        x + 40.0,
        y + 15.0,
        label_cols,
        &fit_text_ellipsis(&data.graphics_title, label_cols),
        RgbColor::new_8bpc(153, 204, 255),
        true,
    )?;
    let scale_height = (cell_height * 1.6).clamp(24.0, 34.0);
    let scale_y = y + height - scale_height - cell_height - 18.0;
    if scale_y > y + 70.0 {
        paint_lcars_horizontal_scale(
            window,
            layers,
            x + 32.0,
            scale_y,
            width - 90.0,
            scale_height,
        )?;
    }

    let detail_cols = ((width - 62.0) / cell_width).floor().max(4.0) as usize;
    window.paint_owt_panel_text(
        layers,
        x + 30.0,
        y + height - cell_height - 10.0,
        detail_cols,
        &fit_text_ellipsis(&data.graphics_detail, detail_cols),
        RgbColor::new_8bpc(255, 204, 112),
        false,
    )?;
    paint_lcars_zoom_control_ticks(
        window,
        layers,
        x + width - 54.0,
        y + 52.0,
        36.0,
        (height - 92.0).max(cell_height * 3.0),
        palette,
    )
}

fn paint_lcars_horizontal_scale(
    window: &mut crate::TermWindow,
    layers: &mut TripleLayerQuadAllocator,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
) -> anyhow::Result<()> {
    if width <= 40.0 || height <= 12.0 {
        return Ok(());
    }
    paint_lcars_generated_bitmap(window, layers, x, y, width, height, |raster| {
        let rail_y = height * 0.34;
        let rail_h = (height * 0.38).max(8.0);
        raster.fill_rect(
            0.0,
            rail_y,
            width * 0.42,
            rail_h,
            LCARS_BYTE_BLUE.with_alpha(210),
        );
        raster.fill_rect(
            width * 0.42,
            rail_y,
            width * 0.28,
            rail_h,
            LCARS_BYTE_AMBER.with_alpha(228),
        );
        raster.fill_rect(width * 0.70, rail_y, width * 0.16, rail_h, LCARS_BYTE_PEACH);
        raster.fill_rect(
            width * 0.86,
            rail_y,
            width * 0.14,
            rail_h,
            LCARS_BYTE_BLACK.with_alpha(220),
        );
        for step in 0..=24 {
            let px = width * step as f32 / 24.0;
            let major = step % 4 == 0;
            let tick_h = if major { height * 0.78 } else { height * 0.48 };
            let tick_w = if major { 2.0 } else { 1.0 };
            raster.fill_rect(
                px,
                height - tick_h,
                tick_w,
                tick_h,
                LCARS_BYTE_BLUE.with_alpha(220),
            );
        }
    })
}

fn paint_lcars_zoom_control_ticks(
    window: &mut crate::TermWindow,
    layers: &mut TripleLayerQuadAllocator,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    palette: LcarsPalette,
) -> anyhow::Result<()> {
    let tick_h = (height / 5.0).clamp(18.0, 28.0);
    for (index, label) in ["Z+", "Z-", "1:1"].iter().enumerate() {
        let tick_y = y + index as f32 * (tick_h + 8.0);
        if tick_y + tick_h > y + height {
            break;
        }
        let fill_byte = match index {
            0 => LCARS_BYTE_PEACH,
            1 => LCARS_BYTE_VIOLET,
            _ => LCARS_BYTE_AMBER,
        };
        paint_lcars_right_cap_bar(window, layers, x, tick_y, width, tick_h, fill_byte)?;
        window.filled_rectangle(
            layers,
            0,
            rect(x + 8.0, tick_y + 5.0, width - 18.0, tick_h - 10.0),
            palette.black,
        )?;
        window.paint_owt_panel_text(
            layers,
            x + 11.0,
            tick_y + ((tick_h - window.render_metrics.cell_size.height as f32) * 0.5).max(0.0),
            3,
            label,
            RgbColor::new_8bpc(255, 204, 112),
            true,
        )?;
    }
    Ok(())
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
    sleek: bool,
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

    if sleek {
        return Ok(());
    }

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
        let gutter = 10.0;
        let body_x = gutter;
        let body_w = (width - gutter).max(1.0);
        raster.fill_rect(0.0, 0.0, gutter, height, LCARS_BYTE_BLACK);
        let mut cursor = 0.0;
        let mut segment_index = 0usize;
        let segments = [
            (0.050, LCARS_BYTE_PEACH),
            (0.080, LCARS_BYTE_ORANGE),
            (0.040, LCARS_BYTE_VIOLET),
            (0.130, LCARS_BYTE_ORANGE),
            (0.090, LCARS_BYTE_BLUE),
            (0.035, LCARS_BYTE_AMBER),
            (0.160, LCARS_BYTE_ORANGE),
            (0.060, LCARS_BYTE_VIOLET),
            (0.110, LCARS_BYTE_ORANGE),
            (0.045, LCARS_BYTE_BLUE),
            (0.140, LCARS_BYTE_ORANGE),
            (0.065, LCARS_BYTE_VIOLET),
        ];
        for (fraction, fill) in segments {
            let segment_h = (height * fraction).clamp(18.0, 150.0);
            if cursor + segment_h > height {
                break;
            }
            raster.fill_rect(body_x, cursor, body_w, segment_h, fill);
            if segment_index % 3 == 1 {
                raster.fill_rect(
                    body_x,
                    cursor + segment_h * 0.42,
                    body_w * 0.54,
                    5.0,
                    LCARS_BYTE_BLACK,
                );
                raster.fill_rect(
                    body_x + body_w * 0.62,
                    cursor + segment_h * 0.42,
                    body_w * 0.38,
                    5.0,
                    LCARS_BYTE_BLUE.with_alpha(230),
                );
            }
            cursor += segment_h + if segment_index % 2 == 0 { 9.0 } else { 15.0 };
            segment_index += 1;
        }
        if cursor < height {
            raster.fill_rect(body_x, cursor, body_w, height - cursor, LCARS_BYTE_ORANGE);
        }
        raster.fill_rect(0.0, height * 0.5 - 18.0, gutter, 36.0, LCARS_BYTE_VIOLET);
        for index in 0..14 {
            let tick_y = 98.0 + (index as f32 * 29.0);
            if tick_y + 10.0 >= height - 48.0 {
                break;
            }
            let tick_w = if index % 3 == 0 {
                gutter
            } else {
                gutter * 0.62
            };
            let fill = match index % 4 {
                0 => LCARS_BYTE_BLUE,
                1 => LCARS_BYTE_PEACH,
                2 => LCARS_BYTE_VIOLET,
                _ => LCARS_BYTE_AMBER,
            };
            raster.fill_rect(0.0, tick_y, tick_w, 3.0, fill.with_alpha(224));
        }
    })
}

fn paint_lcars_docked_side_rail_chrome(
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
    let gutter = 10.0;
    let body_x = x + gutter;
    let body_w = (width - gutter).max(1.0);
    let mut cursor = y;
    let segments = [
        (0.060, palette.peach),
        (0.145, palette.orange),
        (0.120, palette.orange),
        (0.090, palette.violet),
        (0.105, palette.blue),
        (0.060, palette.amber),
        (0.180, palette.orange),
        (0.090, palette.violet),
        (0.150, palette.orange),
    ];

    window.filled_rectangle(layers, 0, rect(x, y, gutter, height), palette.black)?;
    for (index, (fraction, fill)) in segments.iter().copied().enumerate() {
        let segment_h = (height * fraction).clamp(26.0, 172.0);
        if cursor + segment_h > y + height {
            break;
        }
        window.filled_rectangle(layers, 0, rect(body_x, cursor, body_w, segment_h), fill)?;
        if matches!(index, 2 | 5 | 7) {
            window.filled_rectangle(
                layers,
                0,
                rect(body_x, cursor + segment_h * 0.52, body_w * 0.52, 5.0),
                palette.black,
            )?;
        }
        cursor += segment_h + if index % 2 == 0 { 10.0 } else { 16.0 };
    }
    if cursor < y + height {
        window.filled_rectangle(
            layers,
            0,
            rect(body_x, cursor, body_w, y + height - cursor),
            palette.orange,
        )?;
    }
    for (index, tick_y) in [96.0, 132.0, 178.0, 226.0, 282.0, 348.0]
        .iter()
        .copied()
        .enumerate()
    {
        let tick_y = y + tick_y;
        if tick_y + 8.0 >= y + height - 44.0 {
            break;
        }
        let tick_w = if index % 2 == 0 {
            gutter
        } else {
            gutter * 0.56
        };
        let fill = match index % 3 {
            0 => palette.dim_blue,
            1 => palette.amber,
            _ => palette.dim_violet,
        };
        window.filled_rectangle(layers, 0, rect(x, tick_y, tick_w, 3.0), fill)?;
    }
    Ok(())
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
            let header_h = header_height.clamp(18.0, height.max(header_height));
            let rail_w = rail_width.clamp(24.0, width.max(24.0));
            let radius = header_h * 0.55;
            let throat_x = rail_w + 28.0;
            let throat_y = header_h + 10.0;
            let throat_w = (width - throat_x).max(1.0);
            let throat_h = (height - throat_y).max(1.0);

            raster.fill_rounded_rect(0.0, 0.0, width, header_h, radius, LCARS_BYTE_ORANGE);
            raster.fill_rect(radius, 0.0, width - radius, header_h, LCARS_BYTE_ORANGE);
            raster.fill_rect(0.0, 0.0, rail_w, height, LCARS_BYTE_ORANGE);
            raster.fill_rect(0.0, 0.0, rail_w, 38.0, LCARS_BYTE_PEACH);
            raster.fill_rect(
                rail_w - 18.0,
                header_h,
                18.0,
                height - header_h,
                LCARS_BYTE_ORANGE,
            );
            raster.fill_rect(0.0, header_h + 8.0, rail_w, 8.0, LCARS_BYTE_BLACK);
            raster.fill_rect(0.0, header_h + 26.0, rail_w * 0.58, 24.0, LCARS_BYTE_VIOLET);
            raster.fill_rect(
                rail_w * 0.64,
                header_h + 26.0,
                rail_w * 0.36,
                24.0,
                LCARS_BYTE_BLUE,
            );
            raster.fill_rect(
                0.0,
                (height * 0.48).max(header_h + 20.0),
                rail_w,
                7.0,
                LCARS_BYTE_BLACK,
            );
            raster.fill_rect(0.0, height - 54.0, rail_w, 54.0, LCARS_BYTE_VIOLET);

            raster.fill_rounded_rect(
                throat_x,
                throat_y,
                throat_w,
                throat_h,
                (header_h * 0.92).clamp(24.0, 42.0),
                LCARS_BYTE_BLACK,
            );
            raster.fill_rect(
                throat_x + 34.0,
                throat_y,
                throat_w,
                throat_h,
                LCARS_BYTE_BLACK,
            );
            raster.fill_rect(
                rail_w + 22.0,
                height - 18.0,
                (width - rail_w - 42.0).max(1.0),
                5.0,
                LCARS_BYTE_PEACH,
            );
            raster.fill_rect(
                rail_w + 44.0,
                height - 10.0,
                (width * 0.30).max(48.0),
                3.0,
                LCARS_BYTE_BLUE,
            );
            raster.fill_rect(
                width * 0.72,
                header_h + 7.0,
                (width * 0.12).max(28.0),
                5.0,
                LCARS_BYTE_PEACH,
            );
        },
    )
}

fn paint_lcars_docked_primary_elbow(
    window: &mut crate::TermWindow,
    layers: &mut TripleLayerQuadAllocator,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    rail_width: f32,
    header_height: f32,
    palette: LcarsPalette,
) -> anyhow::Result<()> {
    let header_h = header_height.clamp(24.0, height.max(header_height));
    let rail_w = rail_width.clamp(36.0, width.max(36.0));
    let throat_x = x + rail_w + 54.0;
    let throat_y = y + header_h + 18.0;
    let throat_w = (width - rail_w - 54.0).max(1.0);
    let throat_h = (height - header_h - 18.0).max(1.0);

    window.filled_rectangle(layers, 0, rect(x, y, width, header_h), palette.orange)?;
    window.filled_rectangle(layers, 0, rect(x, y, rail_w, height), palette.orange)?;
    window.filled_rectangle(layers, 0, rect(x, y, rail_w, 40.0), palette.peach)?;
    window.filled_rectangle(
        layers,
        0,
        rect(x, y + header_h + 10.0, rail_w, 9.0),
        palette.black,
    )?;
    window.filled_rectangle(
        layers,
        0,
        rect(x, y + header_h + 28.0, rail_w * 0.58, 26.0),
        palette.violet,
    )?;
    window.filled_rectangle(
        layers,
        0,
        rect(x + rail_w * 0.66, y + header_h + 28.0, rail_w * 0.34, 26.0),
        palette.blue,
    )?;
    window.filled_rectangle(
        layers,
        0,
        rect(x, y + height - 62.0, rail_w, 62.0),
        palette.orange,
    )?;
    window.filled_rectangle(
        layers,
        0,
        rect(x, y + height - 62.0, rail_w, 9.0),
        palette.black,
    )?;
    window.filled_rectangle(
        layers,
        0,
        rect(throat_x, throat_y, throat_w, throat_h),
        palette.black,
    )?;
    window.filled_rectangle(
        layers,
        0,
        rect(
            x + rail_w + 26.0,
            y + height - 20.0,
            (width - rail_w - 58.0).max(1.0),
            6.0,
        ),
        palette.peach,
    )?;
    window.filled_rectangle(
        layers,
        0,
        rect(
            x + rail_w + 50.0,
            y + height - 11.0,
            (width * 0.32).max(54.0),
            3.0,
        ),
        palette.dim_blue,
    )?;
    Ok(())
}

fn paint_lcars_docked_header_run(
    window: &mut crate::TermWindow,
    layers: &mut TripleLayerQuadAllocator,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    palette: LcarsPalette,
) -> anyhow::Result<()> {
    let gap = 8.0;
    let mut cursor = x;
    let max_right = x + width;
    let fixed_segments = [
        (0.40, palette.orange),
        (0.04, palette.amber),
        (0.17, palette.violet),
        (0.04, palette.peach),
        (0.18, palette.dim_blue),
    ];
    for (fraction, fill) in fixed_segments {
        let segment_width = (width * fraction)
            .max(22.0)
            .min((max_right - cursor).max(0.0));
        if segment_width <= 0.0 {
            break;
        }
        window.filled_rectangle(layers, 0, rect(cursor, y, segment_width, height), fill)?;
        cursor += segment_width + gap;
    }
    if cursor < max_right {
        window.filled_rectangle(
            layers,
            0,
            rect(cursor, y, max_right - cursor, height),
            palette.dim_blue,
        )?;
    }
    Ok(())
}

fn paint_lcars_content_bay_frame(
    window: &mut crate::TermWindow,
    layers: &mut TripleLayerQuadAllocator,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    sleek: bool,
    palette: LcarsPalette,
) -> anyhow::Result<()> {
    if sleek {
        let bracket_w = 28.0;
        let upper_post_h = (height * 0.30).clamp(28.0, 72.0);
        let lower_post_h = (height * 0.18).clamp(26.0, 58.0);
        let orange_run_w = (width * 0.40).clamp(180.0, 680.0).min(width);
        let peach_run_w = (width * 0.24).clamp(96.0, 360.0);
        let peach_run_x = x + orange_run_w + 12.0;
        window.filled_rectangle(layers, 0, rect(x, y, orange_run_w, 9.0), palette.orange)?;
        if peach_run_x + 24.0 < x + width {
            window.filled_rectangle(
                layers,
                0,
                rect(
                    peach_run_x,
                    y,
                    peach_run_w.min((x + width - peach_run_x).max(0.0)),
                    7.0,
                ),
                palette.peach,
            )?;
        }
        window.filled_rectangle(
            layers,
            0,
            rect(x, y, bracket_w, upper_post_h),
            palette.orange,
        )?;
        window.filled_rectangle(
            layers,
            0,
            rect(x, y + height - lower_post_h, bracket_w, lower_post_h),
            palette.violet,
        )?;
        window.filled_rectangle(
            layers,
            0,
            rect(x, y + height - 5.0, (width * 0.26).clamp(150.0, 440.0), 4.0),
            palette.peach,
        )?;
        return Ok(());
    }

    paint_lcars_generated_bitmap(window, layers, x, y, width, height, |raster| {
        let bracket_w = 28.0;
        let top_h = 9.0;
        let upper_post_h = (height * 0.38).clamp(28.0, 74.0);
        let lower_post_h = (height * 0.27).clamp(22.0, 56.0);
        raster.fill_rect(0.0, 0.0, width * 0.40, top_h, LCARS_BYTE_ORANGE);
        raster.fill_rect(width * 0.54, 0.0, width * 0.24, top_h, LCARS_BYTE_VIOLET);
        raster.fill_rect(width * 0.86, 0.0, width * 0.10, 4.0, LCARS_BYTE_AMBER);
        raster.fill_rect(0.0, 0.0, bracket_w, upper_post_h, LCARS_BYTE_ORANGE);
        raster.fill_rect(
            width - bracket_w,
            0.0,
            bracket_w,
            upper_post_h * 0.82,
            LCARS_BYTE_BLUE.with_alpha(218),
        );
        raster.fill_rect(
            0.0,
            height - lower_post_h,
            bracket_w,
            lower_post_h,
            LCARS_BYTE_PEACH,
        );
        raster.fill_rect(
            width - bracket_w,
            height - lower_post_h * 1.18,
            bracket_w,
            lower_post_h * 1.18,
            LCARS_BYTE_VIOLET,
        );
        raster.fill_rect(0.0, height - 5.0, width * 0.30, 4.0, LCARS_BYTE_PEACH);
        raster.fill_rect(
            width * 0.66,
            height - 5.0,
            width * 0.22,
            3.0,
            LCARS_BYTE_BLUE.with_alpha(210),
        );
        raster.fill_rect(
            bracket_w + 8.0,
            top_h + 8.0,
            width * 0.16,
            3.0,
            LCARS_BYTE_AMBER.with_alpha(218),
        );
        raster.fill_rect(
            width - bracket_w - width * 0.16 - 8.0,
            top_h + 8.0,
            width * 0.16,
            3.0,
            LCARS_BYTE_VIOLET.with_alpha(208),
        );
    })
}

fn paint_lcars_embedded_label_tab(
    window: &mut crate::TermWindow,
    layers: &mut TripleLayerQuadAllocator,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    fill: LcarsByteColor,
    palette: LcarsPalette,
) -> anyhow::Result<()> {
    let width = width.max(height + 12.0);
    paint_lcars_right_cap_bar(window, layers, x, y, width, height, fill)?;
    let bay_x = x + (height * 0.62).clamp(12.0, 22.0);
    let bay_y = y + (height * 0.18).clamp(4.0, 7.0);
    let bay_w = (width - (height * 1.06)).max(24.0);
    let bay_h = (height - ((bay_y - y) * 2.0)).max(8.0);
    window.filled_rectangle(layers, 0, rect(bay_x, bay_y, bay_w, bay_h), palette.black)?;
    window.filled_rectangle(
        layers,
        0,
        rect(x + 4.0, y + height - 4.0, width * 0.30, 2.0),
        palette.black,
    )?;
    Ok(())
}

fn paint_lcars_minor_bar_rhythm(
    window: &mut crate::TermWindow,
    layers: &mut TripleLayerQuadAllocator,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    palette: LcarsPalette,
) -> anyhow::Result<()> {
    let gap = 8.0;
    let segments = [
        (0.34, palette.orange),
        (0.04, palette.amber),
        (0.16, palette.violet),
        (0.08, palette.peach),
        (0.22, palette.dim_blue),
    ];
    let mut cursor = x;
    let max_right = x + width;
    for (fraction, fill) in segments {
        let segment_width = (width * fraction)
            .max(22.0)
            .min((max_right - cursor).max(0.0));
        if segment_width <= 0.0 {
            break;
        }
        window.filled_rectangle(layers, 0, rect(cursor, y, segment_width, height), fill)?;
        cursor += segment_width + gap;
    }
    Ok(())
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
        let gap = 8.0;
        let mut cursor = 0.0;
        let fixed_segments = [
            (0.40, LCARS_BYTE_ORANGE),
            (0.04, LCARS_BYTE_AMBER),
            (0.17, LCARS_BYTE_VIOLET),
            (0.04, LCARS_BYTE_PEACH),
            (0.18, LCARS_BYTE_BLUE.with_alpha(224)),
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
                LCARS_BYTE_BLUE.with_alpha(210),
            );
            raster.fill_rounded_rect(
                cursor + remaining - height,
                0.0,
                height,
                height,
                radius,
                LCARS_BYTE_BLUE.with_alpha(210),
            );
        } else if remaining > 0.0 {
            raster.fill_rect(
                cursor,
                0.0,
                remaining,
                height,
                LCARS_BYTE_BLUE.with_alpha(210),
            );
        }
    })
}

fn paint_lcars_structural_console_chrome(
    window: &mut crate::TermWindow,
    layers: &mut TripleLayerQuadAllocator,
    content_left: f32,
    content_right: f32,
    scene: &LcarsStructuralScene,
    palette: LcarsPalette,
) -> anyhow::Result<()> {
    let plan = &scene.plan;
    let sleek = matches!(scene.mode, LcarsStructuralMode::DockedSurface);
    let signal_top = scene.signal_field.y;
    let content_bay_y = scene.content_bay.y;
    let field_width = (content_right - content_left).max(1.0);
    let upper_rule_y = signal_top - 18.0;
    let lower_rule_y = content_bay_y - 24.0;
    if sleek {
        let signal_rule_width = plan.signal_width.min(field_width * 0.50).max(120.0);
        window.filled_rectangle(
            layers,
            0,
            rect(content_left, upper_rule_y, signal_rule_width * 0.76, 5.0),
            palette.peach,
        )?;
        window.filled_rectangle(
            layers,
            0,
            rect(
                content_left + signal_rule_width * 0.80,
                upper_rule_y,
                signal_rule_width * 0.18,
                5.0,
            ),
            palette.dim_blue,
        )?;
    } else {
        paint_lcars_minor_bar_rhythm(
            window,
            layers,
            content_left,
            upper_rule_y,
            field_width * 0.58,
            5.0,
            palette,
        )?;
        paint_lcars_minor_bar_rhythm(
            window,
            layers,
            content_left,
            lower_rule_y,
            field_width * 0.46,
            5.0,
            palette,
        )?;
        window.filled_rectangle(
            layers,
            0,
            rect(
                content_left + field_width * 0.74,
                lower_rule_y + 10.0,
                field_width * 0.16,
                3.0,
            ),
            palette.dim_violet,
        )?;
    }

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

    let Some(command_bank) = scene.command_bank else {
        return Ok(());
    };
    let rows = command_bank.rows.max(1);
    let bank_x = command_bank.frame.x;
    let bank_y = command_bank.frame.y;
    let bank_width = command_bank.frame.width;
    let bank_height = command_bank.frame.height;
    let button_height = command_bank.button_height;
    let button_gap = command_bank.button_gap;

    if sleek {
        window.filled_rectangle(
            layers,
            0,
            rect(bank_x - 18.0, bank_y - 4.0, 18.0, bank_height + 8.0),
            palette.black,
        )?;
        window.filled_rectangle(
            layers,
            0,
            rect(bank_x - 12.0, bank_y + 4.0, 12.0, bank_height - 8.0),
            palette.orange,
        )?;
        window.filled_rectangle(
            layers,
            0,
            rect(bank_x - 12.0, bank_y + 42.0, 12.0, 9.0),
            palette.black,
        )?;
        window.filled_rectangle(
            layers,
            0,
            rect(bank_x - 12.0, bank_y + bank_height - 32.0, 12.0, 32.0),
            palette.violet,
        )?;
        window.filled_rectangle(
            layers,
            0,
            rect(
                bank_x + bank_width * 0.16,
                bank_y - 8.0,
                bank_width * 0.34,
                5.0,
            ),
            palette.peach,
        )?;
        let dock_top = bank_y + bank_height + 10.0;
        let dock_bottom = content_bay_y - 12.0;
        if dock_bottom > dock_top {
            let dock_h = dock_bottom - dock_top;
            window.filled_rectangle(
                layers,
                0,
                rect(bank_x - 18.0, dock_top - 4.0, 18.0, dock_h + 8.0),
                palette.black,
            )?;
            window.filled_rectangle(
                layers,
                0,
                rect(bank_x - 12.0, dock_top, 12.0, dock_h * 0.58),
                palette.violet,
            )?;
            window.filled_rectangle(
                layers,
                0,
                rect(bank_x - 12.0, dock_top + dock_h * 0.70, 12.0, dock_h * 0.30),
                palette.dim_blue,
            )?;
        }
        let bay_right = scene.content_bay.x + scene.content_bay.width;
        let bridge_x = bay_right + 4.0;
        let bridge_w = bank_x - 14.0 - bridge_x;
        if bridge_w >= 18.0 {
            window.filled_rectangle(
                layers,
                0,
                rect(bridge_x, content_bay_y + 4.0, bridge_w, 5.0),
                palette.dim_blue,
            )?;
        }
    }

    paint_lcars_generated_bitmap(
        window,
        layers,
        bank_x,
        bank_y,
        bank_width,
        bank_height,
        |raster| {
            let spine_w = 18.0;
            if sleek {
                raster.fill_rect(0.0, 0.0, spine_w, bank_height, LCARS_BYTE_BLACK);
                for row in 0..rows {
                    let row_y = 9.0 + (row as f32 * (button_height + button_gap));
                    let rail_fill = match row % 4 {
                        0 => LCARS_BYTE_PEACH,
                        1 => LCARS_BYTE_VIOLET,
                        2 => LCARS_BYTE_BLUE,
                        _ => LCARS_BYTE_AMBER,
                    };
                    raster.fill_rect(0.0, row_y + 4.0, 30.0, button_height - 8.0, rail_fill);
                    raster.fill_rect(26.0, row_y, 5.0, button_height, LCARS_BYTE_BLACK);
                }
                return;
            }

            raster.fill_rect(0.0, 0.0, spine_w, bank_height, LCARS_BYTE_BLACK);
            raster.fill_rect(0.0, 0.0, 10.0, bank_height, LCARS_BYTE_ORANGE);
            raster.fill_rect(0.0, 0.0, 10.0, 34.0, LCARS_BYTE_PEACH);
            raster.fill_rect(0.0, 43.0, 10.0, 11.0, LCARS_BYTE_BLACK);
            raster.fill_rect(0.0, bank_height - 42.0, 10.0, 42.0, LCARS_BYTE_VIOLET);
            raster.fill_rect(12.0, 0.0, 4.0, bank_height, LCARS_BYTE_BLACK);
            raster.fill_rect(spine_w, 0.0, bank_width * 0.36, 5.0, LCARS_BYTE_PEACH);
            if !sleek {
                raster.fill_rect(
                    spine_w + bank_width * 0.42,
                    0.0,
                    bank_width * 0.18,
                    5.0,
                    LCARS_BYTE_VIOLET,
                );
                raster.fill_rect(
                    spine_w + bank_width * 0.48,
                    bank_height - 4.0,
                    bank_width * 0.34,
                    4.0,
                    LCARS_BYTE_AMBER,
                );
            }
            for row in 0..rows {
                let row_y = 9.0 + (row as f32 * (button_height + button_gap));
                let rail_fill = match row % 4 {
                    0 => LCARS_BYTE_PEACH,
                    1 => LCARS_BYTE_VIOLET,
                    2 => LCARS_BYTE_BLUE,
                    _ => LCARS_BYTE_AMBER,
                };
                let paddle_w = if sleek { 30.0 } else { 24.0 };
                raster.fill_rect(0.0, row_y + 5.0, paddle_w, button_height - 10.0, rail_fill);
                if !sleek {
                    let rail_y = row_y + (button_height * 0.5) - 1.0;
                    raster.fill_rect(spine_w, rail_y, 42.0, 5.0, rail_fill);
                    raster.fill_rect(
                        spine_w + 46.0,
                        row_y + button_height - 4.0,
                        bank_width * 0.26,
                        3.0,
                        rail_fill.with_alpha(220),
                    );
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
    let columns = if inner_width >= 560.0 {
        5
    } else if inner_width >= 420.0 {
        4
    } else if inner_width >= 280.0 {
        3
    } else {
        2
    };
    let column_width = inner_width / columns as f32;
    let rows = ((inner_height / (cell_height * 0.86)).floor() as usize).clamp(3, 9);
    let seed = document.id.bytes().fold(0u32, |acc, byte| {
        acc.wrapping_mul(33).wrapping_add(byte as u32)
    });
    let colors = [
        RgbColor::new_8bpc(255, 136, 0),
        RgbColor::new_8bpc(255, 204, 112),
        RgbColor::new_8bpc(197, 143, 255),
        RgbColor::new_8bpc(164, 212, 255),
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
            let row_y = inner_y + (row as f32 * cell_height * 0.86);
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
            let text = if row % 4 == 0 {
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

fn paint_lcars_matrix_bay_chrome(
    window: &mut crate::TermWindow,
    layers: &mut TripleLayerQuadAllocator,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    title_bar_h: f32,
) -> anyhow::Result<()> {
    paint_lcars_generated_bitmap(window, layers, x, y, width, height, |raster| {
        let left_spine = 14.0;
        let top_y = 8.0;
        let header_y = 42.0;
        let body_y = 72.0;
        let body_h = (height - body_y - 10.0).max(28.0);
        let right_guard_x = (width - 12.0).max(left_spine + 1.0);

        raster.fill_rect(0.0, 0.0, width, height, LCARS_BYTE_BLACK);
        raster.fill_rect(
            left_spine,
            0.0,
            (width * 0.62).max(1.0),
            3.0,
            LCARS_BYTE_VIOLET,
        );
        raster.fill_rect(
            (width * 0.52).max(left_spine + 1.0),
            height - 3.0,
            width * 0.28,
            3.0,
            LCARS_BYTE_PEACH,
        );
        raster.fill_rect(0.0, top_y, 8.0, height - 18.0, LCARS_BYTE_BLUE);
        raster.fill_rect(0.0, top_y, 8.0, 30.0, LCARS_BYTE_AMBER);
        raster.fill_rect(0.0, body_y + body_h - 34.0, 8.0, 34.0, LCARS_BYTE_PEACH);
        raster.fill_rect(
            right_guard_x,
            body_y,
            5.0,
            body_h,
            LCARS_BYTE_BLUE.with_alpha(214),
        );
        raster.fill_rect(right_guard_x - 6.0, body_y, 2.0, body_h, LCARS_BYTE_VIOLET);

        let mut cursor = width * 0.36;
        let slab_y = top_y + 2.0;
        for (fraction, fill) in [
            (0.24, LCARS_BYTE_PEACH),
            (0.22, LCARS_BYTE_VIOLET),
            (0.18, LCARS_BYTE_BLUE),
        ] {
            let slab_width = (width * fraction)
                .min((width - cursor - 16.0).max(0.0))
                .max(0.0);
            if slab_width <= 14.0 {
                break;
            }
            raster.fill_rect(cursor, slab_y, slab_width, title_bar_h * 0.70, fill);
            cursor += slab_width + 8.0;
        }

        raster.fill_rect(left_spine, header_y, width * 0.50, 3.0, LCARS_BYTE_AMBER);
        raster.fill_rect(
            width * 0.60,
            header_y + 1.0,
            width * 0.24,
            2.0,
            LCARS_BYTE_VIOLET,
        );
        raster.fill_rect(
            left_spine,
            body_y + body_h - 4.0,
            width * 0.23,
            3.0,
            LCARS_BYTE_PEACH,
        );
        raster.fill_rect(
            width * 0.68,
            body_y + body_h - 9.0,
            width * 0.22,
            2.0,
            LCARS_BYTE_BLUE,
        );
    })
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
    if width < cell_width * 18.0 || height < cell_height * 5.0 {
        return Ok(());
    }

    let title_bar_h = (cell_height * 1.42).clamp(26.0, 34.0);
    let title_text_width = (width * 0.42).max(cell_width * 14.0);
    let title_cols = ((title_text_width - 12.0) / cell_width).max(6.0) as usize;
    paint_lcars_matrix_bay_chrome(window, layers, x, y, width, height, title_bar_h)?;
    window.filled_rectangle(
        layers,
        0,
        rect(x + 18.0, y + 11.0, title_text_width, title_bar_h),
        palette.black,
    )?;
    window.filled_rectangle(
        layers,
        0,
        rect(x + 18.0, y + 11.0, 12.0, title_bar_h),
        palette.amber,
    )?;
    window.paint_owt_panel_text(
        layers,
        x + 36.0,
        y + 16.0,
        title_cols,
        &table.title.to_uppercase(),
        RgbColor::new_8bpc(255, 204, 112),
        true,
    )?;
    let status_label = match (&table.focus_label, &table.sort_label) {
        (Some(focus), Some(sort)) => Some(format!("{focus} / {sort}")),
        (Some(focus), None) => Some(focus.clone()),
        (None, Some(sort)) => Some(sort.clone()),
        (None, None) => None,
    };
    if let Some(status_label) = status_label {
        let sort_cols = ((width * 0.32) / cell_width).floor().max(8.0) as usize;
        window.paint_owt_panel_text(
            layers,
            x + width - (sort_cols as f32 * cell_width) - 34.0,
            y + 16.0,
            sort_cols,
            &fit_text_ellipsis(&status_label.to_uppercase(), sort_cols),
            RgbColor::new_8bpc(203, 168, 255),
            true,
        )?;
    }

    let columns = table.columns.len().max(1);
    let table_left = x + 34.0;
    let table_width = (width - 76.0).max(cell_width * columns as f32);
    let header_y = y + 58.0;
    let row_y = header_y + (cell_height * 1.42);
    let row_height = (cell_height * 1.82).clamp(34.0, 42.0);
    let row_gap = 7.0;
    let detail_band_height = (cell_height * 1.65).clamp(24.0, 34.0);
    let detail_gap = 6.0;
    let detail_count = usize::from(table.group_detail.is_some())
        + usize::from(table.focused_detail.is_some())
        + usize::from(table.cell_detail.is_some())
        + usize::from(table.cell_focus.is_some());
    let detail_stack_height = if detail_count > 0 {
        detail_count as f32 * detail_band_height
            + detail_count.saturating_sub(1) as f32 * detail_gap
    } else {
        0.0
    };
    let detail_reserved_height = if detail_count > 0 {
        detail_stack_height + cell_height + 14.0
    } else {
        0.0
    };
    let available_rows = ((height - (row_y - y) - 10.0 - detail_reserved_height)
        / (row_height + row_gap))
        .floor()
        .max(0.0) as usize;
    let visible_rows = table.rows.len().min(available_rows);
    for hitbox in table_row_action_hitboxes(
        table,
        table_left,
        table_width,
        row_y,
        row_height,
        row_gap,
        visible_rows,
    ) {
        window.ui_items.push(UIItem {
            x: hitbox.x.max(0.0) as usize,
            y: hitbox.y.max(0.0) as usize,
            width: hitbox.width.ceil().max(1.0) as usize,
            height: hitbox.height.ceil().max(1.0) as usize,
            item_type: UIItemType::OwtLcarsAction {
                interface_id: table.interface_id.clone(),
                action_id: hitbox.action_id,
            },
        });
    }
    for hitbox in table_cell_focus_hitboxes(
        table,
        table_left,
        table_width,
        row_y,
        row_height,
        row_gap,
        visible_rows,
        cell_width,
    ) {
        window.ui_items.push(UIItem {
            x: hitbox.x.max(0.0) as usize,
            y: hitbox.y.max(0.0) as usize,
            width: hitbox.width.ceil().max(1.0) as usize,
            height: hitbox.height.ceil().max(1.0) as usize,
            item_type: UIItemType::OwtLcarsTableCell {
                interface_id: table.interface_id.clone(),
                column: hitbox.column,
            },
        });
    }
    let header_colors = [
        palette.peach,
        palette.violet,
        palette.dim_blue,
        palette.amber,
    ];

    for (index, column) in table.columns.iter().enumerate() {
        let (cell_x, cell_cols) =
            table_cell_geometry(table_left, table_width, columns, index, cell_width);
        let cell_px_width = (cell_cols as f32 * cell_width - 10.0).max(cell_width * 4.0);
        let fill = header_colors[index % header_colors.len()];
        window.filled_rectangle(
            layers,
            0,
            rect(
                cell_x + 2.0,
                header_y - 7.0,
                (cell_px_width * 0.78).max(12.0),
                4.0,
            ),
            fill,
        )?;
        window.filled_rectangle(
            layers,
            0,
            rect(
                cell_x + 2.0,
                header_y - 1.0,
                (cell_px_width * 0.82).max(cell_width * 5.0),
                cell_height + 8.0,
            ),
            palette.black,
        )?;
        window.filled_rectangle(
            layers,
            0,
            rect(cell_x + 2.0, header_y + 4.0, 8.0, cell_height * 0.72),
            fill,
        )?;
        window.paint_owt_panel_text(
            layers,
            cell_x + 16.0,
            header_y + 1.0,
            cell_cols.saturating_sub(2),
            &fit_text_ellipsis(&column.to_uppercase(), cell_cols.saturating_sub(2)),
            RgbColor::new_8bpc(153, 204, 255),
            true,
        )?;
    }

    for (row_index, row) in table.rows.iter().take(visible_rows).enumerate() {
        let y = row_y + (row_index as f32 * (row_height + row_gap));
        let lane = if row.focused {
            palette.amber
        } else {
            table_severity_fill(row.severity.as_deref(), row_index, palette)
        };
        let lane_byte = if row.focused {
            LCARS_BYTE_AMBER
        } else {
            table_severity_fill_byte(row.severity.as_deref(), row_index)
        };
        let row_fill = match row_index % 4 {
            0 => LCARS_BYTE_PEACH,
            1 => LCARS_BYTE_BLUE,
            2 => LCARS_BYTE_VIOLET,
            _ => LCARS_BYTE_AMBER,
        };
        window.filled_rectangle(
            layers,
            0,
            rect(
                table_left - 10.0,
                y - 2.0,
                table_width + 18.0,
                row_height + 4.0,
            ),
            palette.black,
        )?;
        if row.focused {
            window.filled_rectangle(
                layers,
                0,
                rect(table_left - 10.0, y - 2.0, table_width + 18.0, 3.0),
                palette.amber,
            )?;
            window.filled_rectangle(
                layers,
                0,
                rect(
                    table_left - 10.0,
                    y + row_height + 1.0,
                    table_width + 18.0,
                    3.0,
                ),
                palette.amber,
            )?;
        }
        paint_lcars_left_cap_bar(
            window,
            layers,
            table_left - 8.0,
            y,
            24.0,
            row_height,
            lane_byte,
        )?;
        paint_lcars_right_cap_bar(
            window,
            layers,
            table_left + table_width - 18.0,
            y,
            28.0,
            row_height,
            row_fill.with_alpha(240),
        )?;
        window.filled_rectangle(
            layers,
            0,
            rect(table_left + 22.0, y + 5.0, table_width * 0.36, 3.0),
            lane,
        )?;
        window.filled_rectangle(
            layers,
            0,
            rect(
                table_left + table_width * 0.48,
                y + row_height - 7.0,
                table_width * 0.30,
                3.0,
            ),
            color(row_fill.red, row_fill.green, row_fill.blue),
        )?;
        if row
            .group
            .as_ref()
            .map(|group| {
                row_index == 0
                    || table
                        .rows
                        .get(row_index - 1)
                        .and_then(|prior| prior.group.as_ref())
                        != Some(group)
            })
            .unwrap_or(false)
        {
            let group = row.group.as_deref().unwrap_or_default();
            let group_text = table_group_summary_for(&table.group_summaries, group)
                .map(table_group_summary_label)
                .unwrap_or_else(|| format!("GRP {group}"));
            let group_cols = ((table_width * 0.38) / cell_width)
                .floor()
                .clamp(12.0, 30.0) as usize;
            window.paint_owt_panel_text(
                layers,
                table_left + 30.0,
                y + 2.0,
                group_cols,
                &fit_text_ellipsis(&group_text.to_uppercase(), group_cols),
                RgbColor::new_8bpc(203, 168, 255),
                true,
            )?;
        }
        let mut flags = Vec::new();
        if row.focused {
            flags.push("FOCUS");
        }
        if row.action_id.is_some() {
            flags.push("DRILL");
        }
        if row.provenance.is_some() {
            flags.push("PROV");
        }
        if row.cell_severity.iter().any(Option::is_some)
            || row.cell_provenance.iter().any(Option::is_some)
        {
            flags.push("CELL");
        }
        if !flags.is_empty() {
            let flags = flags.join(" ");
            window.paint_owt_panel_text(
                layers,
                table_left + table_width - 108.0,
                y + 2.0,
                13,
                &fit_text_ellipsis(&flags, 13),
                RgbColor::new_8bpc(255, 204, 112),
                true,
            )?;
        }
        for (cell_index, cell) in row.cells.iter().enumerate().take(columns) {
            let (cell_x, cell_cols) =
                table_cell_geometry(table_left, table_width, columns, cell_index, cell_width);
            let cell_px_width = (cell_cols as f32 * cell_width - 8.0).max(cell_width * 4.0);
            let text_cols = cell_cols.saturating_sub(2).max(1);
            let focused_cell = row.focused
                && table
                    .cell_focus
                    .as_ref()
                    .map(|cell_focus| cell_focus.column_index == cell_index)
                    .unwrap_or(false);
            let cell_severity = row
                .cell_severity
                .get(cell_index)
                .and_then(|value| value.as_deref());
            let cell_provenance = row
                .cell_provenance
                .get(cell_index)
                .and_then(|value| value.as_deref());
            let cell_lane = cell_severity
                .map(|severity| table_severity_fill(Some(severity), row_index, palette))
                .unwrap_or(lane);
            if focused_cell {
                window.filled_rectangle(
                    layers,
                    0,
                    rect(cell_x + 2.0, y + 1.0, cell_px_width * 0.92, 3.0),
                    palette.amber,
                )?;
                window.filled_rectangle(
                    layers,
                    0,
                    rect(
                        cell_x + 2.0,
                        y + row_height - 4.0,
                        cell_px_width * 0.92,
                        3.0,
                    ),
                    palette.amber,
                )?;
                window.filled_rectangle(
                    layers,
                    0,
                    rect(cell_x + 2.0, y + 4.0, 4.0, row_height - 8.0),
                    palette.amber,
                )?;
            }
            if cell_severity.is_some() {
                window.filled_rectangle(
                    layers,
                    0,
                    rect(cell_x + 4.0, y + 3.0, cell_px_width * 0.72, 3.0),
                    with_alpha(cell_lane, 0.92),
                )?;
            }
            if cell_provenance.is_some() {
                window.filled_rectangle(
                    layers,
                    0,
                    rect(
                        cell_x + 4.0,
                        y + row_height - 3.0,
                        cell_px_width * 0.42,
                        2.0,
                    ),
                    with_alpha(palette.cyan, 0.76),
                )?;
            }
            let (text_x, default_text_color) = if cell_index == 0 {
                (cell_x + 24.0, RgbColor::new_8bpc(255, 204, 112))
            } else if cell_index + 1 == columns {
                (cell_x + 12.0, RgbColor::new_8bpc(153, 204, 255))
            } else {
                window.filled_rectangle(
                    layers,
                    0,
                    rect(cell_x + 4.0, y + 8.0, cell_px_width * 0.86, 2.0),
                    with_alpha(cell_lane, 0.74),
                )?;
                window.filled_rectangle(
                    layers,
                    0,
                    rect(
                        cell_x + 4.0,
                        y + row_height - 6.0,
                        cell_px_width * 0.68,
                        2.0,
                    ),
                    with_alpha(cell_lane, 0.82),
                )?;
                (
                    cell_x + 12.0,
                    table_severity_text(row.severity.as_deref(), cell_index),
                )
            };
            let text_color = cell_severity
                .map(|severity| table_severity_text(Some(severity), cell_index))
                .unwrap_or(default_text_color);
            window.paint_owt_panel_text(
                layers,
                text_x,
                y + ((row_height - cell_height) * 0.5).max(0.0),
                text_cols,
                &fit_text_ellipsis(cell, text_cols),
                text_color,
                true,
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
    if detail_count > 0 {
        let mut detail_y = y + height - cell_height - 10.0 - detail_stack_height;
        if let Some(group_detail) = &table.group_detail {
            paint_lcars_table_detail_band(
                window,
                layers,
                x,
                detail_y,
                width,
                detail_band_height,
                "GROUP DETAIL",
                group_detail,
                RgbColor::new_8bpc(203, 168, 255),
                RgbColor::new_8bpc(153, 204, 255),
                palette,
            )?;
            detail_y += detail_band_height + detail_gap;
        }
        if let Some(focused_detail) = &table.focused_detail {
            paint_lcars_table_detail_band(
                window,
                layers,
                x,
                detail_y,
                width,
                detail_band_height,
                "ROW DETAIL",
                focused_detail,
                RgbColor::new_8bpc(255, 204, 112),
                RgbColor::new_8bpc(153, 204, 255),
                palette,
            )?;
            detail_y += detail_band_height + detail_gap;
        }
        if let Some(cell_detail) = &table.cell_detail {
            paint_lcars_table_detail_band(
                window,
                layers,
                x,
                detail_y,
                width,
                detail_band_height,
                "CELL DETAIL",
                cell_detail,
                RgbColor::new_8bpc(153, 204, 255),
                RgbColor::new_8bpc(255, 204, 112),
                palette,
            )?;
            detail_y += detail_band_height + detail_gap;
        }
        if let Some(cell_focus) = &table.cell_focus {
            paint_lcars_table_detail_band(
                window,
                layers,
                x,
                detail_y,
                width,
                detail_band_height,
                "CELL FOCUS",
                &format!("{} | {}", cell_focus.label, cell_focus.detail),
                RgbColor::new_8bpc(255, 204, 112),
                RgbColor::new_8bpc(153, 204, 255),
                palette,
            )?;
        }
    }
    if let Some(overflow_text) = overflow_text {
        window.paint_owt_panel_text(
            layers,
            x + width - 108.0,
            y + height - cell_height - 4.0,
            14,
            &overflow_text,
            RgbColor::new_8bpc(255, 149, 96),
            true,
        )?;
    }
    if let Some(provenance) = &table.provenance {
        let provenance_cols = ((width - 150.0).max(cell_width * 8.0) / cell_width)
            .floor()
            .max(8.0) as usize;
        window.paint_owt_panel_text(
            layers,
            x + 34.0,
            y + height - cell_height - 4.0,
            provenance_cols,
            &fit_text_ellipsis(&format!("PROV {provenance}"), provenance_cols),
            RgbColor::new_8bpc(153, 204, 255),
            true,
        )?;
    }

    Ok(())
}

fn paint_lcars_table_detail_band(
    window: &mut crate::TermWindow,
    layers: &mut TripleLayerQuadAllocator,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    label: &str,
    detail: &str,
    label_color: RgbColor,
    detail_color: RgbColor,
    palette: LcarsPalette,
) -> anyhow::Result<()> {
    let cell_width = window.render_metrics.cell_size.width as f32;
    let label_cols = 13;
    let detail_cols = ((width - 184.0).max(cell_width * 8.0) / cell_width)
        .floor()
        .max(8.0) as usize;
    window.filled_rectangle(
        layers,
        0,
        rect(x + 30.0, y, width - 70.0, height),
        palette.black,
    )?;
    window.filled_rectangle(layers, 0, rect(x + 30.0, y, 14.0, height), palette.amber)?;
    window.filled_rectangle(
        layers,
        0,
        rect(x + 52.0, y + 4.0, (width * 0.18).max(46.0), 3.0),
        palette.peach,
    )?;
    window.filled_rectangle(
        layers,
        0,
        rect(x + width - 110.0, y + height - 6.0, 72.0, 3.0),
        palette.cyan,
    )?;
    window.paint_owt_panel_text(
        layers,
        x + 52.0,
        y + 8.0,
        label_cols,
        label,
        label_color,
        true,
    )?;
    window.paint_owt_panel_text(
        layers,
        x + 152.0,
        y + 8.0,
        detail_cols,
        &fit_text_ellipsis(&detail.to_uppercase(), detail_cols),
        detail_color,
        true,
    )?;
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

fn table_row_action_hitboxes(
    table: &LcarsTableData,
    table_left: f32,
    table_width: f32,
    row_y: f32,
    row_height: f32,
    row_gap: f32,
    visible_rows: usize,
) -> Vec<LcarsTableRowActionHitbox> {
    table
        .rows
        .iter()
        .take(visible_rows)
        .enumerate()
        .filter_map(|(row_index, row)| {
            let action_id = row.action_id.as_ref()?;
            let y = row_y + (row_index as f32 * (row_height + row_gap));
            Some(LcarsTableRowActionHitbox {
                action_id: action_id.clone(),
                x: table_left - 10.0,
                y: y - 2.0,
                width: table_width + 18.0,
                height: row_height + 4.0,
            })
        })
        .collect()
}

fn table_cell_focus_hitboxes(
    table: &LcarsTableData,
    table_left: f32,
    table_width: f32,
    row_y: f32,
    row_height: f32,
    row_gap: f32,
    visible_rows: usize,
    cell_width: f32,
) -> Vec<LcarsTableCellHitbox> {
    let columns = table.columns.len().max(1);
    table
        .rows
        .iter()
        .take(visible_rows)
        .enumerate()
        .flat_map(|(row_index, row)| {
            let y = row_y + (row_index as f32 * (row_height + row_gap));
            row.cells
                .iter()
                .enumerate()
                .take(columns)
                .filter_map(move |(cell_index, cell)| {
                    if cell.trim().is_empty() {
                        return None;
                    }
                    let column = table.columns.get(cell_index)?.clone();
                    let (cell_x, cell_cols) = table_cell_geometry(
                        table_left,
                        table_width,
                        columns,
                        cell_index,
                        cell_width,
                    );
                    Some(LcarsTableCellHitbox {
                        column,
                        x: cell_x + 2.0,
                        y: y + 1.0,
                        width: (cell_cols as f32 * cell_width - 8.0).max(cell_width * 3.0),
                        height: row_height - 2.0,
                    })
                })
                .collect::<Vec<_>>()
        })
        .collect()
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

fn table_severity_fill_byte(severity: Option<&str>, index: usize) -> LcarsByteColor {
    match severity.map(|value| value.to_ascii_lowercase()) {
        Some(value) if matches!(value.as_str(), "error" | "blocked" | "critical") => {
            LcarsByteColor::rgb(207, 79, 79)
        }
        Some(value) if matches!(value.as_str(), "warning" | "stale" | "legacy") => LCARS_BYTE_AMBER,
        Some(value) if matches!(value.as_str(), "success" | "ok" | "ready" | "current") => {
            LCARS_BYTE_BLUE
        }
        _ => match index % 4 {
            0 => LCARS_BYTE_BLUE,
            1 => LCARS_BYTE_VIOLET,
            2 => LCARS_BYTE_AMBER,
            _ => LCARS_BYTE_PEACH,
        },
    }
}

fn table_severity_text(severity: Option<&str>, cell_index: usize) -> RgbColor {
    match severity.map(|value| value.to_ascii_lowercase()) {
        Some(value) if matches!(value.as_str(), "error" | "blocked" | "critical") => {
            RgbColor::new_8bpc(207, 79, 79)
        }
        Some(value) if matches!(value.as_str(), "warning" | "stale" | "legacy") => {
            RgbColor::new_8bpc(255, 204, 112)
        }
        Some(value) if matches!(value.as_str(), "success" | "ok" | "ready" | "current") => {
            RgbColor::new_8bpc(153, 204, 255)
        }
        _ if cell_index == 0 => RgbColor::new_8bpc(255, 136, 0),
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
    hotkey: Option<usize>,
    palette: LcarsPalette,
) -> anyhow::Result<()> {
    let hovered = window.owt_lcars_rect_hovered(x, y, width, height);
    let accent_byte = if active {
        lcars_action_accent_byte(fill_byte, true)
    } else if hovered {
        fill_byte.with_alpha(255)
    } else {
        lcars_action_accent_byte(fill_byte, false)
    };
    let accent_fill = color(accent_byte.red, accent_byte.green, accent_byte.blue);
    let body_fill = if active || hovered { accent_fill } else { fill };
    let cell_width = window.render_metrics.cell_size.width as f32;
    let cell_height = window.render_metrics.cell_size.height as f32;
    let pressed_offset = if active {
        2.0
    } else if hovered {
        1.0
    } else {
        0.0
    };
    let tab_width = (height * 1.22).clamp(38.0, 58.0).min(width * 0.30);
    let tab_gap = 8.0;
    let cap_width = (height * 1.04).clamp(34.0, 54.0).min(width * 0.24);
    let body_x = x + tab_width + tab_gap;
    let body_y = y + pressed_offset;
    let body_height = (height - (pressed_offset * 2.0)).max(8.0);
    let body_width = (width - tab_width - tab_gap).max(1.0);
    let cap_x = x + width - cap_width;
    let slab_width = (body_width - (cap_width * 0.46)).max(cell_width * 6.0);
    let label_strip_x = body_x + 12.0;
    let label_right = (cap_x - 12.0).max(label_strip_x + cell_width * 3.0);
    let label_strip_width = (label_right - label_strip_x).max(32.0);
    let label_strip_height = (cell_height + 5.0)
        .min((body_height - 10.0).max(cell_height))
        .max(cell_height);
    let label_strip_y = body_y + ((body_height - label_strip_height) * 0.5).max(0.0);

    window.filled_rectangle(layers, 0, rect(x, y, width, height), palette.black)?;
    paint_lcars_cast_shadow(
        window,
        layers,
        x,
        body_y,
        width,
        body_height,
        active,
        hovered,
    )?;
    if active {
        window.filled_rectangle(
            layers,
            0,
            rect(x - 5.0, y + 3.0, 4.0, height - 4.0),
            palette.amber,
        )?;
        window.filled_rectangle(
            layers,
            0,
            rect(x - 1.0, y + height - 4.0, width * 0.54, 3.0),
            palette.peach,
        )?;
    } else if hovered {
        window.filled_rectangle(
            layers,
            0,
            rect(x - 3.0, y + 5.0, 3.0, height - 10.0),
            palette.violet,
        )?;
        window.filled_rectangle(
            layers,
            0,
            rect(x + tab_width + tab_gap, y + height - 4.0, width * 0.38, 2.0),
            palette.dim_blue,
        )?;
    }
    paint_lcars_left_cap_bar(
        window,
        layers,
        x,
        body_y,
        tab_width,
        body_height,
        if active {
            LCARS_BYTE_PEACH
        } else if hovered {
            fill_byte.with_alpha(255)
        } else {
            fill_byte.with_alpha(236)
        },
    )?;
    paint_lcars_raised_slab_edges(
        window,
        layers,
        x,
        body_y,
        tab_width,
        body_height,
        fill_byte,
        active,
        hovered,
    )?;
    window.filled_rectangle(
        layers,
        0,
        rect(x + tab_width, y, tab_gap, height),
        palette.black,
    )?;
    window.filled_rectangle(
        layers,
        0,
        rect(body_x, body_y, body_width, body_height),
        body_fill,
    )?;
    paint_lcars_raised_slab_edges(
        window,
        layers,
        body_x,
        body_y,
        body_width,
        body_height,
        accent_byte,
        active,
        hovered,
    )?;
    let lower_rule_width = if active {
        slab_width * 0.56
    } else if hovered {
        slab_width * 0.46
    } else {
        slab_width * 0.34
    };
    window.filled_rectangle(
        layers,
        0,
        rect(body_x, body_y + body_height - 6.0, lower_rule_width, 2.0),
        palette.black,
    )?;
    paint_lcars_right_cap_bar(
        window,
        layers,
        cap_x,
        body_y,
        cap_width,
        body_height,
        accent_byte,
    )?;
    paint_lcars_raised_slab_edges(
        window,
        layers,
        cap_x,
        body_y,
        cap_width,
        body_height,
        accent_byte,
        active,
        hovered,
    )?;
    if active || hovered {
        window.filled_rectangle(
            layers,
            0,
            rect(
                cap_x + 6.0,
                body_y + body_height - 9.0,
                cap_width - 12.0,
                if active { 4.0 } else { 3.0 },
            ),
            palette.black,
        )?;
    }
    paint_lcars_recessed_bay_depth(
        window,
        layers,
        label_strip_x,
        label_strip_y,
        label_strip_width,
        label_strip_height,
        accent_byte,
    )?;

    if let Some(hotkey) = hotkey {
        let hotkey_text = format!("{hotkey:02}");
        let hotkey_cols = (tab_width / cell_width).floor().max(1.0) as usize;
        window.paint_owt_panel_text(
            layers,
            x + 5.0,
            body_y + ((body_height - cell_height) * 0.5).max(0.0),
            hotkey_cols,
            &hotkey_text,
            RgbColor::new_8bpc(0, 0, 0),
            true,
        )?;
    }

    let label_cols = (label_strip_width / cell_width).floor().max(1.0) as usize;
    let visible_label_cols = label_cols.min(text_cols.max(1));
    let visible_label = fit_text_ellipsis(text, visible_label_cols);
    window.paint_owt_panel_text(
        layers,
        label_strip_x + 6.0,
        label_strip_y + ((label_strip_height - cell_height) * 0.5).max(0.0),
        visible_label_cols,
        &visible_label,
        if active {
            RgbColor::new_8bpc(255, 240, 176)
        } else if hovered {
            RgbColor::new_8bpc(255, 204, 112)
        } else {
            text_fg
        },
        true,
    )?;
    Ok(())
}

fn paint_lcars_left_cap_bar(
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
        raster.fill_rounded_rect(0.0, 0.0, height, height, radius, fill);
        raster.fill_rect(radius, 0.0, width - radius, height, fill);
    })
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

fn clamp_to_available(value: f32, preferred_min: f32, available_max: f32) -> f32 {
    let max = available_max.max(1.0);
    let min = preferred_min.min(max);
    value.clamp(min, max)
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

fn lcars_compact_menu_label(text: &str) -> String {
    let normalized = text
        .replace(['\r', '\n', '_', '-', '.', '/'], " ")
        .split_whitespace()
        .next()
        .unwrap_or("LCARS")
        .to_uppercase();
    fit_text_ellipsis(&normalized, 9)
}

fn lcars_saved_interface_menu_detail(saved: &crate::owt_native::SavedInterfaceSummary) -> String {
    let interface = saved
        .interface_id
        .as_deref()
        .unwrap_or(saved.store_id.as_str());
    match saved.owner_label.as_deref() {
        Some(owner) if !owner.is_empty() => format!("{owner} / {interface}"),
        _ => interface.to_string(),
    }
}

fn lcars_saved_owner_options(
    saved_interfaces: &[crate::owt_native::SavedInterfaceSummary],
) -> Vec<LcarsSavedOwnerOption> {
    let mut owners = BTreeMap::<String, String>::new();
    for saved in saved_interfaces {
        let Some(key) = saved.owner_key.as_deref().filter(|key| !key.is_empty()) else {
            continue;
        };
        let label = saved
            .owner_label
            .as_deref()
            .filter(|label| !label.is_empty())
            .unwrap_or(key);
        owners
            .entry(key.to_string())
            .or_insert_with(|| label.to_string());
    }
    owners
        .into_iter()
        .map(|(key, label)| LcarsSavedOwnerOption { key, label })
        .collect()
}

fn lcars_saved_visible_interfaces<'a>(
    state: &'a OwtLcarsSurfaceMenuState,
) -> Vec<&'a crate::owt_native::SavedInterfaceSummary> {
    match state.saved_owner_filter.as_deref() {
        Some(filter) => state
            .saved_interfaces
            .iter()
            .filter(|saved| saved.owner_key.as_deref() == Some(filter))
            .collect(),
        None => state.saved_interfaces.iter().collect(),
    }
}

fn lcars_saved_owner_filter_detail(state: &OwtLcarsSurfaceMenuState) -> String {
    match state.saved_owner_filter.as_deref() {
        Some(filter) => lcars_saved_owner_options(&state.saved_interfaces)
            .into_iter()
            .find(|owner| owner.key == filter)
            .map(|owner| format!("FILTER {}", owner.label))
            .unwrap_or_else(|| "FILTER UNKNOWN OWNER".to_string()),
        None => "FILTER ALL OWNERS".to_string(),
    }
}

fn lcars_next_saved_owner_filter(
    saved_interfaces: &[crate::owt_native::SavedInterfaceSummary],
    current_filter: Option<&str>,
) -> Option<String> {
    let owners = lcars_saved_owner_options(saved_interfaces);
    if owners.len() <= 1 {
        return None;
    }
    match current_filter {
        None => Some(owners[0].key.clone()),
        Some(current) => owners
            .iter()
            .position(|owner| owner.key == current)
            .and_then(|index| owners.get(index + 1))
            .map(|owner| owner.key.clone()),
    }
}

fn lcars_saved_menu_status(state: &OwtLcarsSurfaceMenuState) -> String {
    let owner_count = lcars_saved_owner_options(&state.saved_interfaces).len();
    let visible_count = lcars_saved_visible_interfaces(state).len();
    match (owner_count, state.saved_owner_filter.as_deref()) {
        (0, _) => "LOAD LOCAL PROFILE INTERFACE".to_string(),
        (_, Some(filter)) => {
            let owner_label = lcars_saved_owner_options(&state.saved_interfaces)
                .into_iter()
                .find(|owner| owner.key == filter)
                .map(|owner| owner.label)
                .unwrap_or_else(|| "UNKNOWN OWNER".to_string());
            format!("LOAD OWNER {owner_label} / ITEMS {visible_count:02}")
        }
        (_, None) => {
            format!("LOAD LOCAL PROFILE INTERFACE / OWNERS {owner_count:02}")
        }
    }
}

fn lcars_scope_owner_label(scope: &Scope) -> String {
    format!(
        "{} {}",
        lcars_scope_kind_label(&scope.kind),
        lcars_compact_owner_id(&scope.id)
    )
}

fn lcars_scope_kind_label(kind: &ScopeKind) -> &'static str {
    match kind {
        ScopeKind::Pane => "PANE",
        ScopeKind::Tab => "TAB",
        ScopeKind::Window => "WINDOW",
        ScopeKind::Workspace => "WORKSPACE",
        ScopeKind::Project => "PROJECT",
        ScopeKind::Session => "SESSION",
    }
}

fn lcars_compact_owner_id(id: &str) -> String {
    let trimmed = id.trim().trim_end_matches(['/', '\\']);
    fit_text_ellipsis(
        trimmed
            .rsplit(['/', '\\', ':'])
            .find(|part| !part.trim().is_empty())
            .unwrap_or(trimmed),
        28,
    )
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

fn owt_lcars_design_grid_enabled() -> bool {
    std::env::var("OWT_LCARS_DESIGN_GRID")
        .map(|value| {
            let value = value.trim().to_ascii_lowercase();
            matches!(value.as_str(), "1" | "true" | "yes" | "on")
        })
        .unwrap_or(false)
}

fn paint_lcars_design_grid(
    window: &mut crate::TermWindow,
    layers: &mut TripleLayerQuadAllocator,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
) -> anyhow::Result<()> {
    let cyan = with_alpha(color(48, 170, 255), 0.88);
    let amber = with_alpha(color(255, 204, 112), 0.82);
    let mut gx = x;
    while gx <= x + width {
        window.filled_rectangle(layers, 0, rect(gx, y, 1.0, height), cyan)?;
        gx += 32.0;
    }
    let mut gy = y;
    while gy <= y + height {
        window.filled_rectangle(layers, 0, rect(x, gy, width, 1.0), amber)?;
        gy += 32.0;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        assign_lcars_hotkeys, clamp_to_available, collect_panel_items,
        compute_lcars_structural_scene, compute_thelcars_cockpit_grid,
        describe_lcars_render_projection, has_structural_lcars_layout, lcars_action_accent_byte,
        lcars_action_fill_byte, lcars_action_strip_mode,
        lcars_action_strip_side_width_from_metrics, lcars_action_strip_vertical_rail,
        lcars_block_composition_data, lcars_compact_button_reserved_pixels,
        lcars_corner_button_mode, lcars_depth_metrics, lcars_keyboard_action_slots,
        lcars_next_saved_owner_filter, lcars_palette_profile, lcars_primitive_legend_rows,
        lcars_render_scene_badge_lines, lcars_render_scene_summary,
        lcars_saved_interface_menu_detail, lcars_saved_menu_status,
        lcars_saved_owner_filter_detail, lcars_scope_owner_label, lcars_structural_breakpoint,
        lcars_structural_mode, lcars_surface_menu_properties, lcars_surface_menu_slot_rects,
        lcars_surface_placement, lcars_surface_visible, lcars_table_activation_key,
        lcars_table_cell_focus_movement, lcars_table_data, lcars_table_focus_mode_key,
        lcars_table_focus_movement, node_line, node_provenance_label,
        owt_lcars_drag_drop_layout_properties, owt_lcars_drag_drop_target_text,
        owt_lcars_native_surface_viewport_ready, owt_lcars_resize_layout_properties,
        parse_lcars_panel_layout, parse_lcars_surface_origin, plan_lcars_structural_console,
        prioritized_side_panel_signal_lines, structural_signal_text_cols,
        structural_signal_text_needs_backing, structural_signal_visual_limit,
        table_cell_focus_hitboxes, table_group_summary_label, table_row_action_hitboxes,
        thelcars_adaptive_layout_label, thelcars_cockpit_detail_text, thelcars_demo_metadata,
        thelcars_metric_count, thelcars_navigation_labels, thelcars_signal_lines,
        thelcars_theme_labels, wrap_text_lines, LcarsDetailPlacement, LcarsPaletteProfile,
        LcarsPanelLayout, LcarsSceneRect, LcarsStructuralBreakpoint, LcarsStructuralMode,
        LcarsSurfaceMenuAction, LcarsSurfaceOrientation, LcarsSurfaceOrigin, LcarsSurfacePlacement,
        LcarsSurfaceReservation, LcarsTableGroupSummary, PanelLine, PanelLineKind,
        TheLcarsCockpitDensity, TheLcarsDemoMetadata, LCARS_BOTTOM_PANEL_MIN_HEIGHT,
        LCARS_PANEL_GAP, LCARS_PANEL_MARGIN, LCARS_SIDE_PANEL_MIN_WIDTH,
    };
    use crate::termwindow::{OwtLcarsSurfaceMenuMode, OwtLcarsSurfaceMenuState};
    use owt_control::{
        ActionKind, FactProvenance, FactState, InterfaceDocument, Scope, ScopeKind,
        TableCellFocusMovement, TableFocusMovement, UiAction, UiNode, UiNodeKind,
    };
    use window::{KeyCode, PhysKeyCode};

    fn rects_overlap(a: LcarsSceneRect, b: LcarsSceneRect) -> bool {
        a.x < b.right() && a.right() > b.x && a.y < b.bottom() && a.bottom() > b.y
    }

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
        let no_actions = plan_lcars_structural_console(
            120.0,
            1500.0,
            90.0,
            320.0,
            10.0,
            20.0,
            true,
            false,
            0,
            LcarsStructuralMode::Composition,
        );
        let with_actions = plan_lcars_structural_console(
            120.0,
            1500.0,
            90.0,
            320.0,
            10.0,
            20.0,
            true,
            false,
            4,
            LcarsStructuralMode::Composition,
        );

        assert!(!no_actions.command_visible);
        assert_eq!(no_actions.action_slots, 0);
        assert_eq!(no_actions.command_width, 0.0);
        assert!(with_actions.command_visible);
        assert!(with_actions.action_slots > 0);
        assert!(no_actions.detail_width >= with_actions.detail_width);
    }

    #[test]
    fn structural_breakpoints_keep_compact_command_banks_small() {
        let compact = plan_lcars_structural_console(
            120.0,
            820.0,
            90.0,
            320.0,
            10.0,
            20.0,
            false,
            true,
            4,
            LcarsStructuralMode::Composition,
        );
        let wide = plan_lcars_structural_console(
            120.0,
            1800.0,
            90.0,
            320.0,
            10.0,
            20.0,
            false,
            true,
            4,
            LcarsStructuralMode::Composition,
        );

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
        assert!(!wide.two_action_columns);
        assert!(wide.command_width > compact.command_width);
        assert!(wide.detail_width > compact.detail_width);
    }

    #[test]
    fn stacked_detail_reserves_signal_overflow_guard() {
        let stacked = plan_lcars_structural_console(
            120.0,
            720.0,
            90.0,
            320.0,
            10.0,
            20.0,
            false,
            true,
            4,
            LcarsStructuralMode::Composition,
        );

        assert_eq!(stacked.detail, LcarsDetailPlacement::Stacked);
        let visual_limit = structural_signal_visual_limit(&stacked, 320.0, 20.0);
        assert!(visual_limit < stacked.detail_top);
        assert!(visual_limit <= stacked.detail_top - 7.0);
    }

    #[test]
    fn computed_structural_scene_protects_labels_and_terminal_bay() {
        let scene = compute_lcars_structural_scene(
            120.0,
            1500.0,
            40.0,
            430.0,
            900.0,
            10.0,
            20.0,
            true,
            false,
            5,
            LcarsStructuralMode::Composition,
        );

        assert!(scene.scope_tab.width >= 300.0);
        assert!(scene.content_bay.height >= 92.0);
        assert!(scene.content_label.y < scene.content_bay.bottom());
        assert!(scene.status_tab.x > scene.content_label.x);
        let command_bank = scene.command_bank.expect("command bank scene");
        assert!(command_bank.frame.width >= scene.plan.command_width);
        assert!(command_bank.button_top > command_bank.frame.y);
        assert!(command_bank.rows > 0);
    }

    #[test]
    fn primitive_legend_mode_keeps_every_node_kind_visible() {
        let document = primitive_legend_document();

        assert_eq!(
            lcars_structural_mode(&document),
            LcarsStructuralMode::PrimitiveLegend
        );
        let rows = lcars_primitive_legend_rows(&document);

        assert_eq!(rows.len(), 20);
        assert!(rows.iter().all(|row| row.present));
        assert_eq!(rows[0].label, "PANEL");
        assert_eq!(rows[10].label, "COMMAND_GRID");
        assert_eq!(rows[15].label, "TABLE");
        assert_eq!(rows[19].label, "SPACER");
    }

    #[test]
    fn primitive_legend_planning_suppresses_structural_detail_promotion() {
        let primitive = plan_lcars_structural_console(
            120.0,
            1500.0,
            90.0,
            320.0,
            10.0,
            20.0,
            true,
            true,
            1,
            LcarsStructuralMode::PrimitiveLegend,
        );
        let composition = plan_lcars_structural_console(
            120.0,
            1500.0,
            90.0,
            320.0,
            10.0,
            20.0,
            true,
            true,
            4,
            LcarsStructuralMode::Composition,
        );

        assert_eq!(primitive.detail, LcarsDetailPlacement::None);
        assert!(primitive.command_width <= 260.0);
        assert_ne!(composition.detail, LcarsDetailPlacement::None);
    }

    #[test]
    fn block_composition_mode_extracts_labelled_data_and_graphics() {
        let document = block_composition_document();

        assert_eq!(
            lcars_structural_mode(&document),
            LcarsStructuralMode::BlockComposition
        );
        let data = lcars_block_composition_data(&document);

        assert_eq!(data.title, "SCAN CONTROL");
        assert_eq!(data.boxes.len(), 2);
        assert_eq!(data.boxes[0].label, "RANGE WINDOW");
        assert_eq!(data.boxes[0].value, "28741");
        assert_eq!(data.boxes[1].label, "FREQUENCY BAND");
        assert_eq!(data.boxes[1].value, "581257-365");
        assert_eq!(data.graphics_title, "LONG RANGE SCAN");
        assert_eq!(data.graphics_detail, "ZOOM VECTOR READY");
        assert!(data.has_graphics);
        assert_eq!(data.command_bank_title, "COMMAND BANK");
    }

    #[test]
    fn thelcars_control_panel_profile_routes_to_dedicated_mode() {
        let mut document = InterfaceDocument::new(
            "lcars.test.thelcars_control_panel",
            "TheLCARS Control Panel",
            Scope::new(ScopeKind::Project, "/tmp/owt-thelcars"),
        );
        document.theme = Some("lcars".to_string());

        let mut root = UiNode::new("thelcars-root", UiNodeKind::Panel);
        root.properties
            .insert("profile".to_string(), "thelcars_control_panel".to_string());
        document.nodes.push(root);

        assert_eq!(
            lcars_structural_mode(&document),
            LcarsStructuralMode::TheLcarsControlPanel
        );
        assert!(has_structural_lcars_layout(&document));
    }

    #[test]
    fn thelcars_private_demo_profile_alias_routes_to_dedicated_mode() {
        let mut document = InterfaceDocument::new(
            "lcars.test.thelcars_private_demo",
            "TheLCARS Private Demo",
            Scope::new(ScopeKind::Project, "/tmp/owt-thelcars-private"),
        );
        document.theme = Some("lcars".to_string());

        let mut root = UiNode::new("thelcars-root", UiNodeKind::Panel);
        root.properties.insert(
            "information_shape".to_string(),
            "thelcars_private_demo".to_string(),
        );
        document.nodes.push(root);

        assert_eq!(
            lcars_structural_mode(&document),
            LcarsStructuralMode::TheLcarsControlPanel
        );
        assert!(has_structural_lcars_layout(&document));
    }

    #[test]
    fn readable_shell_profile_aliases_route_to_docked_surface() {
        for alias in [
            "daily_shell",
            "lcars_shell",
            "operator_shell",
            "readable_shell",
            "terminal_shell",
            "owt_shell",
        ] {
            let mut document = InterfaceDocument::new(
                format!("lcars.test.{alias}"),
                "LCARS Shell",
                Scope::new(ScopeKind::Project, "/tmp/owt-lcars-shell"),
            );
            document.theme = Some("lcars".to_string());

            let mut root = UiNode::new("shell-root", UiNodeKind::Panel);
            root.properties
                .insert("profile".to_string(), alias.to_string());
            document.nodes.push(root);

            assert_eq!(
                lcars_structural_mode(&document),
                LcarsStructuralMode::DockedSurface
            );
            assert!(has_structural_lcars_layout(&document));
        }

        let scene = compute_lcars_structural_scene(
            180.0,
            1760.0,
            60.0,
            430.0,
            900.0,
            10.0,
            20.0,
            false,
            false,
            3,
            LcarsStructuralMode::DockedSurface,
        );
        let command_bank = scene.command_bank.expect("shell command bank scene");

        assert!(scene.plan.signal_rows <= 3);
        assert!(command_bank.button_height >= 42.0);
    }

    #[test]
    fn information_shape_profiles_route_to_structural_composition() {
        for shape in [
            "fleet_matrix",
            "incident_summary",
            "queue_triage",
            "project_status",
            "artifact_browser",
        ] {
            let mut document = InterfaceDocument::new(
                format!("lcars.test.{shape}"),
                "LCARS Information Shape",
                Scope::new(ScopeKind::Project, "/tmp/owt-lcars-shape"),
            );
            document.theme = Some("lcars".to_string());

            let mut root = UiNode::new("shape-root", UiNodeKind::Panel);
            root.properties
                .insert("information_shape".to_string(), shape.to_string());
            document.nodes.push(root);

            let projection = describe_lcars_render_projection(&document);
            assert!(has_structural_lcars_layout(&document));
            assert_eq!(
                lcars_structural_mode(&document),
                LcarsStructuralMode::Composition
            );
            assert_eq!(projection.renderer_path, "structural_lcars");
            assert_eq!(projection.structural_profile, Some("composition"));
            assert_eq!(projection.requested_profile.as_deref(), Some(shape));
        }
    }

    #[test]
    fn action_strip_profile_routes_to_minimal_renderer() {
        let mut document = InterfaceDocument::new(
            "lcars.test.action_strip",
            "Folder Buttons",
            Scope::new(ScopeKind::Session, "session.folders"),
        );
        document.theme = Some("lcars".to_string());

        let mut root = UiNode::new("root", UiNodeKind::Panel);
        root.properties
            .insert("profile".to_string(), "action_strip".to_string());
        let mut button = UiNode::new("button.open", UiNodeKind::Button);
        button.action_id = Some("folders.open.tools".to_string());
        root.children.push(button);
        document.nodes.push(root);

        let projection = describe_lcars_render_projection(&document);
        assert!(lcars_action_strip_mode(&document));
        assert_eq!(projection.renderer_path, "action_strip");
        assert!(!has_structural_lcars_layout(&document));
        assert_eq!(
            lcars_keyboard_action_slots(&document),
            vec!["folders.open.tools".to_string()]
        );
    }

    #[test]
    fn action_strip_left_rail_uses_vertical_stack_branch() {
        let mut document = InterfaceDocument::new(
            "lcars.test.action_strip.left",
            "Folder Buttons",
            Scope::new(ScopeKind::Session, "session.folders"),
        );
        document.theme = Some("lcars".to_string());

        let mut root = UiNode::new("root", UiNodeKind::Panel);
        root.properties
            .insert("profile".to_string(), "action_strip".to_string());
        root.properties
            .insert("layout".to_string(), "left_rail".to_string());
        let mut first = UiNode::new("button.first", UiNodeKind::Button);
        first.action_id = Some("folders.open.first".to_string());
        let mut second = UiNode::new("button.second", UiNodeKind::Button);
        second.action_id = Some("folders.open.second".to_string());
        root.children.push(first);
        root.children.push(second);
        document.nodes.push(root);

        let projection = describe_lcars_render_projection(&document);
        let placement = lcars_surface_placement(&document);

        assert!(lcars_action_strip_mode(&document));
        assert_eq!(projection.renderer_path, "action_strip");
        assert_eq!(projection.layout, "left_rail");
        assert_eq!(projection.orientation, "vertical");
        assert!(lcars_action_strip_vertical_rail(placement));
    }

    #[test]
    fn compact_bottom_button_reservation_stays_button_sized() {
        let reserved = lcars_compact_button_reserved_pixels(34.0);

        assert!(reserved < LCARS_BOTTOM_PANEL_MIN_HEIGHT);
        assert_eq!(reserved, LCARS_PANEL_MARGIN + 8.0 + 34.0 + LCARS_PANEL_GAP);
    }

    #[test]
    fn action_strip_side_width_is_content_sized() {
        let width = lcars_action_strip_side_width_from_metrics(9.0, 10, None, 1400.0)
            .expect("side action strip width");

        assert!(width < LCARS_SIDE_PANEL_MIN_WIDTH);
        assert!(width >= 220.0);
    }

    #[test]
    fn corner_button_profile_defaults_to_reserved_top_button() {
        let mut document = InterfaceDocument::new(
            "lcars.test.corner_button",
            "SINGLES RENDERS",
            Scope::new(ScopeKind::Project, "/tmp/owt-corner-button"),
        );
        document.theme = Some("lcars".to_string());
        document.actions.push(UiAction::new(
            "cycles.carriersingles.renders.open",
            "SINGLES RENDERS",
            ActionKind::Open,
        ));

        let mut root = UiNode::new("root", UiNodeKind::Panel);
        root.properties
            .insert("layout".to_string(), "overlay".to_string());
        root.properties
            .insert("profile".to_string(), "corner_button".to_string());
        root.properties.insert(
            "information_shape".to_string(),
            "cycle_render_shortcut".to_string(),
        );
        let mut frame = UiNode::new("frame.decorative", UiNodeKind::Frame);
        frame
            .children
            .push(UiNode::new("bars.decorative", UiNodeKind::BarRun));
        let mut button = UiNode::new("button.open", UiNodeKind::Button);
        button.action_id = Some("cycles.carriersingles.renders.open".to_string());
        root.children.push(frame);
        root.children.push(button);
        document.nodes.push(root);

        let projection = describe_lcars_render_projection(&document);
        assert!(lcars_corner_button_mode(&document));
        assert_eq!(projection.renderer_path, "corner_button");
        assert_eq!(projection.layout, "top");
        assert_eq!(projection.origin, "top_left");
        assert_eq!(projection.reservation, "reserved");
        assert!(projection.reserves_terminal_space);
        assert_eq!(projection.floating_x, None);
        assert_eq!(projection.floating_y, None);
        assert!(!has_structural_lcars_layout(&document));
        assert_eq!(projection.structural_profile, None);
        assert_eq!(
            lcars_keyboard_action_slots(&document),
            vec!["cycles.carriersingles.renders.open".to_string()]
        );
    }

    #[test]
    fn corner_button_overlay_requires_explicit_terminal_overlay_opt_in() {
        let mut document = InterfaceDocument::new(
            "lcars.test.corner_button.overlay",
            "SINGLES RENDERS",
            Scope::new(ScopeKind::Project, "/tmp/owt-corner-button-overlay"),
        );
        document.theme = Some("lcars".to_string());
        document.actions.push(UiAction::new(
            "cycles.carriersingles.renders.open",
            "SINGLES RENDERS",
            ActionKind::Open,
        ));

        let mut root = UiNode::new("root", UiNodeKind::Panel);
        root.properties
            .insert("layout".to_string(), "overlay".to_string());
        root.properties
            .insert("reservation".to_string(), "overlay".to_string());
        root.properties
            .insert("profile".to_string(), "corner_button".to_string());
        root.properties
            .insert("allow_terminal_overlay".to_string(), "true".to_string());
        root.properties
            .insert("floating_anchor".to_string(), "center".to_string());
        root.properties
            .insert("floating_x".to_string(), "640".to_string());
        root.properties
            .insert("floating_y".to_string(), "360".to_string());
        root.properties
            .insert("floating_width".to_string(), "220".to_string());
        root.properties
            .insert("floating_height".to_string(), "42".to_string());
        let mut button = UiNode::new("button.open", UiNodeKind::Button);
        button.action_id = Some("cycles.carriersingles.renders.open".to_string());
        root.children.push(button);
        document.nodes.push(root);

        let projection = describe_lcars_render_projection(&document);

        assert_eq!(projection.renderer_path, "corner_button");
        assert_eq!(projection.layout, "overlay");
        assert_eq!(projection.reservation, "overlay");
        assert!(!projection.reserves_terminal_space);
        assert_eq!(projection.floating_anchor.as_deref(), Some("center"));
        assert_eq!(projection.floating_x.as_deref(), Some("640"));
        assert_eq!(projection.floating_y.as_deref(), Some("360"));
        assert_eq!(projection.floating_width.as_deref(), Some("220"));
        assert_eq!(projection.floating_height.as_deref(), Some("42"));
    }

    #[test]
    fn thelcars_private_demo_metadata_reads_local_template_metrics() {
        let mut document = InterfaceDocument::new(
            "lcars.test.thelcars_private_demo",
            "TheLCARS Private Demo",
            Scope::new(ScopeKind::Project, "/tmp/owt-thelcars-private"),
        );
        document.theme = Some("lcars".to_string());

        let mut root = UiNode::new("thelcars-root", UiNodeKind::Panel);
        root.properties.insert(
            "information_shape".to_string(),
            "thelcars_private_demo".to_string(),
        );
        root.properties.insert(
            "private_template_status".to_string(),
            "detected".to_string(),
        );
        root.properties.insert(
            "private_theme_variants".to_string(),
            "Classic, Nemesis Blue, Lower Decks".to_string(),
        );
        root.properties.insert(
            "private_bar_rhythm".to_string(),
            "bar-1-6-width=40%; bar-2-7-width=4%".to_string(),
        );
        document.nodes.push(root);

        let metadata = thelcars_demo_metadata(&document);

        assert_eq!(metadata.template_status, "DETECTED");
        assert_eq!(
            thelcars_theme_labels(&metadata.themes),
            vec!["CLASSIC", "NEMESIS BLUE", "LOWER DECKS"]
        );
        assert!(metadata.bar_rhythm.contains("40%"));
    }

    #[test]
    fn thelcars_private_demo_counts_metrics_without_leaking_values() {
        let metadata = TheLcarsDemoMetadata {
            template_status: "DETECTED".to_string(),
            themes: "Classic, Ultra Layout".to_string(),
            palette_roles: "orange=#f80; blue=#56f; ice=#9cf".to_string(),
            frame_metrics: "lfw=240px; nav-width=240px".to_string(),
            bar_rhythm: "bar-1-6-width=40%; bar-2-7-width=4%".to_string(),
        };

        assert_eq!(thelcars_metric_count(&metadata.palette_roles), 3);
        assert_eq!(thelcars_metric_count(&metadata.frame_metrics), 2);
        assert_eq!(thelcars_metric_count("none detected"), 0);
        assert_eq!(
            thelcars_adaptive_layout_label(&metadata),
            "ULTRA TO ADAPTIVE"
        );

        let detail = thelcars_cockpit_detail_text(&metadata);

        assert!(detail.contains("NATIVE COCKPIT"));
        assert!(detail.contains("THEMES 02"));
        assert!(detail.contains("PAL 03"));
        assert!(detail.contains("FRAME 02"));
        assert!(detail.contains("BAR 02"));
        assert!(detail.contains("ULTRA TO ADAPTIVE"));
        assert!(!detail.contains("#f80"));
        assert!(!detail.contains("240px"));
        assert!(!detail.contains("40%"));
    }

    #[test]
    fn thelcars_cockpit_grid_redistributes_across_anchors() {
        let top = compute_thelcars_cockpit_grid(
            LcarsSceneRect {
                x: 8.0,
                y: 48.0,
                width: 1800.0,
                height: 520.0,
            },
            LcarsSurfacePlacement::from_layout(LcarsPanelLayout::Top),
            10.0,
            20.0,
            3,
            8,
        )
        .expect("top cockpit grid");
        assert_eq!(top.density, TheLcarsCockpitDensity::Wide);
        assert!(top.secondary_bay.is_some());
        assert!(top.command_stack.is_some());
        assert!(top.primary_bay.right() < top.command_stack.unwrap().x);

        let left = compute_thelcars_cockpit_grid(
            LcarsSceneRect {
                x: 8.0,
                y: 48.0,
                width: 420.0,
                height: 760.0,
            },
            LcarsSurfacePlacement::from_layout(LcarsPanelLayout::Left),
            10.0,
            20.0,
            3,
            8,
        )
        .expect("left cockpit grid");
        assert_eq!(left.placement.layout, LcarsPanelLayout::Left);
        assert!(left.secondary_bay.is_none());
        assert!(left.primary_bay.x > left.rail.right());
        assert!(left.command_stack.unwrap().y > left.primary_bay.y);

        let bottom = compute_thelcars_cockpit_grid(
            LcarsSceneRect {
                x: 8.0,
                y: 660.0,
                width: 1800.0,
                height: 260.0,
            },
            LcarsSurfacePlacement::from_origin(LcarsSurfaceOrigin::BottomRight),
            10.0,
            20.0,
            2,
            5,
        )
        .expect("bottom cockpit grid");
        assert_eq!(bottom.density, TheLcarsCockpitDensity::Strip);
        assert_eq!(bottom.placement.origin, LcarsSurfaceOrigin::BottomRight);
        assert!(bottom.command_stack.is_some());
        assert!(bottom.primary_bay.right() < bottom.command_stack.unwrap().x);
    }

    #[test]
    fn thelcars_cockpit_grid_collapses_secondary_when_narrow() {
        let grid = compute_thelcars_cockpit_grid(
            LcarsSceneRect {
                x: 8.0,
                y: 48.0,
                width: 760.0,
                height: 360.0,
            },
            LcarsSurfacePlacement::from_layout(LcarsPanelLayout::Top),
            10.0,
            20.0,
            3,
            8,
        )
        .expect("compact cockpit grid");

        assert_eq!(grid.density, TheLcarsCockpitDensity::Compact);
        assert!(grid.secondary_bay.is_none());
        assert!(grid.command_stack.is_none());
        assert!(grid.primary_bay.width >= grid.scope_tab.width * 0.72);
    }

    #[test]
    fn thelcars_cockpit_detail_text_hides_raw_template_metadata() {
        let mut document = InterfaceDocument::new(
            "lcars.test.thelcars_detail",
            "TheLCARS Detail",
            Scope::new(ScopeKind::Project, "/tmp/owt-thelcars-detail"),
        );
        document.theme = Some("lcars".to_string());
        let mut root = UiNode::new("thelcars-root", UiNodeKind::Panel);
        root.properties.insert(
            "private_template_status".to_string(),
            "detected".to_string(),
        );
        root.properties.insert(
            "private_theme_variants".to_string(),
            "Classic, Ultra".to_string(),
        );
        root.properties.insert(
            "private_palette_roles".to_string(),
            "orange=#f80".to_string(),
        );
        root.properties
            .insert("private_frame_metrics".to_string(), "lfw=240px".to_string());
        root.properties.insert(
            "private_bar_rhythm".to_string(),
            "bar-1-6-width=40%".to_string(),
        );
        document.nodes.push(root);

        let detail = thelcars_cockpit_detail_text(&thelcars_demo_metadata(&document));

        assert!(detail.contains("NATIVE COCKPIT"));
        assert!(detail.contains("THEMES 02"));
        assert!(detail.contains("PAL 01"));
        assert!(detail.contains("FRAME 01"));
        assert!(detail.contains("BAR 01"));
        assert!(!detail.contains("orange="));
        assert!(!detail.contains("lfw="));
        assert!(!detail.contains("bar-1"));
        assert!(!detail.contains("40%"));
    }

    #[test]
    fn builder_highlight_metadata_does_not_pollute_visible_text() {
        let mut node = UiNode::new("metric.live-build", UiNodeKind::Metric);
        node.label = Some("PRIMARY BAY".to_string());
        node.properties
            .insert("value".to_string(), "WIRING SIGNAL FEED".to_string());
        node.properties
            .insert("owt_builder_highlight".to_string(), "pulse".to_string());
        node.properties.insert(
            "owt_builder_highlight_label".to_string(),
            "node".to_string(),
        );

        let line = node_line(&node).expect("metric line");
        assert_eq!(line.text, "PRIMARY BAY: WIRING SIGNAL FEED");
        assert!(!line.text.starts_with("BUILD PULSE"));

        let highlight = line.builder_highlight.expect("builder highlight");
        assert_eq!(highlight.mode, "PULSE");
        assert_eq!(highlight.label, "NODE");
    }

    #[test]
    fn lcars_depth_metrics_are_bounded_and_stateful() {
        assert!(lcars_depth_metrics(0.0, 40.0, false, false).is_none());
        assert!(lcars_depth_metrics(100.0, 0.0, false, false).is_none());

        let raised = lcars_depth_metrics(100.0, 40.0, false, false).expect("raised depth");
        let hovered = lcars_depth_metrics(100.0, 40.0, false, true).expect("hovered depth");
        let pressed = lcars_depth_metrics(100.0, 40.0, true, false).expect("pressed depth");

        assert!(hovered.shadow_offset > raised.shadow_offset);
        assert!(pressed.shadow_offset < raised.shadow_offset);
        assert!(hovered.top_edge_alpha > raised.top_edge_alpha);
        assert!(pressed.bottom_edge_alpha > raised.bottom_edge_alpha);

        for metrics in [raised, hovered, pressed] {
            for alpha in [
                metrics.shadow_alpha,
                metrics.top_edge_alpha,
                metrics.bottom_edge_alpha,
                metrics.inner_edge_alpha,
            ] {
                assert!((0.0..=1.0).contains(&alpha));
            }
        }
    }

    #[test]
    fn thelcars_navigation_labels_use_semantic_structure() {
        let lines = vec![
            PanelLine::semantic(PanelLineKind::Metric, "LOAD: 0.42"),
            PanelLine::semantic(PanelLineKind::Frame, "PRIMARY FRAME"),
            PanelLine::semantic(PanelLineKind::SideRail, "LEFT RAIL"),
            PanelLine::semantic(PanelLineKind::BarRun, "HEADER RUN"),
            PanelLine::semantic(PanelLineKind::ContentBay, "LIVE BAY"),
            PanelLine::semantic(PanelLineKind::CommandGrid, "COMMAND STACK"),
            PanelLine::action("INSPECT", "inspect.demo"),
        ];

        let labels = thelcars_navigation_labels(&lines);

        assert_eq!(
            labels,
            vec![
                "PRIMARY FRAME",
                "LEFT RAIL",
                "HEADER RUN",
                "LIVE BAY",
                "COMMAND STACK"
            ]
        );
    }

    #[test]
    fn thelcars_signal_lines_prioritize_runtime_content() {
        let mut lines = vec![
            PanelLine::semantic(PanelLineKind::Frame, "FRAME"),
            PanelLine::semantic(PanelLineKind::SideRail, "RAIL"),
            PanelLine::semantic(PanelLineKind::BarRun, "BARS"),
            PanelLine::semantic(PanelLineKind::ContentBay, "BAY"),
            PanelLine::semantic(PanelLineKind::Metric, "LOAD: 0.42"),
            PanelLine::semantic(PanelLineKind::Table, "TASKS"),
            PanelLine::semantic(PanelLineKind::DataCascade, "SIGNALS"),
            PanelLine::semantic(PanelLineKind::Badge, "SAFE"),
            PanelLine::semantic(PanelLineKind::Text, "DETAIL"),
        ];
        lines.push(PanelLine::action("REFRESH", "refresh.demo"));

        let selected = thelcars_signal_lines(&lines, 4)
            .into_iter()
            .map(|line| line.text.as_str())
            .collect::<Vec<_>>();

        assert_eq!(selected, vec!["LOAD: 0.42", "TASKS", "SIGNALS", "SAFE"]);
    }

    #[test]
    fn block_composition_preserves_mx3_semantics_without_graphics_fallbacks() {
        let document = mx3_block_composition_document();
        let data = lcars_block_composition_data(&document);
        let table = lcars_table_data(&document, 6, 4).expect("mx3 table data");

        assert_eq!(data.title, "MX3 OPERATIONS BAY");
        assert_eq!(data.boxes.len(), 4);
        assert_eq!(data.boxes[0].label, "HOLD TOTAL");
        assert_eq!(data.boxes[0].value, "0");
        assert_eq!(data.boxes[1].label, "CAUSE MIX");
        assert_eq!(data.boxes[1].value, "NONE 0");
        assert_eq!(data.boxes[2].label, "TOP RCPT / SENDER");
        assert_eq!(data.boxes[3].label, "SNAPSHOT AGE");
        assert!(!data.has_graphics);
        assert_eq!(data.graphics_title, "");
        assert_eq!(data.graphics_detail, "");
        assert_eq!(data.command_bank_title, "SIDE-EFFECT-FREE COMMAND BANK");
        assert_eq!(table.title, "HOLD STATUS DATA");
    }

    #[test]
    fn provenance_label_prefers_first_class_fact_state() {
        let mut node = UiNode::new("metric.kernel", UiNodeKind::Metric);
        node.label = Some("KERNEL".to_string());
        node.properties
            .insert("value".to_string(), "6.8.0-test".to_string());
        node.provenance = Some(FactProvenance {
            source: Some("local:/proc".to_string()),
            collected_at: Some("2026-05-18T00:00:00Z".to_string()),
            state: Some(FactState::Stale),
            ..FactProvenance::default()
        });

        assert_eq!(
            node_provenance_label(&node).as_deref(),
            Some("STALE local:/proc 2026-05-18 00:00")
        );
        let line = node_line(&node).expect("metric line");
        assert!(line
            .text
            .contains("KERNEL: 6.8.0-test [STALE local:/proc 2026-05-18 00:00]"));
    }

    #[test]
    fn table_data_reads_legacy_provenance_properties() {
        let mut document = InterfaceDocument::new(
            "lcars.test.provenance",
            "LCARS Provenance",
            Scope::new(ScopeKind::Project, "/tmp/owt-provenance"),
        );
        document.theme = Some("lcars".to_string());
        let mut root = UiNode::new("root", UiNodeKind::Panel);
        let mut table = UiNode::new("table.kernel", UiNodeKind::Table);
        table.label = Some("KERNEL MATRIX".to_string());
        table
            .properties
            .insert("columns".to_string(), "host|kernel".to_string());
        table
            .properties
            .insert("rows".to_string(), "local|6.8.0-test".to_string());
        table
            .properties
            .insert("provenance".to_string(), "local:/proc".to_string());
        table.properties.insert(
            "collected_at".to_string(),
            "2026-05-18T00:00:00Z".to_string(),
        );
        table
            .properties
            .insert("state".to_string(), "observed".to_string());
        root.children.push(table);
        document.nodes.push(root);

        let table = lcars_table_data(&document, 4, 4).expect("table data");

        assert_eq!(table.title, "KERNEL MATRIX");
        assert_eq!(
            table.provenance.as_deref(),
            Some("OBS local:/proc 2026-05-18 00:00")
        );
    }

    #[test]
    fn table_data_sorts_groups_and_marks_focused_rows() {
        let mut document = InterfaceDocument::new(
            "lcars.test.table_semantics",
            "LCARS Table Semantics",
            Scope::new(ScopeKind::Project, "/tmp/owt-table-semantics"),
        );
        document.theme = Some("lcars".to_string());
        let mut root = UiNode::new("root", UiNodeKind::Panel);
        let mut table = UiNode::new("table.kernel", UiNodeKind::Table);
        table.label = Some("KERNEL MATRIX".to_string());
        table
            .properties
            .insert("columns".to_string(), "host|kernel|cohort".to_string());
        table.properties.insert(
            "rows".to_string(),
            "beta|6.1|current\nalpha|4.4|legacy".to_string(),
        );
        table
            .properties
            .insert("group_by".to_string(), "cohort".to_string());
        table
            .properties
            .insert("sort_by".to_string(), "host".to_string());
        table
            .properties
            .insert("focused_row".to_string(), "alpha".to_string());
        root.children.push(table);
        document.nodes.push(root);

        let table = lcars_table_data(&document, 4, 4).expect("table data");

        assert_eq!(table.sort_label.as_deref(), Some("SORT HOST ASC"));
        assert_eq!(table.rows[0].cells[0], "alpha");
        assert_eq!(table.rows[0].group.as_deref(), Some("legacy"));
        assert!(table.rows[0].focused);
        let focused_detail = table.focused_detail.as_deref().expect("focused row detail");
        assert!(focused_detail.contains("GROUP legacy"));
        assert!(focused_detail.contains("HOST alpha"));
        assert!(focused_detail.contains("KERNEL 4.4"));
        let group_detail = table.group_detail.as_deref().expect("group detail");
        assert!(group_detail.contains("COHORT legacy"));
        assert!(group_detail.contains("ROWS 1"));
        assert!(group_detail.contains("STATE WARN"));
        assert!(group_detail.contains("HOST alpha"));
        assert_eq!(table.rows[1].cells[0], "beta");
        assert_eq!(table.rows[1].group.as_deref(), Some("current"));
        assert!(!table.rows[1].focused);
        assert_eq!(
            table.group_summaries[0],
            LcarsTableGroupSummary {
                group: "legacy".to_string(),
                row_count: 1,
                action_count: 0,
                focused: true,
                provenance_count: 0,
                severity: Some("warning".to_string())
            }
        );
        assert_eq!(
            table_group_summary_label(&table.group_summaries[0]),
            "GRP legacy 1R FOCUS WARN"
        );
        assert_eq!(
            table_group_summary_label(&table.group_summaries[1]),
            "GRP current 1R OK"
        );
    }

    #[test]
    fn table_group_summary_honors_focused_group_metadata() {
        let mut document = InterfaceDocument::new(
            "lcars.test.table_focus_group",
            "LCARS Table Focus Group",
            Scope::new(ScopeKind::Project, "/tmp/owt-table-focus-group"),
        );
        document.theme = Some("lcars".to_string());
        let mut root = UiNode::new("root", UiNodeKind::Panel);
        let mut table = UiNode::new("table.kernel", UiNodeKind::Table);
        table
            .properties
            .insert("columns".to_string(), "host|cohort".to_string());
        table
            .properties
            .insert("rows".to_string(), "alpha|legacy\nbeta|current".to_string());
        table
            .properties
            .insert("group_by".to_string(), "cohort".to_string());
        table
            .properties
            .insert("focused_group".to_string(), "current".to_string());
        root.children.push(table);
        document.nodes.push(root);

        let table = lcars_table_data(&document, 4, 4).expect("table data");

        assert_eq!(
            table_group_summary_label(&table.group_summaries[0]),
            "GRP legacy 1R WARN"
        );
        assert_eq!(
            table_group_summary_label(&table.group_summaries[1]),
            "GRP current 1R FOCUS OK"
        );
        assert_eq!(
            table.group_detail.as_deref(),
            Some("COHORT current | ROWS 1 | STATE OK | FOCUS | HOST beta")
        );
    }

    #[test]
    fn table_group_summary_uses_explicit_cohort_nodes() {
        let mut document = InterfaceDocument::new(
            "lcars.test.explicit_cohorts",
            "LCARS Explicit Cohorts",
            Scope::new(ScopeKind::Project, "/tmp/owt-explicit-cohorts"),
        );
        document.theme = Some("lcars".to_string());
        let mut root = UiNode::new("root", UiNodeKind::Panel);
        let mut table = UiNode::new("table.kernel", UiNodeKind::Table);
        table
            .properties
            .insert("columns".to_string(), "host|kernel|cohort".to_string());
        table
            .properties
            .insert("rows".to_string(), "alpha|4.4|legacy".to_string());
        table
            .properties
            .insert("group_by".to_string(), "cohort".to_string());
        table
            .properties
            .insert("focused_group".to_string(), "legacy".to_string());
        table.properties.insert(
            "cohort_summary_node".to_string(),
            "cohorts.kernel".to_string(),
        );
        let mut cohorts = UiNode::new("cohorts.kernel", UiNodeKind::Group);
        cohorts.role = Some("cohort_summary".to_string());
        cohorts
            .properties
            .insert("source_table".to_string(), "table.kernel".to_string());
        let mut cohort = UiNode::new("cohort.legacy", UiNodeKind::Group);
        cohort.role = Some("cohort".to_string());
        cohort.label = Some("legacy".to_string());
        cohort
            .properties
            .insert("cohort_value".to_string(), "legacy".to_string());
        cohort
            .properties
            .insert("row_count".to_string(), "7".to_string());
        cohort
            .properties
            .insert("drilldown_count".to_string(), "3".to_string());
        cohort
            .properties
            .insert("severity".to_string(), "critical".to_string());
        cohorts.children.push(cohort);
        root.children.push(table);
        root.children.push(cohorts);
        document.nodes.push(root);

        let table = lcars_table_data(&document, 4, 4).expect("table data");

        assert_eq!(
            table.group_summaries[0],
            LcarsTableGroupSummary {
                group: "legacy".to_string(),
                row_count: 7,
                action_count: 3,
                focused: true,
                provenance_count: 0,
                severity: Some("critical".to_string())
            }
        );
        assert_eq!(
            table_group_summary_label(&table.group_summaries[0]),
            "GRP legacy 7R 3D FOCUS ERR"
        );
        assert_eq!(
            table.group_detail.as_deref(),
            Some("COHORT legacy | ROWS 7 | DRILL 3 | STATE ERR | FOCUS | HOST alpha")
        );
    }

    #[test]
    fn table_group_focus_mode_filters_rows_to_expanded_group() {
        let mut document = InterfaceDocument::new(
            "lcars.test.table_group_focus_mode",
            "LCARS Table Group Focus Mode",
            Scope::new(ScopeKind::Project, "/tmp/owt-table-group-focus-mode"),
        );
        document.theme = Some("lcars".to_string());
        let mut root = UiNode::new("root", UiNodeKind::Panel);
        let mut table = UiNode::new("table.kernel", UiNodeKind::Table);
        table
            .properties
            .insert("columns".to_string(), "host|kernel|cohort".to_string());
        table.properties.insert(
            "rows".to_string(),
            "alpha|4.4|legacy\nbeta|4.4|legacy\ngamma|6.8|current".to_string(),
        );
        table
            .properties
            .insert("group_by".to_string(), "cohort".to_string());
        table
            .properties
            .insert("focused_group".to_string(), "legacy".to_string());
        table
            .properties
            .insert("focus_mode".to_string(), "group".to_string());
        table
            .properties
            .insert("expanded_group".to_string(), "legacy".to_string());
        root.children.push(table);
        document.nodes.push(root);

        let table = lcars_table_data(&document, 4, 4).expect("table data");

        assert_eq!(table.rows.len(), 2);
        assert!(table
            .rows
            .iter()
            .all(|row| row.group.as_deref() == Some("legacy")));
        assert_eq!(table.focus_label.as_deref(), Some("FOCUS legacy 2R +1 OUT"));
        assert_eq!(
            table.group_detail.as_deref(),
            Some("COHORT legacy | ROWS 2 | STATE WARN | FOCUS | HOST alpha,beta")
        );
        assert_eq!(table.group_summaries.len(), 2);
    }

    #[test]
    fn table_child_rows_keep_drilldown_and_row_provenance() {
        let mut document = InterfaceDocument::new(
            "lcars.test.row_metadata",
            "LCARS Row Metadata",
            Scope::new(ScopeKind::Project, "/tmp/owt-row-metadata"),
        );
        document.theme = Some("lcars".to_string());
        let mut root = UiNode::new("root", UiNodeKind::Panel);
        let mut table = UiNode::new("table.kernel", UiNodeKind::Table);
        table
            .properties
            .insert("columns".to_string(), "host|kernel|state".to_string());
        table
            .properties
            .insert("focused_column".to_string(), "state".to_string());
        let mut row = UiNode::new("row.local", UiNodeKind::DataCascade);
        row.properties
            .insert("cells".to_string(), "local|6.8|ok".to_string());
        row.properties
            .insert("group".to_string(), "current".to_string());
        row.properties
            .insert("focused".to_string(), "true".to_string());
        row.properties.insert(
            "cell_severity".to_string(),
            "success|warning|error".to_string(),
        );
        row.properties.insert(
            "cell_provenance".to_string(),
            "local:/proc||platform.uname".to_string(),
        );
        row.action_id = Some("kernel.local.drilldown".to_string());
        row.provenance = Some(FactProvenance {
            source: Some("local:/proc".to_string()),
            state: Some(FactState::Observed),
            ..FactProvenance::default()
        });
        table.children.push(row);
        root.children.push(table);
        document.nodes.push(root);

        let table = lcars_table_data(&document, 4, 4).expect("table data");

        assert_eq!(table.interface_id, "lcars.test.row_metadata");
        assert_eq!(table.rows[0].group.as_deref(), Some("current"));
        assert!(table.rows[0].focused);
        assert_eq!(
            table_group_summary_label(&table.group_summaries[0]),
            "GRP current 1R 1D FOCUS 1P OK"
        );
        assert_eq!(
            table.rows[0].action_id.as_deref(),
            Some("kernel.local.drilldown")
        );
        assert_eq!(table.rows[0].provenance.as_deref(), Some("OBS local:/proc"));
        let focused_detail = table.focused_detail.as_deref().expect("focused row detail");
        assert!(focused_detail.contains("DRILL kernel.local.drilldown"));
        assert!(focused_detail.contains("PROV OBS local:/proc"));
        assert!(focused_detail.contains("CELL 5M"));
        let cell_detail = table.cell_detail.as_deref().expect("focused cell detail");
        assert!(cell_detail.contains("HOST S:success/P:local:/proc"));
        assert!(cell_detail.contains("KERNEL S:warning"));
        assert!(cell_detail.contains("STATE S:error/P:platform.uname"));
        let cell_focus = table.cell_focus.as_ref().expect("focused cell popover");
        assert_eq!(cell_focus.column_index, 2);
        assert_eq!(cell_focus.label, "CELL STATE");
        assert!(cell_focus.detail.contains("STATE ok"));
        assert!(cell_focus.detail.contains("SEV error"));
        assert!(cell_focus.detail.contains("PROV platform.uname"));
        assert!(cell_focus.detail.contains("GROUP current"));
        assert!(cell_focus.detail.contains("HOST local"));
        assert!(cell_focus.detail.contains("DRILL kernel.local.drilldown"));
        let group_detail = table.group_detail.as_deref().expect("group detail");
        assert!(group_detail.contains("COHORT current"));
        assert!(group_detail.contains("DRILL 1"));
        assert!(group_detail.contains("PROV 1"));
        assert!(group_detail.contains("CELL 5"));
        assert_eq!(
            table.rows[0].cell_severity,
            vec![
                Some("success".to_string()),
                Some("warning".to_string()),
                Some("error".to_string())
            ]
        );
        assert_eq!(
            table.rows[0].cell_provenance,
            vec![
                Some("local:/proc".to_string()),
                None,
                Some("platform.uname".to_string())
            ]
        );
        let hitboxes = table_row_action_hitboxes(&table, 10.0, 300.0, 50.0, 40.0, 7.0, 1);
        assert_eq!(hitboxes.len(), 1);
        assert_eq!(hitboxes[0].action_id, "kernel.local.drilldown");
        assert_eq!(hitboxes[0].x, 0.0);
        assert_eq!(hitboxes[0].y, 48.0);
        let cell_hitboxes =
            table_cell_focus_hitboxes(&table, 10.0, 300.0, 50.0, 40.0, 7.0, 1, 10.0);
        assert_eq!(cell_hitboxes.len(), 3);
        assert_eq!(cell_hitboxes[0].column, "host");
        assert_eq!(cell_hitboxes[1].column, "kernel");
        assert_eq!(cell_hitboxes[2].column, "state");
    }

    #[test]
    fn table_child_focus_key_marks_rendered_focus() {
        let mut document = InterfaceDocument::new(
            "lcars.test.row_focus_key",
            "LCARS Row Focus Key",
            Scope::new(ScopeKind::Project, "/tmp/owt-row-focus-key"),
        );
        document.theme = Some("lcars".to_string());
        let mut root = UiNode::new("root", UiNodeKind::Panel);
        let mut table = UiNode::new("table.kernel", UiNodeKind::Table);
        table
            .properties
            .insert("columns".to_string(), "host|kernel|state".to_string());
        table
            .properties
            .insert("focused_row".to_string(), "local".to_string());
        let mut row = UiNode::new("row.local", UiNodeKind::DataCascade);
        row.properties
            .insert("cells".to_string(), "node-1|6.8|ok".to_string());
        row.properties
            .insert("focus_key".to_string(), "local".to_string());
        table.children.push(row);
        root.children.push(table);
        document.nodes.push(root);

        let table = lcars_table_data(&document, 4, 4).expect("table data");

        assert!(table.rows[0].focused);
    }

    #[test]
    fn lcars_table_focus_keys_use_control_alt_shift_arrows() {
        assert_eq!(
            lcars_table_focus_movement(&KeyCode::DownArrow, None),
            Some(TableFocusMovement::Next)
        );
        assert_eq!(
            lcars_table_focus_movement(&KeyCode::UpArrow, None),
            Some(TableFocusMovement::Previous)
        );
        assert_eq!(
            lcars_table_focus_movement(&KeyCode::LeftArrow, None),
            Some(TableFocusMovement::PreviousGroup)
        );
        assert_eq!(
            lcars_table_focus_movement(&KeyCode::RightArrow, None),
            Some(TableFocusMovement::NextGroup)
        );
        assert_eq!(
            lcars_table_focus_movement(&KeyCode::Char('x'), Some(&PhysKeyCode::PageUp)),
            Some(TableFocusMovement::First)
        );
        assert_eq!(
            lcars_table_focus_movement(&KeyCode::Char('x'), Some(&PhysKeyCode::PageDown)),
            Some(TableFocusMovement::Last)
        );
        assert_eq!(lcars_table_focus_movement(&KeyCode::Char('x'), None), None);
        assert!(lcars_table_activation_key(&KeyCode::Char('\r'), None));
        assert!(lcars_table_activation_key(
            &KeyCode::Char('x'),
            Some(&PhysKeyCode::Return)
        ));
        assert!(!lcars_table_activation_key(&KeyCode::Char('x'), None));
        assert!(lcars_table_focus_mode_key(&KeyCode::Char(' '), None));
        assert!(lcars_table_focus_mode_key(
            &KeyCode::Char('x'),
            Some(&PhysKeyCode::Space)
        ));
        assert!(!lcars_table_focus_mode_key(&KeyCode::Char('x'), None));
        assert_eq!(
            lcars_table_cell_focus_movement(&KeyCode::Char('<'), None),
            Some(TableCellFocusMovement::Previous)
        );
        assert_eq!(
            lcars_table_cell_focus_movement(&KeyCode::Char('>'), None),
            Some(TableCellFocusMovement::Next)
        );
        assert_eq!(
            lcars_table_cell_focus_movement(&KeyCode::Char('x'), Some(&PhysKeyCode::Home)),
            Some(TableCellFocusMovement::First)
        );
        assert_eq!(
            lcars_table_cell_focus_movement(&KeyCode::Char('x'), Some(&PhysKeyCode::End)),
            Some(TableCellFocusMovement::Last)
        );
    }

    #[test]
    fn block_composition_planning_suppresses_diagnostic_detail_promotion() {
        let planned = plan_lcars_structural_console(
            120.0,
            1500.0,
            90.0,
            320.0,
            10.0,
            20.0,
            true,
            true,
            3,
            LcarsStructuralMode::BlockComposition,
        );

        assert_eq!(planned.detail, LcarsDetailPlacement::None);
        assert!(planned.command_visible);
        assert!(planned.signal_width > 0.0);
    }

    #[test]
    fn block_composition_can_reflow_to_side_or_bottom_without_losing_content() {
        let mut document = block_composition_document();
        document.actions.push(UiAction::new(
            "lcars.block.zoom_in",
            "ZOOM+",
            ActionKind::Inspect,
        ));
        let mut button = UiNode::new("zoom-in", UiNodeKind::Button);
        button.label = Some("zoom in".to_string());
        button.action_id = Some("lcars.block.zoom_in".to_string());
        document.nodes[0].children.push(button);

        let baseline_actions = lcars_keyboard_action_slots(&document);
        let baseline_data = lcars_block_composition_data(&document);
        for (dock, expected_layout) in [
            ("right", LcarsPanelLayout::Right),
            ("bottom-right", LcarsPanelLayout::Bottom),
        ] {
            let mut moved = document.clone();
            moved.nodes[0]
                .properties
                .insert("dock".to_string(), dock.to_string());
            let placement = lcars_surface_placement(&moved);
            let moved_data = lcars_block_composition_data(&moved);

            assert_eq!(
                lcars_structural_mode(&moved),
                LcarsStructuralMode::BlockComposition
            );
            assert_eq!(placement.layout, expected_layout);
            assert_eq!(placement.reservation, LcarsSurfaceReservation::Reserved);
            assert_eq!(lcars_keyboard_action_slots(&moved), baseline_actions);
            assert_eq!(moved_data.title, baseline_data.title);
            assert_eq!(moved_data.boxes.len(), baseline_data.boxes.len());
            assert_eq!(moved_data.graphics_title, baseline_data.graphics_title);
        }
    }

    #[test]
    fn docked_surface_planning_prioritizes_composition_over_diagnostics() {
        let compact = plan_lcars_structural_console(
            120.0,
            860.0,
            90.0,
            320.0,
            10.0,
            20.0,
            true,
            true,
            3,
            LcarsStructuralMode::DockedSurface,
        );
        let wide = plan_lcars_structural_console(
            120.0,
            1840.0,
            90.0,
            320.0,
            10.0,
            20.0,
            true,
            true,
            3,
            LcarsStructuralMode::DockedSurface,
        );

        assert_eq!(compact.detail, LcarsDetailPlacement::None);
        assert_eq!(wide.detail, LcarsDetailPlacement::None);
        assert!(compact.command_width >= 285.0);
        assert!(wide.command_width >= 340.0);
        assert!(compact.signal_rows <= 3);
        assert!(wide.signal_rows <= 4);
        assert!(compact.command_left + compact.command_width <= 860.0);
        assert!(wide.command_left + wide.command_width <= 1840.0);
    }

    #[test]
    fn docked_surface_scene_docks_status_tab_to_command_bank() {
        let scene = compute_lcars_structural_scene(
            180.0,
            1760.0,
            60.0,
            430.0,
            900.0,
            10.0,
            20.0,
            false,
            false,
            3,
            LcarsStructuralMode::DockedSurface,
        );
        let command_bank = scene.command_bank.expect("docked command bank scene");
        let bank_right = scene.plan.command_left + scene.plan.command_width;

        assert!(scene.status_tab.x >= scene.plan.command_left - 1.0);
        assert!(scene.status_tab.x + scene.status_tab.width <= bank_right + 1.0);
        assert!(command_bank.frame.x <= scene.plan.command_left);
        assert!(scene.content_bay.height >= 420.0);
        let bay_right = scene.content_bay.x + scene.content_bay.width;
        assert!(command_bank.frame.x >= bay_right);
        assert!(command_bank.frame.x - bay_right <= 12.0);
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
    fn lcars_available_clamp_handles_narrow_resize_widths() {
        let label = clamp_to_available(48.0, 150.0, 36.0);
        let data_label = clamp_to_available(32.0, 80.0, 28.0);
        let negative_max = clamp_to_available(12.0, 80.0, -6.0);

        assert_eq!(label, 36.0);
        assert_eq!(data_label, 28.0);
        assert_eq!(negative_max, 1.0);
    }

    #[test]
    fn active_action_ack_accent_is_distinct_from_default_button_fills() {
        for index in 0..4 {
            let default_fill = lcars_action_fill_byte(index);
            let active_fill = lcars_action_accent_byte(default_fill, true);
            assert_ne!(lcars_action_accent_byte(default_fill, true), default_fill);
            assert!(
                active_fill == super::LCARS_BYTE_PEACH || active_fill == super::LCARS_BYTE_AMBER
            );
            assert_eq!(lcars_action_accent_byte(default_fill, false), default_fill);
        }
    }

    #[test]
    fn action_hotkeys_do_not_pollute_visible_labels() {
        let mut lines = vec![PanelLine::action("Inspect primitives", "inspect")];

        assign_lcars_hotkeys(&mut lines);

        assert_eq!(lines[0].hotkey, Some(1));
        assert_eq!(lines[0].text, "Inspect primitives");
    }

    #[test]
    fn declared_action_label_controls_command_bank_text() {
        let mut document = InterfaceDocument::new(
            "lcars.test.action_label",
            "LCARS Action Label",
            Scope::new(ScopeKind::Project, "/tmp/owt-action-label"),
        );
        document.actions.push(UiAction::new(
            "dev.inspect.button",
            "ACK",
            ActionKind::Inspect,
        ));
        let mut root = UiNode::new("root", UiNodeKind::Panel);
        let mut button = UiNode::new("button", UiNodeKind::Button);
        button.label = Some("ack path button".to_string());
        button.action_id = Some("dev.inspect.button".to_string());
        root.children.push(button);
        document.nodes.push(root);

        let action_line = collect_panel_items(&document)
            .into_iter()
            .find(|line| line.action_id.as_deref() == Some("dev.inspect.button"))
            .expect("declared action line");

        assert_eq!(action_line.text, "ACK");
    }

    #[test]
    fn side_panel_signal_selection_prioritizes_data_over_chrome() {
        let mut lines = vec![
            PanelLine::semantic(PanelLineKind::Frame, "FRAME"),
            PanelLine::semantic(PanelLineKind::SideRail, "RAIL"),
            PanelLine::semantic(PanelLineKind::BarRun, "BARS"),
            PanelLine::semantic(PanelLineKind::ContentBay, "BAY"),
            PanelLine::semantic(PanelLineKind::Metric, "LOAD: 0.42"),
            PanelLine::semantic(PanelLineKind::Table, "TASKS"),
            PanelLine::semantic(PanelLineKind::DataCascade, "SIGNALS"),
            PanelLine::semantic(PanelLineKind::Badge, "SAFE"),
        ];
        lines.push(PanelLine::action("REFRESH", "refresh.demo"));

        let selected = prioritized_side_panel_signal_lines(&lines, 4)
            .into_iter()
            .map(|line| line.text.as_str())
            .collect::<Vec<_>>();

        assert_eq!(selected, vec!["LOAD: 0.42", "TASKS", "SIGNALS", "SAFE"]);
    }

    #[test]
    fn parses_left_and_right_rail_layouts_distinctly() {
        assert_eq!(
            parse_lcars_panel_layout("left_rail"),
            Some(LcarsPanelLayout::Left)
        );
        assert_eq!(
            parse_lcars_panel_layout("right_rail"),
            Some(LcarsPanelLayout::Right)
        );
        assert_eq!(
            parse_lcars_panel_layout("browser_left"),
            Some(LcarsPanelLayout::Left)
        );
        assert_eq!(
            parse_lcars_panel_layout("browser_right"),
            Some(LcarsPanelLayout::Right)
        );
    }

    #[test]
    fn render_projection_reports_actual_lcars_path_and_normalized_layout() {
        let mut document = InterfaceDocument::new(
            "lcars.test.projection",
            "LCARS Projection",
            Scope::new(ScopeKind::Project, "/tmp/owt-projection"),
        );
        let mut root = UiNode::new("root", UiNodeKind::Panel);
        root.properties
            .insert("layout".to_string(), "rail-right".to_string());
        root.properties
            .insert("profile".to_string(), "fleet_matrix".to_string());
        root.properties
            .insert("anchor".to_string(), "edge-sticky".to_string());
        root.properties
            .insert("z_order".to_string(), "5".to_string());
        root.properties
            .insert("priority".to_string(), "9".to_string());
        root.properties
            .insert("min_terminal_cells".to_string(), "80x24".to_string());
        root.properties
            .insert("collapse_policy".to_string(), "summary".to_string());
        document.nodes.push(root);

        let projection = describe_lcars_render_projection(&document);

        assert_eq!(projection.renderer_path, "structural_lcars");
        assert_eq!(projection.layout, "right_rail");
        assert_eq!(projection.origin, "right");
        assert_eq!(projection.reservation, "reserved");
        assert_eq!(projection.orientation, "vertical");
        assert!(projection.reserves_terminal_space);
        assert_eq!(projection.anchor.as_deref(), Some("edge-sticky"));
        assert_eq!(projection.z_order, Some(5));
        assert_eq!(projection.priority, Some(9));
        assert_eq!(projection.min_terminal_cells.as_deref(), Some("80x24"));
        assert_eq!(projection.collapse_policy.as_deref(), Some("summary"));
        assert_eq!(projection.structural_profile, Some("composition"));
        assert_eq!(
            projection.requested_profile.as_deref(),
            Some("fleet_matrix")
        );
    }

    #[test]
    fn render_projection_exposes_information_shape_contract_metadata() {
        let mut document = InterfaceDocument::new(
            "lcars.test.profile_contract",
            "LCARS Profile Contract",
            Scope::new(ScopeKind::Project, "/tmp/owt-profile-contract"),
        );
        document.theme = Some("lcars".to_string());

        let mut root = UiNode::new("profile-root", UiNodeKind::Panel);
        root.properties
            .insert("information_shape".to_string(), "queue_triage".to_string());
        root.properties
            .insert("profile_family".to_string(), "triage_queue".to_string());
        root.properties
            .insert("table_density".to_string(), "dense".to_string());
        root.properties
            .insert("cohort_key".to_string(), "queue".to_string());
        root.properties
            .insert("state_key".to_string(), "disposition".to_string());
        root.properties
            .insert("severity_key".to_string(), "priority".to_string());
        root.properties.insert(
            "lifecycle_controls".to_string(),
            "refresh,pin,hide".to_string(),
        );
        root.properties.insert(
            "action_roles".to_string(),
            "refresh,inspect,advance,defer,drilldown".to_string(),
        );
        root.properties.insert(
            "refresh_policy".to_string(),
            "explicit_snapshot".to_string(),
        );
        root.properties.insert(
            "drilldown_policy".to_string(),
            "queue_item_focus".to_string(),
        );
        document.nodes.push(root);

        let projection = describe_lcars_render_projection(&document);

        assert_eq!(
            projection.requested_profile.as_deref(),
            Some("queue_triage")
        );
        assert_eq!(projection.profile_family.as_deref(), Some("triage_queue"));
        assert_eq!(projection.table_density.as_deref(), Some("dense"));
        assert_eq!(projection.cohort_key.as_deref(), Some("queue"));
        assert_eq!(projection.state_key.as_deref(), Some("disposition"));
        assert_eq!(projection.severity_key.as_deref(), Some("priority"));
        assert_eq!(
            projection.lifecycle_controls.as_deref(),
            Some("refresh,pin,hide")
        );
        assert_eq!(
            projection.action_roles.as_deref(),
            Some("refresh,inspect,advance,defer,drilldown")
        );
        assert_eq!(
            projection.refresh_policy.as_deref(),
            Some("explicit_snapshot")
        );
        assert_eq!(
            projection.drilldown_policy.as_deref(),
            Some("queue_item_focus")
        );
    }

    #[test]
    fn palette_profile_aliases_select_non_default_palettes() {
        let mut document = InterfaceDocument::new(
            "lcars.test.palette",
            "Palette",
            Scope::new(ScopeKind::Session, "palette"),
        );
        let mut root = UiNode::new("palette-root", UiNodeKind::Panel);
        root.properties
            .insert("palette_profile".to_string(), "science-station".to_string());
        document.nodes.push(root);

        assert_eq!(
            lcars_palette_profile(&document),
            LcarsPaletteProfile::ScienceStation
        );

        document.nodes[0]
            .properties
            .insert("palette_profile".to_string(), "bright_classic".to_string());
        assert_eq!(
            lcars_palette_profile(&document),
            LcarsPaletteProfile::BrightClassic
        );

        document.nodes[0]
            .properties
            .insert("palette_profile".to_string(), "classic".to_string());
        assert_eq!(
            lcars_palette_profile(&document),
            LcarsPaletteProfile::Classic
        );
    }

    #[test]
    fn render_projection_reports_explicit_palette_profile() {
        let mut document = InterfaceDocument::new(
            "lcars.test.palette_projection",
            "Palette Projection",
            Scope::new(ScopeKind::Session, "palette"),
        );
        let mut root = UiNode::new("palette-root", UiNodeKind::Panel);
        root.properties
            .insert("profile".to_string(), "fleet_matrix".to_string());
        root.properties
            .insert("palette".to_string(), "muted_science".to_string());
        document.nodes.push(root);

        let projection = describe_lcars_render_projection(&document);

        assert_eq!(projection.renderer_path, "structural_lcars");
        assert_eq!(projection.palette_profile, Some("science_station"));
    }

    #[test]
    fn parses_corner_docking_aliases_as_surface_origins() {
        assert_eq!(
            parse_lcars_surface_origin("top-left"),
            Some(LcarsSurfaceOrigin::TopLeft)
        );
        assert_eq!(
            parse_lcars_surface_origin("right bottom"),
            Some(LcarsSurfaceOrigin::BottomRight)
        );
        assert_eq!(
            parse_lcars_panel_layout("bottom_right"),
            Some(LcarsPanelLayout::Bottom)
        );
        assert_eq!(
            parse_lcars_surface_origin("undocked"),
            Some(LcarsSurfaceOrigin::Overlay)
        );
        assert_eq!(
            parse_lcars_panel_layout("widget"),
            Some(LcarsPanelLayout::Overlay)
        );
    }

    #[test]
    fn moving_surface_changes_placement_without_rewriting_content() {
        let top = placement_test_document(None, None, None);
        let moved = placement_test_document(Some("bottom-right"), None, None);

        let top_placement = lcars_surface_placement(&top);
        let moved_placement = lcars_surface_placement(&moved);

        assert_eq!(top_placement.origin, LcarsSurfaceOrigin::TopLeft);
        assert_eq!(top_placement.layout, LcarsPanelLayout::Top);
        assert_eq!(
            top_placement.orientation,
            LcarsSurfaceOrientation::Horizontal
        );
        assert_eq!(moved_placement.origin, LcarsSurfaceOrigin::BottomRight);
        assert_eq!(moved_placement.layout, LcarsPanelLayout::Bottom);
        assert_eq!(
            moved_placement.reservation,
            LcarsSurfaceReservation::Reserved
        );

        let top_actions: Vec<_> = collect_panel_items(&top)
            .into_iter()
            .filter_map(|line| {
                let action_id = line.action_id?;
                Some((action_id, line.text))
            })
            .collect();
        let moved_actions: Vec<_> = collect_panel_items(&moved)
            .into_iter()
            .filter_map(|line| {
                let action_id = line.action_id?;
                Some((action_id, line.text))
            })
            .collect();

        assert_eq!(top_actions, moved_actions);
    }

    #[test]
    fn placement_can_overlay_without_changing_semantic_layout() {
        let document = placement_test_document(Some("left"), Some("overlay"), Some("vertical"));
        let placement = lcars_surface_placement(&document);

        assert_eq!(placement.origin, LcarsSurfaceOrigin::Left);
        assert_eq!(placement.layout, LcarsPanelLayout::Left);
        assert_eq!(placement.reservation, LcarsSurfaceReservation::Overlay);
        assert_eq!(placement.orientation, LcarsSurfaceOrientation::Vertical);
    }

    #[test]
    fn render_scene_summary_groups_active_surface_slots() {
        let left = placement_test_document(Some("left"), None, None);
        let right = placement_test_document(Some("right"), None, None);
        let overlay = placement_test_document(Some("overlay"), Some("overlay"), Some("auto"));
        let summary = lcars_render_scene_summary(&[left, right, overlay]);

        assert_eq!(summary.active_count, 3);
        assert_eq!(summary.reserved_count, 2);
        assert_eq!(summary.overlay_count, 1);
        assert!(summary.conflict_hints.is_empty());
        assert_eq!(
            summary
                .slots
                .iter()
                .map(|slot| (slot.slot, slot.active_count, slot.reserved_count))
                .collect::<Vec<_>>(),
            vec![("left_rail", 1, 1), ("right_rail", 1, 1), ("overlay", 1, 0)]
        );
        assert_eq!(
            lcars_render_scene_badge_lines(&summary)
                .as_ref()
                .map(|lines| lines.0.as_str()),
            Some("SCENE PLAN")
        );
        assert!(lcars_render_scene_badge_lines(&summary)
            .unwrap()
            .1
            .contains("3A / 2R / 1O"));
    }

    #[test]
    fn render_scene_summary_reports_same_slot_reserved_conflicts() {
        let left_a = placement_test_document(Some("left"), None, None);
        let mut left_b = placement_test_document(Some("left"), None, None);
        left_b.id = "lcars.test.placement.secondary".to_string();
        let summary = lcars_render_scene_summary(&[left_a, left_b]);

        assert_eq!(summary.active_count, 2);
        assert_eq!(summary.reserved_count, 2);
        assert_eq!(summary.conflict_hints.len(), 1);
        assert!(summary.conflict_hints[0].contains("left_rail has 2 reserved surfaces"));
        assert_eq!(
            lcars_render_scene_badge_lines(&summary),
            Some((
                "SCENE CONFLICT".to_string(),
                "2A / 2R / L2R / L2 CONFLICT".to_string()
            ))
        );
    }

    #[test]
    fn surface_menu_bottom_right_docks_reserve_terminal_space() {
        let properties = lcars_surface_menu_properties(LcarsSurfaceMenuAction::DockBottomRight)
            .expect("bottom-right dock action should produce layout properties");

        assert_eq!(
            properties.get("layout").map(String::as_str),
            Some("bottom_strip")
        );
        assert_eq!(
            properties.get("placement").map(String::as_str),
            Some("bottom-right")
        );
        assert_eq!(
            properties.get("origin").map(String::as_str),
            Some("bottom-right")
        );
        assert_eq!(
            properties.get("reservation").map(String::as_str),
            Some("reserved")
        );
        assert_eq!(properties.get("reserve").map(String::as_str), Some("true"));
        assert_eq!(properties.get("display").map(String::as_str), Some("block"));

        let mut document = placement_test_document(None, None, None);
        for (key, value) in properties {
            document.nodes[0].properties.insert(key, value);
        }
        let placement = lcars_surface_placement(&document);

        assert_eq!(placement.origin, LcarsSurfaceOrigin::BottomRight);
        assert_eq!(placement.layout, LcarsPanelLayout::Bottom);
        assert_eq!(placement.reservation, LcarsSurfaceReservation::Reserved);
        assert!(placement.reserves_terminal_space());
    }

    #[test]
    fn surface_menu_move_actions_emit_complete_visible_layout_patch() {
        for (action, layout, placement, origin, dock, orientation) in [
            (
                LcarsSurfaceMenuAction::DockTopLeft,
                "docked_top",
                "top-left",
                "top-left",
                "top",
                "horizontal",
            ),
            (
                LcarsSurfaceMenuAction::DockLeftRail,
                "left_rail",
                "left",
                "left",
                "left_rail",
                "vertical",
            ),
            (
                LcarsSurfaceMenuAction::DockRightRail,
                "right_rail",
                "right",
                "right",
                "right_rail",
                "vertical",
            ),
            (
                LcarsSurfaceMenuAction::DockBottomStrip,
                "bottom_strip",
                "bottom",
                "bottom",
                "bottom_strip",
                "horizontal",
            ),
            (
                LcarsSurfaceMenuAction::DockBottomRight,
                "bottom_strip",
                "bottom-right",
                "bottom-right",
                "bottom_right",
                "horizontal",
            ),
        ] {
            let properties =
                lcars_surface_menu_properties(action).expect("move action should patch layout");
            assert_eq!(properties.get("active").map(String::as_str), Some("true"));
            assert_eq!(properties.get("layout").map(String::as_str), Some(layout));
            assert_eq!(
                properties.get("placement").map(String::as_str),
                Some(placement)
            );
            assert_eq!(properties.get("origin").map(String::as_str), Some(origin));
            assert_eq!(properties.get("dock").map(String::as_str), Some(dock));
            assert_eq!(
                properties.get("orientation").map(String::as_str),
                Some(orientation)
            );
            assert_eq!(
                properties.get("reservation").map(String::as_str),
                Some("reserved")
            );
            assert_eq!(properties.get("reserve").map(String::as_str), Some("true"));
            assert_eq!(properties.get("visible").map(String::as_str), Some("true"));
            assert_eq!(properties.get("hidden").map(String::as_str), Some("false"));
            assert_eq!(properties.get("display").map(String::as_str), Some("block"));
        }
    }

    #[test]
    fn surface_menu_move_restores_hidden_surface_before_reanchoring() {
        let mut document = placement_test_document(Some("top"), None, None);
        document.nodes[0]
            .properties
            .insert("hidden".to_string(), "true".to_string());
        document.nodes[0]
            .properties
            .insert("visible".to_string(), "false".to_string());
        document.nodes[0]
            .properties
            .insert("display".to_string(), "none".to_string());
        assert!(!lcars_surface_visible(&document));

        let properties = lcars_surface_menu_properties(LcarsSurfaceMenuAction::DockRightRail)
            .expect("right dock action should produce layout properties");
        for (key, value) in properties {
            document.nodes[0].properties.insert(key, value);
        }

        let placement = lcars_surface_placement(&document);
        assert!(lcars_surface_visible(&document));
        assert_eq!(placement.origin, LcarsSurfaceOrigin::Right);
        assert_eq!(placement.layout, LcarsPanelLayout::Right);
        assert_eq!(placement.orientation, LcarsSurfaceOrientation::Vertical);
        assert_eq!(placement.reservation, LcarsSurfaceReservation::Reserved);
    }

    #[test]
    fn surface_menu_main_layout_groups_commands_without_overlap() {
        let slots = lcars_surface_menu_slot_rects(
            OwtLcarsSurfaceMenuMode::Main,
            100.0,
            80.0,
            300.0,
            28.0,
            4.0,
        );

        assert_eq!(slots[0].x, slots[1].x);
        assert_eq!(slots[1].x, slots[6].x);
        assert!(slots[0].y < slots[1].y);
        assert!(slots[1].y < slots[6].y);

        assert_eq!(slots[7].x, slots[5].x);
        assert_eq!(slots[5].x, slots[2].x);
        assert_eq!(slots[2].x, slots[4].x);
        assert_eq!(slots[4].x, slots[3].x);
        assert!(slots[7].y < slots[5].y);
        assert!(slots[5].y < slots[2].y);
        assert!(slots[2].y < slots[4].y);
        assert!(slots[4].y < slots[3].y);

        for left in 0..slots.len() {
            for right in (left + 1)..slots.len() {
                assert!(
                    !rects_overlap(slots[left], slots[right]),
                    "slot {} overlaps slot {}",
                    left,
                    right
                );
            }
        }
    }

    #[test]
    fn surface_menu_saved_layout_uses_two_page_columns() {
        let slots = lcars_surface_menu_slot_rects(
            OwtLcarsSurfaceMenuMode::LoadSaved,
            100.0,
            80.0,
            300.0,
            28.0,
            4.0,
        );

        assert_eq!(slots[0].x, slots[1].x);
        assert_eq!(slots[1].x, slots[2].x);
        assert_eq!(slots[2].x, slots[3].x);
        assert!(slots[0].y < slots[1].y);
        assert!(slots[1].y < slots[2].y);
        assert!(slots[2].y < slots[3].y);

        assert_eq!(slots[4].x, slots[5].x);
        assert_eq!(slots[5].x, slots[6].x);
        assert_eq!(slots[6].x, slots[7].x);
        assert_eq!(slots[4].y, slots[0].y);
        assert!(slots[4].x > slots[0].x);

        for left in 0..slots.len() {
            for right in (left + 1)..slots.len() {
                assert!(
                    !rects_overlap(slots[left], slots[right]),
                    "slot {} overlaps slot {}",
                    left,
                    right
                );
            }
        }
    }

    #[test]
    fn native_lcars_surface_disables_below_minimal_viewport() {
        assert!(!owt_lcars_native_surface_viewport_ready(319.0, 220.0));
        assert!(!owt_lcars_native_surface_viewport_ready(360.0, 179.0));
        assert!(owt_lcars_native_surface_viewport_ready(360.0, 220.0));
    }

    #[test]
    fn surface_drag_drop_maps_edges_to_docks_and_center_to_overlay() {
        let left = owt_lcars_drag_drop_layout_properties(8.0, 400.0, 1200.0, 800.0);
        assert_eq!(left.get("layout").map(String::as_str), Some("left_rail"));
        assert_eq!(left.get("placement").map(String::as_str), Some("left"));
        assert_eq!(left.get("dock").map(String::as_str), Some("left_rail"));
        assert_eq!(
            left.get("orientation").map(String::as_str),
            Some("vertical")
        );
        assert_eq!(left.get("display").map(String::as_str), Some("block"));

        let right = owt_lcars_drag_drop_layout_properties(1192.0, 400.0, 1200.0, 800.0);
        assert_eq!(right.get("layout").map(String::as_str), Some("right_rail"));
        assert_eq!(right.get("placement").map(String::as_str), Some("right"));
        assert_eq!(right.get("dock").map(String::as_str), Some("right_rail"));
        assert_eq!(
            right.get("orientation").map(String::as_str),
            Some("vertical")
        );

        let top = owt_lcars_drag_drop_layout_properties(600.0, 8.0, 1200.0, 800.0);
        assert_eq!(top.get("layout").map(String::as_str), Some("docked_top"));
        assert_eq!(top.get("placement").map(String::as_str), Some("top-left"));
        assert_eq!(top.get("dock").map(String::as_str), Some("top"));
        assert_eq!(
            top.get("orientation").map(String::as_str),
            Some("horizontal")
        );

        let bottom = owt_lcars_drag_drop_layout_properties(600.0, 792.0, 1200.0, 800.0);
        assert_eq!(
            bottom.get("layout").map(String::as_str),
            Some("bottom_strip")
        );
        assert_eq!(bottom.get("placement").map(String::as_str), Some("bottom"));
        assert_eq!(bottom.get("dock").map(String::as_str), Some("bottom_strip"));
        assert_eq!(
            bottom.get("orientation").map(String::as_str),
            Some("horizontal")
        );

        let bottom_right = owt_lcars_drag_drop_layout_properties(1192.0, 792.0, 1200.0, 800.0);
        assert_eq!(
            bottom_right.get("layout").map(String::as_str),
            Some("bottom_strip")
        );
        assert_eq!(
            bottom_right.get("placement").map(String::as_str),
            Some("bottom-right")
        );
        assert_eq!(
            bottom_right.get("dock").map(String::as_str),
            Some("bottom_right")
        );

        let overlay = owt_lcars_drag_drop_layout_properties(600.0, 400.0, 1200.0, 800.0);
        assert_eq!(overlay.get("layout").map(String::as_str), Some("overlay"));
        assert_eq!(overlay.get("dock").map(String::as_str), Some("overlay"));
        assert_eq!(
            overlay.get("reservation").map(String::as_str),
            Some("overlay")
        );
        assert_eq!(overlay.get("reserve").map(String::as_str), Some("overlay"));
        assert_eq!(overlay.get("placement").map(String::as_str), Some("free"));
        assert_eq!(overlay.get("display").map(String::as_str), Some("block"));
        assert_eq!(
            overlay.get("floating_anchor").map(String::as_str),
            Some("center")
        );
        assert_eq!(overlay.get("floating_x").map(String::as_str), Some("600"));
        assert_eq!(overlay.get("floating_y").map(String::as_str), Some("400"));
        assert!(overlay.contains_key("floating_width"));
        assert!(overlay.contains_key("floating_height"));

        assert_eq!(
            owt_lcars_drag_drop_target_text(8.0, 400.0, 1200.0, 800.0),
            ("LEFT", "DOCK LEFT RAIL")
        );
        assert_eq!(
            owt_lcars_drag_drop_target_text(600.0, 400.0, 1200.0, 800.0),
            ("FREE", "FLOAT OVERLAY")
        );
    }

    #[test]
    fn surface_resize_preserves_top_left_and_clamps_geometry() {
        let resized = owt_lcars_resize_layout_properties(120.0, 80.0, 640.0, 410.0, 1000.0, 700.0);
        assert_eq!(resized.get("dock").map(String::as_str), Some("overlay"));
        assert_eq!(resized.get("reserve").map(String::as_str), Some("overlay"));
        assert_eq!(resized.get("placement").map(String::as_str), Some("free"));
        assert_eq!(
            resized.get("floating_anchor").map(String::as_str),
            Some("top_left")
        );
        assert_eq!(resized.get("floating_x").map(String::as_str), Some("120"));
        assert_eq!(resized.get("floating_y").map(String::as_str), Some("80"));
        assert_eq!(
            resized.get("floating_width").map(String::as_str),
            Some("520")
        );
        assert_eq!(
            resized.get("floating_height").map(String::as_str),
            Some("330")
        );

        let clamped =
            owt_lcars_resize_layout_properties(120.0, 80.0, 9000.0, 9000.0, 1000.0, 700.0);
        assert_eq!(
            clamped.get("floating_width").map(String::as_str),
            Some("872")
        );
        assert_eq!(
            clamped.get("floating_height").map(String::as_str),
            Some("612")
        );

        let min_clamped =
            owt_lcars_resize_layout_properties(120.0, 80.0, 140.0, 95.0, 1000.0, 700.0);
        assert_eq!(
            min_clamped.get("floating_width").map(String::as_str),
            Some("320")
        );
        assert_eq!(
            min_clamped.get("floating_height").map(String::as_str),
            Some("166")
        );
    }

    #[test]
    fn undocked_projection_preserves_floating_geometry() {
        let mut document = placement_test_document(Some("undocked"), Some("overlay"), Some("auto"));
        let root = &mut document.nodes[0];
        root.properties
            .insert("floating_anchor".to_string(), "center".to_string());
        root.properties
            .insert("floating_x".to_string(), "640".to_string());
        root.properties
            .insert("floating_y".to_string(), "360".to_string());
        root.properties
            .insert("floating_width".to_string(), "520".to_string());
        root.properties
            .insert("floating_height".to_string(), "260".to_string());

        let placement = lcars_surface_placement(&document);
        let projection = describe_lcars_render_projection(&document);

        assert_eq!(placement.origin, LcarsSurfaceOrigin::Overlay);
        assert_eq!(placement.layout, LcarsPanelLayout::Overlay);
        assert_eq!(placement.reservation, LcarsSurfaceReservation::Overlay);
        assert!(!projection.reserves_terminal_space);
        assert_eq!(projection.floating_anchor.as_deref(), Some("center"));
        assert_eq!(projection.floating_x.as_deref(), Some("640"));
        assert_eq!(projection.floating_y.as_deref(), Some("360"));
        assert_eq!(projection.floating_width.as_deref(), Some("520"));
        assert_eq!(projection.floating_height.as_deref(), Some("260"));
    }

    #[test]
    fn surface_menu_saved_detail_includes_owner_label() {
        let saved = crate::owt_native::SavedInterfaceSummary {
            store_id: "kernel-left".to_string(),
            title: Some("Kernel Left".to_string()),
            interface_id: Some("tools.kernel.left".to_string()),
            owner_label: Some("PROJECT tools".to_string()),
            owner_key: Some("project:/home/buanzo/git/tools".to_string()),
        };

        assert_eq!(
            lcars_saved_interface_menu_detail(&saved),
            "PROJECT tools / tools.kernel.left"
        );
    }

    #[test]
    fn surface_menu_owner_label_uses_scope_final_segment() {
        let scope = Scope::new(ScopeKind::Project, "/home/buanzo/git/tools");
        assert_eq!(lcars_scope_owner_label(&scope), "PROJECT tools");
    }

    #[test]
    fn surface_menu_owner_filter_cycles_between_owner_groups() {
        let saved = vec![
            saved_interface_summary(
                "kernel-left",
                "PROJECT tools",
                "project:/home/buanzo/git/tools",
            ),
            saved_interface_summary(
                "carrier-right",
                "PROJECT carriertv",
                "project:/home/buanzo/git/tools/python/carriertv",
            ),
        ];

        let first = lcars_next_saved_owner_filter(&saved, None);
        assert_eq!(first.as_deref(), Some("project:/home/buanzo/git/tools"));
        let second = lcars_next_saved_owner_filter(&saved, first.as_deref());
        assert_eq!(
            second.as_deref(),
            Some("project:/home/buanzo/git/tools/python/carriertv")
        );
        assert_eq!(
            lcars_next_saved_owner_filter(&saved, second.as_deref()),
            None
        );
    }

    #[test]
    fn surface_menu_owner_filter_status_counts_visible_items() {
        let state = OwtLcarsSurfaceMenuState {
            mode: OwtLcarsSurfaceMenuMode::LoadSaved,
            selected_idx: 0,
            saved_page: 0,
            saved_owner_filter: Some("project:/home/buanzo/git/tools".to_string()),
            saved_interfaces: vec![
                saved_interface_summary(
                    "kernel-left",
                    "PROJECT tools",
                    "project:/home/buanzo/git/tools",
                ),
                saved_interface_summary(
                    "kernel-right",
                    "PROJECT tools",
                    "project:/home/buanzo/git/tools",
                ),
                saved_interface_summary(
                    "carrier-right",
                    "PROJECT carriertv",
                    "project:/home/buanzo/git/tools/python/carriertv",
                ),
            ],
            status_line: None,
            target_interface_id: None,
            anchor_x: 0.0,
            anchor_y: 0.0,
        };

        assert_eq!(
            lcars_saved_owner_filter_detail(&state),
            "FILTER PROJECT tools"
        );
        assert_eq!(
            lcars_saved_menu_status(&state),
            "LOAD OWNER PROJECT tools / ITEMS 02"
        );
    }

    #[test]
    fn surface_control_properties_can_hide_lcars_without_destroying_document() {
        let mut document = placement_test_document(Some("right"), None, None);
        assert!(lcars_surface_visible(&document));

        document.nodes[0]
            .properties
            .insert("visible".to_string(), "false".to_string());

        assert!(!lcars_surface_visible(&document));
        assert_eq!(
            lcars_keyboard_action_slots(&document),
            vec!["lcars.demo.layout"]
        );
    }

    fn saved_interface_summary(
        store_id: &str,
        owner_label: &str,
        owner_key: &str,
    ) -> crate::owt_native::SavedInterfaceSummary {
        crate::owt_native::SavedInterfaceSummary {
            store_id: store_id.to_string(),
            title: Some(store_id.to_string()),
            interface_id: Some(format!("tools.{store_id}")),
            owner_label: Some(owner_label.to_string()),
            owner_key: Some(owner_key.to_string()),
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

    fn primitive_legend_document() -> InterfaceDocument {
        let mut document = InterfaceDocument::new(
            "lcars.test.primitive_legend",
            "LCARS Primitive Legend",
            Scope::new(ScopeKind::Project, "/tmp/owt-primitive-legend"),
        );
        document.theme = Some("lcars".to_string());

        let mut root = UiNode::new("legend-root", UiNodeKind::Panel);
        root.label = Some("primitive legend root".to_string());
        root.properties
            .insert("profile".to_string(), "primitive_legend".to_string());
        root.properties
            .insert("test".to_string(), "primitive_coverage".to_string());

        let specs = [
            ("region", UiNodeKind::Region, "region boundary"),
            ("group", UiNodeKind::Group, "group cluster"),
            ("frame", UiNodeKind::Frame, "chrome frame"),
            ("side-rail", UiNodeKind::SideRail, "side rail"),
            ("content-bay", UiNodeKind::ContentBay, "content bay"),
            ("text", UiNodeKind::Text, "plain text"),
            ("bar", UiNodeKind::Bar, "single bar"),
            ("bar-run", UiNodeKind::BarRun, "segmented run"),
            ("elbow", UiNodeKind::Elbow, "dominant elbow"),
            ("command-grid", UiNodeKind::CommandGrid, "command grid"),
            ("data-cascade", UiNodeKind::DataCascade, "data cascade"),
            ("button", UiNodeKind::Button, "ack button"),
            ("badge", UiNodeKind::Badge, "badge ok"),
            ("list", UiNodeKind::List, "list rows"),
            ("table", UiNodeKind::Table, "table primitive"),
            ("metric", UiNodeKind::Metric, "metric value"),
            ("progress", UiNodeKind::Progress, "progress 42%"),
            ("image", UiNodeKind::Image, "image placeholder"),
            ("spacer", UiNodeKind::Spacer, "spacer primitive"),
        ];

        for (id, kind, label) in specs {
            let mut node = UiNode::new(id, kind);
            node.label = Some(label.to_string());
            if node.kind == UiNodeKind::Button {
                node.action_id = Some("dev.inspect.button".to_string());
            }
            root.children.push(node);
        }
        document.nodes.push(root);
        document
    }

    fn placement_test_document(
        dock: Option<&str>,
        reservation: Option<&str>,
        orientation: Option<&str>,
    ) -> InterfaceDocument {
        let mut document = InterfaceDocument::new(
            "lcars.test.placement",
            "LCARS Placement",
            Scope::new(ScopeKind::Project, "/tmp/owt-placement"),
        );
        document.theme = Some("lcars".to_string());
        document.actions.push(UiAction::new(
            "lcars.demo.layout",
            "LAYOUT",
            ActionKind::Inspect,
        ));

        let mut root = UiNode::new("placement-root", UiNodeKind::Panel);
        root.properties
            .insert("layout".to_string(), "top".to_string());
        root.properties
            .insert("profile".to_string(), "structural_console".to_string());
        if let Some(dock) = dock {
            root.properties.insert("dock".to_string(), dock.to_string());
        }
        if let Some(reservation) = reservation {
            root.properties
                .insert("reservation".to_string(), reservation.to_string());
        }
        if let Some(orientation) = orientation {
            root.properties
                .insert("orientation".to_string(), orientation.to_string());
        }

        let mut button = UiNode::new("layout-button", UiNodeKind::Button);
        button.label = Some("move layout".to_string());
        button.action_id = Some("lcars.demo.layout".to_string());
        root.children.push(button);

        document.nodes.push(root);
        document
    }

    fn block_composition_document() -> InterfaceDocument {
        let mut document = InterfaceDocument::new(
            "lcars.test.block_composition",
            "LCARS Block Composition",
            Scope::new(ScopeKind::Project, "/tmp/owt-block-composition"),
        );
        document.theme = Some("lcars".to_string());

        let mut root = UiNode::new("block-root", UiNodeKind::Panel);
        root.properties
            .insert("profile".to_string(), "block_composition".to_string());

        let mut group = UiNode::new("scan-control", UiNodeKind::Group);
        group.label = Some("scan control".to_string());
        group.role = Some("labelled_box".to_string());

        let mut range_label = UiNode::new("range-label", UiNodeKind::Text);
        range_label.text = Some("range window".to_string());
        range_label.role = Some("label".to_string());
        let mut range_value = UiNode::new("range-value", UiNodeKind::Metric);
        range_value
            .properties
            .insert("value".to_string(), "28741".to_string());

        let mut band_label = UiNode::new("band-label", UiNodeKind::Text);
        band_label.text = Some("frequency band".to_string());
        band_label.role = Some("label".to_string());
        let mut band_value = UiNode::new("band-value", UiNodeKind::Metric);
        band_value
            .properties
            .insert("value".to_string(), "581257-365".to_string());

        group.children.push(range_label);
        group.children.push(range_value);
        group.children.push(band_label);
        group.children.push(band_value);
        root.children.push(group);

        let mut graphics = UiNode::new("scan-graphics", UiNodeKind::Image);
        graphics.label = Some("long range scan".to_string());
        graphics.role = Some("graphics_panel".to_string());
        graphics
            .properties
            .insert("detail".to_string(), "zoom vector ready".to_string());
        root.children.push(graphics);

        document.nodes.push(root);
        document
    }

    fn mx3_block_composition_document() -> InterfaceDocument {
        let mut document = InterfaceDocument::new(
            "mrec.mrecic.mx3.native.ops",
            "MRECIC MX3 OPS",
            Scope::new(
                ScopeKind::Project,
                "/home/buanzo/git/tools/mrec/mrecic_cycle",
            ),
        );
        document.theme = Some("lcars".to_string());

        let mut root = UiNode::new("root.mx3", UiNodeKind::Panel);
        root.properties
            .insert("profile".to_string(), "block_composition".to_string());
        root.properties.insert(
            "information_shape".to_string(),
            "mx3_ops_snapshot".to_string(),
        );

        let mut bay = UiNode::new("bay.operations", UiNodeKind::Group);
        bay.label = Some("MX3 OPERATIONS BAY".to_string());
        bay.role = Some("labelled_box".to_string());
        for (id, label, value, status, text) in [
            (
                "box.hold_total",
                "HOLD TOTAL",
                "0",
                "OK",
                "0 held messages\nRecommendation: No action",
            ),
            (
                "box.cause_mix",
                "CAUSE MIX",
                "NONE 0",
                "OK",
                "RATE 0 | MILTER 0 | OTHER 0 | UNKNOWN 0",
            ),
            (
                "box.top_entities",
                "TOP RCPT / SENDER",
                "LOCAL CACHE",
                "INFO",
                "RCPT none\nFROM none",
            ),
            (
                "box.snapshot_age",
                "SNAPSHOT AGE",
                "UNKNOWN",
                "INFO",
                "unknown\nState: UNKNOWN\nSource: hub-mx3 -> mx3",
            ),
        ] {
            let mut box_node = UiNode::new(id, UiNodeKind::DataCascade);
            box_node.label = Some(label.to_string());
            box_node.text = Some(text.to_string());
            box_node
                .properties
                .insert("value".to_string(), value.to_string());
            box_node
                .properties
                .insert("status".to_string(), status.to_string());
            bay.children.push(box_node);
        }
        root.children.push(bay);

        let mut table = UiNode::new("table.hold.status", UiNodeKind::Table);
        table.label = Some("HOLD STATUS DATA".to_string());
        table.properties.insert(
            "headers".to_string(),
            "ID | TO | FROM | CAUSE | SCORE | SUBJECT".to_string(),
        );
        table.properties.insert(
            "rows".to_string(),
            "NO CACHED SAMPLE | - | - | - | - | No cached MX3 snapshot yet. Run refresh first."
                .to_string(),
        );
        root.children.push(table);

        let mut commands = UiNode::new("grid.commands", UiNodeKind::CommandGrid);
        commands.label = Some("SIDE-EFFECT-FREE COMMAND BANK".to_string());
        root.children.push(commands);

        document.nodes.push(root);
        document
    }
}
