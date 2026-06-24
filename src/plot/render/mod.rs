pub mod layout;
pub mod metrics;

use super::{
    config::{PngEncodeMode, Theme},
    data::Quote,
    moves::{ImportantMove, MoveKind},
    render::{
        layout::{
            ChartLayout, PositionedText, build_layout, format_brl_cent, x_of_index, y_of_price,
        },
        metrics::{FontMetricsCache, TextRotation},
    },
};
use anyhow::Result;
use image::{
    ColorType, ImageEncoder,
    codecs::png::{CompressionType, FilterType, PngEncoder},
};
use plotters::{
    coord::Shift,
    prelude::*,
    style::{FontTransform, TextStyle},
};
use std::{fs, io, path::Path};

#[derive(Debug)]
pub struct RenderEngine {
    theme: Theme,
    metrics: FontMetricsCache,
}

#[derive(Clone, Debug)]
pub struct ChartPlan {
    pub layout: ChartLayout,
    pub points: Vec<(i32, i32)>,
    pub movement_layers: Vec<MovementLayer>,
}

#[derive(Clone, Debug)]
pub struct MovementLayer {
    pub start_idx: usize,
    pub end_idx: usize,
    pub line_color: RGBColor,
    pub label_color: RGBColor,
    pub label: PositionedText,
}

#[derive(Clone, Debug)]
pub struct PngStats {
    pub mode: String,
    pub png_bytes: usize,
}

impl RenderEngine {
    pub fn new(theme: Theme) -> Self {
        Self {
            theme,
            metrics: FontMetricsCache::default(),
        }
    }

    pub fn theme(&self) -> &Theme {
        &self.theme
    }

    pub fn warm_up(&mut self, quotes: &[Quote], moves: &[ImportantMove]) {
        let t = &self.theme.typography;
        let common = [
            "USD/BRL - últimos 30 pregões",
            "escala vertical: 19 px = R$ 0,01 | pernas importantes: >= R$ 0,03 | detectadas: 9",
            self.theme.axis.x_label,
            self.theme.axis.y_label,
        ];
        for s in common {
            self.metrics
                .measure(s, t.title_font_px, TextRotation::None, t);
            self.metrics
                .measure(s, t.subtitle_font_px, TextRotation::None, t);
            self.metrics
                .measure(s, t.axis_label_font_px, TextRotation::None, t);
        }
        for quote in quotes {
            let label = quote.date.format("%d/%m").to_string();
            self.metrics
                .measure(&label, t.axis_tick_font_px, TextRotation::Rotate270, t);
            let cents = (quote.usd_brl * 100.0).round() as i32;
            let y_label = format_brl_cent(cents);
            self.metrics
                .measure(&y_label, t.axis_tick_font_px, TextRotation::None, t);
        }
        for mv in moves {
            let label = format_delta(mv.delta);
            self.metrics
                .measure(&label, t.movement_label_font_px, TextRotation::None, t);
        }
    }

    pub fn metric_cache_len(&self) -> usize {
        self.metrics.len()
    }

    pub fn plan(&mut self, quotes: &[Quote], moves: &[ImportantMove]) -> Result<ChartPlan> {
        let layout = build_layout(quotes, moves, &self.theme, &mut self.metrics)?;

        let points: Vec<(i32, i32)> = quotes
            .iter()
            .enumerate()
            .map(|(idx, q)| {
                (
                    x_of_index(idx, quotes.len(), layout.plot_left, layout.plot_width),
                    y_of_price(
                        q.usd_brl,
                        layout.y_min_cent,
                        layout.px_per_cent,
                        layout.plot_bottom,
                    ),
                )
            })
            .collect();

        let movement_layers = moves
            .iter()
            .map(|mv| self.build_movement_layer(mv, &points, &layout))
            .collect::<Result<Vec<_>>>()?;

        Ok(ChartPlan {
            layout,
            points,
            movement_layers,
        })
    }

    pub fn rasterize_rgb(&self, plan: &ChartPlan) -> Result<Vec<u8>> {
        let w = self.theme.canvas.width;
        let h = self.theme.canvas.height;
        let mut rgb = vec![0u8; w as usize * h as usize * 3];

        {
            let backend = BitMapBackend::with_buffer(&mut rgb, (w, h));
            let root = backend.into_drawing_area();
            self.draw_plan(&root, plan)?;
            root.present().map_err(pe)?;
        }

        Ok(rgb)
    }

