use plotters::prelude::*;

#[derive(Clone, Debug)]
pub struct Theme {
    pub canvas: CanvasTheme,
    pub typography: TypographyTheme,
    pub palette: PaletteTheme,
    pub layout: LayoutTheme,
    pub scale: ScaleTheme,
    pub axis: AxisTheme,
    pub series: SeriesTheme,
    pub movement: MovementTheme,
    pub png: PngTheme,
}

#[derive(Clone, Debug)]
pub struct CanvasTheme {
    pub width: u32,
    pub height: u32,
}

#[derive(Clone, Debug)]
pub struct TypographyTheme {
    pub font_family: &'static str,
    pub title_font_px: u32,
    pub subtitle_font_px: u32,
    pub axis_tick_font_px: u32,
    pub axis_label_font_px: u32,
    pub movement_label_font_px: u32,

    // Centralized font measurement policy. Replace this struct later with a real
    // font measurement backend without touching chart layout logic.
    pub avg_advance_em: f64,
    pub line_height_em: f64,
}

#[derive(Clone, Debug)]
pub struct PaletteTheme {
    pub canvas_bg: RGBColor,
    pub plot_bg: RGBColor,
    pub axis: RGBColor,
    pub grid: RGBAColor,
    pub text: RGBColor,
    pub muted_text: RGBColor,
}

#[derive(Clone, Debug)]
pub struct LayoutTheme {
    // These are margins around text and chart content. The actual plot rectangle
    // is measured from tick-label sizes, title sizes, axis-label sizes, and these margins.
    pub outer_left_px: i32,
    pub outer_right_px: i32,
    pub outer_top_px: i32,
    pub outer_bottom_px: i32,

    pub title_to_subtitle_gap_px: i32,
    pub subtitle_to_plot_gap_px: i32,

    pub y_tick_label_to_axis_gap_px: i32,
    pub x_axis_to_tick_end_px: i32,
    pub x_tick_to_label_gap_px: i32,
    pub x_labels_to_axis_labels_gap_px: i32,
    pub axis_label_to_canvas_edge_gap_px: i32,

    pub plot_min_inner_top_gap_px: i32,
    pub plot_min_inner_bottom_gap_px: i32,

    pub movement_label_canvas_gap_px: i32,

    pub max_x_tick_labels: usize,
    pub min_x_tick_label_gap_px: i32,
}

#[derive(Clone, Debug)]
pub struct ScaleTheme {
    // None => maximize vertical usage while keeping a constant px/cent inside this chart.
    // Some(n) => enforce n pixels per BRL cent; errors if the data doesn't fit.
    pub force_px_per_cent: Option<i32>,
    pub min_px_per_cent: i32,
    pub max_px_per_cent: i32,
    pub padding_cents: i32,
}

#[derive(Clone, Debug)]
pub struct AxisTheme {
    pub x_label: &'static str,
    pub y_label: &'static str,
    pub axis_stroke_width_px: u32,
    pub grid_stroke_width_px: u32,
    pub tick_stroke_width_px: u32,
    pub x_tick_length_px: i32,
}

#[derive(Clone, Debug)]
pub struct SeriesTheme {
    pub quote_line: RGBColor,
    pub quote_width_px: u32,
    pub point_fill: RGBColor,
    pub point_outline: RGBColor,
    pub point_radius_px: i32,
    pub point_outline_extra_radius_px: i32,
}

#[derive(Clone, Debug)]
pub struct MovementTheme {
    pub important_delta_cents: i32,

    pub rise_line: RGBColor,
    pub fall_line: RGBColor,
    pub rise_label: RGBColor,
    pub fall_label: RGBColor,
    pub line_width_px: u32,

    pub label_offset_from_segment_px: i32,
    pub label_bg: Option<RGBAColor>,
    pub label_bg_padding_x_px: i32,
    pub label_bg_padding_y_px: i32,
}

#[derive(Clone, Copy, Debug)]
pub enum PngEncodeMode {
    // Fastest practical PNG path: one image::PngEncoder pass with low compression.
    Fast,
    // image::PngEncoder default policy. Usually slightly smaller and slightly slower than Fast.
    Balanced,
    // image::PngEncoder explicit DEFLATE level, clamped by CLI/parser to 1..=9.
    Level(u8),
    // Valid PNG with no DEFLATE compression. Useful as a latency baseline, rarely good for transport.
    Uncompressed,
    // One-pass oxipng from raw pixels. Slower, but avoids the old encode-then-recompress path.
    OxipngRaw { preset: u8 },
}

impl PngEncodeMode {
    pub fn name(self) -> String {
        match self {
            Self::Fast => "image-fast".to_string(),
            Self::Balanced => "image-balanced".to_string(),
            Self::Level(level) => format!("image-level-{level}"),
            Self::Uncompressed => "image-uncompressed".to_string(),
            Self::OxipngRaw { preset } => format!("oxipng-raw-preset-{preset}"),
        }
    }
}

#[derive(Clone, Debug)]
pub struct PngTheme {
    pub mode: PngEncodeMode,
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            canvas: CanvasTheme { width: 1280, height: 720 },
            typography: TypographyTheme {
                font_family: "sans-serif",
                title_font_px: 32,
                subtitle_font_px: 17,
                axis_tick_font_px: 17,
                axis_label_font_px: 18,
                movement_label_font_px: 18,
                avg_advance_em: 0.62,
                line_height_em: 1.18,
            },
            palette: PaletteTheme {
                canvas_bg: RGBColor(16, 20, 30),
                plot_bg: RGBColor(24, 30, 44),
                axis: RGBColor(174, 185, 204),
                grid: RGBAColor(210, 220, 240, 0.16),
                text: RGBColor(238, 242, 248),
                muted_text: RGBColor(175, 184, 198),
            },
            layout: LayoutTheme {
                outer_left_px: 18,
                outer_right_px: 42,
                outer_top_px: 32,
                outer_bottom_px: 18,
                title_to_subtitle_gap_px: 4,
                subtitle_to_plot_gap_px: 22,
                y_tick_label_to_axis_gap_px: 18,
                x_axis_to_tick_end_px: 7,
                x_tick_to_label_gap_px: 7,
                x_labels_to_axis_labels_gap_px: 18,
                axis_label_to_canvas_edge_gap_px: 2,
                plot_min_inner_top_gap_px: 12,
                plot_min_inner_bottom_gap_px: 12,
                movement_label_canvas_gap_px: 4,
                max_x_tick_labels: 8,
                min_x_tick_label_gap_px: 38,
            },
            scale: ScaleTheme {
                force_px_per_cent: None,
                min_px_per_cent: 5,
                max_px_per_cent: 22,
                padding_cents: 4,
            },
            axis: AxisTheme {
                x_label: "data",
                y_label: "R$ por US$ 1",
                axis_stroke_width_px: 2,
                grid_stroke_width_px: 1,
                tick_stroke_width_px: 1,
                x_tick_length_px: 7,
            },
            series: SeriesTheme {
                quote_line: RGBColor(110, 168, 255),
                quote_width_px: 3,
                point_fill: RGBColor(238, 242, 248),
                point_outline: RGBColor(16, 20, 30),
                point_radius_px: 4,
                point_outline_extra_radius_px: 2,
            },
            movement: MovementTheme {
                important_delta_cents: 3,
                rise_line: RGBColor(70, 210, 125),
                fall_line: RGBColor(240, 82, 82),
                rise_label: RGBColor(70, 230, 140),
                fall_label: RGBColor(255, 95, 95),
                line_width_px: 8,
                label_offset_from_segment_px: 14,
                label_bg: None,
                label_bg_padding_x_px: 8,
                label_bg_padding_y_px: 5,
            },
            png: PngTheme { mode: PngEncodeMode::Fast },
        }
    }
}