    pub fn encode_png(&self, rgb: Vec<u8>) -> Result<(Vec<u8>, PngStats)> {
        let mode = self.theme.png.mode;
        let png = match mode {
            PngEncodeMode::Fast => {
                self.encode_png_with_image(&rgb, CompressionType::Fast, FilterType::Adaptive)?
            }
            PngEncodeMode::Balanced => {
                self.encode_png_with_image(&rgb, CompressionType::Default, FilterType::Adaptive)?
            }
            PngEncodeMode::Level(level) => {
                let level = level.clamp(1, 9);
                self.encode_png_with_image(
                    &rgb,
                    CompressionType::Level(level),
                    FilterType::Adaptive,
                )?
            }
            PngEncodeMode::Uncompressed => self.encode_png_with_image(
                &rgb,
                CompressionType::Uncompressed,
                FilterType::NoFilter,
            )?,
            PngEncodeMode::OxipngRaw { preset } => self.encode_png_with_oxipng_raw(rgb, preset)?,
        };

        let png_bytes = png.len();
        Ok((
            png,
            PngStats {
                mode: mode.name(),
                png_bytes,
            },
        ))
    }

    fn encode_png_with_image(
        &self,
        rgb: &[u8],
        compression: CompressionType,
        filter: FilterType,
    ) -> Result<Vec<u8>> {
        let mut out = Vec::with_capacity(rgb.len());
        let encoder = PngEncoder::new_with_quality(&mut out, compression, filter);
        encoder.write_image(
            rgb,
            self.theme.canvas.width,
            self.theme.canvas.height,
            ColorType::Rgb8.into(),
        )?;
        Ok(out)
    }

    fn encode_png_with_oxipng_raw(&self, rgb: Vec<u8>, preset: u8) -> Result<Vec<u8>> {
        let raw = oxipng::RawImage::new(
            self.theme.canvas.width,
            self.theme.canvas.height,
            oxipng::ColorType::RGB {
                transparent_color: None,
            },
            oxipng::BitDepth::Eight,
            rgb,
        )?;
        let opts = oxipng::Options::from_preset(preset.min(6));
        Ok(raw.create_optimized_png(&opts)?)
    }

    pub fn write_png(&self, output: impl AsRef<Path>, png: &[u8]) -> Result<()> {
        fs::write(output, png)?;
        Ok(())
    }

    fn build_movement_layer(
        &mut self,
        mv: &ImportantMove,
        points: &[(i32, i32)],
        layout: &ChartLayout,
    ) -> Result<MovementLayer> {
        let (x1, y1) = points[mv.start_idx];
        let (x2, y2) = points[mv.end_idx];
        let label = format_delta(mv.delta);
        let bbox = self.metrics.measure(
            &label,
            self.theme.typography.movement_label_font_px,
            TextRotation::None,
            &self.theme.typography,
        );

        let mid_x = (x1 + x2) / 2;
        let mid_y = (y1 + y2) / 2;

        let mut x = mid_x - bbox.width / 2;
        let mut y = match mv.kind {
            MoveKind::Rise => {
                mid_y - bbox.height - self.theme.movement.label_offset_from_segment_px
            }
            MoveKind::Fall => mid_y + self.theme.movement.label_offset_from_segment_px,
        };

        let bg_pad_x = if self.theme.movement.label_bg.is_some() {
            self.theme.movement.label_bg_padding_x_px
        } else {
            0
        };
        let bg_pad_y = if self.theme.movement.label_bg.is_some() {
            self.theme.movement.label_bg_padding_y_px
        } else {
            0
        };

        let gap = self.theme.layout.movement_label_canvas_gap_px;
        x = clamp_i32(
            x,
            layout.plot_left + bg_pad_x + gap,
            layout.plot_right - bbox.width - bg_pad_x - gap,
        );
        y = clamp_i32(
            y,
            layout.plot_top + bg_pad_y + gap,
            layout.plot_bottom - bbox.height - bg_pad_y - gap,
        );

        let (line_color, label_color) = match mv.kind {
            MoveKind::Rise => (
                self.theme.movement.rise_line,
                self.theme.movement.rise_label,
            ),
            MoveKind::Fall => (
                self.theme.movement.fall_line,
                self.theme.movement.fall_label,
            ),
        };

        Ok(MovementLayer {
            start_idx: mv.start_idx,
            end_idx: mv.end_idx,
            line_color,
            label_color,
            label: PositionedText {
                text: label,
                x,
                y,
                bbox,
            },
        })
    }

    fn draw_plan<DB: DrawingBackend>(
        &self,
        root: &DrawingArea<DB, Shift>,
        plan: &ChartPlan,
    ) -> Result<()> {
        let layout = &plan.layout;
        let p = &self.theme.palette;
        let axis = &self.theme.axis;

        root.fill(&p.canvas_bg).map_err(pe)?;

        root.draw(&Rectangle::new(
            [
                (layout.plot_left, layout.plot_top),
                (layout.plot_right, layout.plot_bottom),
            ],
            p.plot_bg.filled(),
        ))
        .map_err(pe)?;

        self.draw_text(
            root,
            &layout.title,
            self.theme.typography.title_font_px,
            p.text,
            FontTransform::None,
        )?;
        self.draw_text(
            root,
            &layout.subtitle,
            self.theme.typography.subtitle_font_px,
            p.muted_text,
            FontTransform::None,
        )?;

        for tick in &layout.y_ticks {
            root.draw(&PathElement::new(
                vec![(layout.plot_left, tick.y), (layout.plot_right, tick.y)],
                ShapeStyle::from(&p.grid).stroke_width(axis.grid_stroke_width_px),
            ))
            .map_err(pe)?;
            self.draw_text(
                root,
                &tick.label,
                self.theme.typography.axis_tick_font_px,
                p.muted_text,
                FontTransform::None,
            )?;
        }

        let axis_style = ShapeStyle::from(&p.axis).stroke_width(axis.axis_stroke_width_px);
        root.draw(&PathElement::new(
            vec![
                (layout.plot_left, layout.plot_top),
                (layout.plot_left, layout.plot_bottom),
            ],
            axis_style,
        ))
        .map_err(pe)?;
        root.draw(&PathElement::new(
            vec![
                (layout.plot_left, layout.plot_bottom),
                (layout.plot_right, layout.plot_bottom),
            ],
            axis_style,
        ))
        .map_err(pe)?;

        for tick in &layout.x_ticks {
            root.draw(&PathElement::new(
                vec![
                    (tick.x, layout.plot_bottom),
                    (tick.x, layout.plot_bottom + axis.x_tick_length_px),
                ],
                ShapeStyle::from(&p.axis).stroke_width(axis.tick_stroke_width_px),
            ))
            .map_err(pe)?;
            self.draw_text(
                root,
                &tick.label,
                self.theme.typography.axis_tick_font_px,
                p.muted_text,
                FontTransform::Rotate270,
            )?;
        }

        root.draw(&PathElement::new(
            plan.points.clone(),
            ShapeStyle::from(&self.theme.series.quote_line)
                .stroke_width(self.theme.series.quote_width_px),
        ))
        .map_err(pe)?;

        for layer in &plan.movement_layers {
            root.draw(&PathElement::new(
                plan.points[layer.start_idx..=layer.end_idx].to_vec(),
                ShapeStyle::from(&layer.line_color).stroke_width(self.theme.movement.line_width_px),
            ))
            .map_err(pe)?;

            if let Some(bg) = self.theme.movement.label_bg {
                let pad_x = self.theme.movement.label_bg_padding_x_px;
                let pad_y = self.theme.movement.label_bg_padding_y_px;
                root.draw(&Rectangle::new(
                    [
                        (layer.label.x - pad_x, layer.label.y - pad_y),
                        (
                            layer.label.x + layer.label.bbox.width + pad_x,
                            layer.label.y + layer.label.bbox.height + pad_y,
                        ),
                    ],
                    bg.filled(),
                ))
                .map_err(pe)?;
            }

            self.draw_text(
                root,
                &layer.label,
                self.theme.typography.movement_label_font_px,
                layer.label_color,
                FontTransform::None,
            )?;
        }

        for &(x, y) in &plan.points {
            root.draw(&Circle::new(
                (x, y),
                self.theme.series.point_radius_px + self.theme.series.point_outline_extra_radius_px,
                ShapeStyle::from(&self.theme.series.point_outline).filled(),
            ))
            .map_err(pe)?;
            root.draw(&Circle::new(
                (x, y),
                self.theme.series.point_radius_px,
                ShapeStyle::from(&self.theme.series.point_fill).filled(),
            ))
            .map_err(pe)?;
        }

        for label in &layout.axis_labels {
            self.draw_text(
                root,
                label,
                self.theme.typography.axis_label_font_px,
                p.muted_text,
                FontTransform::None,
            )?;
        }

        Ok(())
    }

    fn draw_text<DB: DrawingBackend>(
        &self,
        root: &DrawingArea<DB, Shift>,
        text: &PositionedText,
        font_px: u32,
        color: RGBColor,
        transform: FontTransform,
    ) -> Result<()> {
        let style = TextStyle::from((self.theme.typography.font_family, font_px).into_font())
            .color(&color)
            .transform(transform);
        root.draw(&Text::new(text.text.clone(), (text.x, text.y), style))
            .map_err(pe)?;
        Ok(())
    }
}

fn clamp_i32(v: i32, lo: i32, hi: i32) -> i32 {
    if lo > hi {
        return lo;
    }
    v.max(lo).min(hi)
}

fn format_delta(delta: f64) -> String {
    let prefix = if delta >= 0.0 { "+R$ " } else { "-R$ " };
    format!("{}{:.2}", prefix, delta.abs()).replace('.', ",")
}

fn pe<E: std::fmt::Debug>(e: E) -> io::Error {
    io::Error::other(format!("plotters error: {e:?}"))
}
