use super::super::{
    config::Theme,
    data::Quote,
    moves::ImportantMove,
    render::metrics::{FontMetricsCache, TextBoxPx, TextRotation},
};
use anyhow::{Result, anyhow};

#[derive(Clone, Debug)]
pub struct ChartLayout {
    pub width: u32,
    pub height: u32,

    pub plot_left: i32,
    pub plot_right: i32,
    pub plot_top: i32,
    pub plot_bottom: i32,
    pub plot_width: i32,
    pub plot_height: i32,

    pub y_min_cent: i32,
    pub y_max_cent: i32,
    pub px_per_cent: i32,

    pub title: PositionedText,
    pub subtitle: PositionedText,
    pub y_ticks: Vec<YTickLayout>,
    pub x_ticks: Vec<XTickLayout>,
    pub axis_labels: Vec<PositionedText>,
}

#[derive(Clone, Debug)]
pub struct PositionedText {
    pub text: String,
    pub x: i32,
    pub y: i32,
    pub bbox: TextBoxPx,
}

#[derive(Clone, Debug)]
pub struct YTickLayout {
    pub cents: i32,
    pub y: i32,
    pub label: PositionedText,
}

#[derive(Clone, Debug)]
pub struct XTickLayout {
    pub idx: usize,
    pub x: i32,
    pub label: PositionedText,
}

pub fn build_layout(
    quotes: &[Quote],
    moves: &[ImportantMove],
    theme: &Theme,
    metrics: &mut FontMetricsCache,
) -> Result<ChartLayout> {
    if quotes.len() < 2 {
        return Err(anyhow!("need at least two quotes"));
    }

    let width = theme.canvas.width;
    let height = theme.canvas.height;
    let w = width as i32;
    let h = height as i32;

    let min_price = quotes
        .iter()
        .map(|q| q.usd_brl)
        .fold(f64::INFINITY, f64::min);
    let max_price = quotes
        .iter()
        .map(|q| q.usd_brl)
        .fold(f64::NEG_INFINITY, f64::max);

    let y_min_cent = (min_price * 100.0).floor() as i32 - theme.scale.padding_cents;
    let y_max_cent = (max_price * 100.0).ceil() as i32 + theme.scale.padding_cents;
    let y_span_cents = (y_max_cent - y_min_cent).max(1);

    let y_tick_cents = build_y_tick_cents(y_min_cent, y_max_cent);
    let y_tick_labels: Vec<String> = y_tick_cents.iter().map(|c| format_brl_cent(*c)).collect();
    let max_y_tick_label_width = y_tick_labels
        .iter()
        .map(|s| {
            metrics
                .measure(
                    s,
                    theme.typography.axis_tick_font_px,
                    TextRotation::None,
                    &theme.typography,
                )
                .width
        })
        .max()
        .unwrap_or(0);

    let plot_left = theme.layout.outer_left_px
        + max_y_tick_label_width
        + theme.layout.y_tick_label_to_axis_gap_px;
    let plot_right = w - theme.layout.outer_right_px;
    let plot_width = plot_right - plot_left;
    if plot_width <= 0 {
        return Err(anyhow!(
            "not enough horizontal room after measured labels and margins"
        ));
    }

    let title = format!("USD/BRL - últimos {} pregões", quotes.len());
    let title_bbox = metrics.measure(
        &title,
        theme.typography.title_font_px,
        TextRotation::None,
        &theme.typography,
    );
    let title_pos = PositionedText {
        text: title,
        x: plot_left,
        y: theme.layout.outer_top_px,
        bbox: title_bbox,
    };

    let subtitle_height = metrics
        .measure(
            "subtitle",
            theme.typography.subtitle_font_px,
            TextRotation::None,
            &theme.typography,
        )
        .height;
    let preliminary_subtitle_y =
        title_pos.y + title_bbox.height + theme.layout.title_to_subtitle_gap_px;
    let preliminary_plot_area_top =
        preliminary_subtitle_y + subtitle_height + theme.layout.subtitle_to_plot_gap_px;

    let x_indices = choose_evenly_spaced_indices(
        quotes.len(),
        theme.layout.max_x_tick_labels,
        plot_width,
        theme.layout.min_x_tick_label_gap_px,
    );

    let x_label_bboxes: Vec<TextBoxPx> = x_indices
        .iter()
        .map(|idx| {
            let s = quotes[*idx].date.format("%d/%m").to_string();
            metrics.measure(
                &s,
                theme.typography.axis_tick_font_px,
                TextRotation::Rotate270,
                &theme.typography,
            )
        })
        .collect();
    let max_x_label_height = x_label_bboxes.iter().map(|b| b.height).max().unwrap_or(0);

    let axis_label_font = theme.typography.axis_label_font_px;
    let x_axis_label_bbox = metrics.measure(
        theme.axis.x_label,
        axis_label_font,
        TextRotation::None,
        &theme.typography,
    );
    let y_axis_label_bbox = metrics.measure(
        theme.axis.y_label,
        axis_label_font,
        TextRotation::None,
        &theme.typography,
    );
    let axis_label_height = if theme.axis.x_label.is_empty() && theme.axis.y_label.is_empty() {
        0
    } else {
        x_axis_label_bbox.height.max(y_axis_label_bbox.height)
    };
    let axis_label_gap = if axis_label_height == 0 {
        0
    } else {
        theme.layout.x_labels_to_axis_labels_gap_px
    };

    let bottom_reserved = theme.layout.outer_bottom_px
        + theme.layout.axis_label_to_canvas_edge_gap_px
        + axis_label_height
        + axis_label_gap
        + max_x_label_height
        + theme.layout.x_tick_to_label_gap_px
        + theme.axis.x_tick_length_px;

    let plot_area_bottom = h - bottom_reserved;
    let available_h = plot_area_bottom - preliminary_plot_area_top;
    if available_h <= 0 {
        return Err(anyhow!(
            "not enough vertical room after measured labels and margins"
        ));
    }

    let inner_available_h = available_h
        - theme.layout.plot_min_inner_top_gap_px
        - theme.layout.plot_min_inner_bottom_gap_px;
    if inner_available_h <= 0 {
        return Err(anyhow!("not enough inner vertical room for plot"));
    }

    let px_per_cent = match theme.scale.force_px_per_cent {
        Some(px) => px,
        None => (inner_available_h / y_span_cents)
            .max(theme.scale.min_px_per_cent)
            .min(theme.scale.max_px_per_cent),
    };

    let plot_height = y_span_cents * px_per_cent;
    if plot_height > inner_available_h {
        return Err(anyhow!(
            "data range needs {plot_height}px, but only {inner_available_h}px are available; reduce force_px_per_cent or increase canvas height"
        ));
    }

    let plot_top = preliminary_plot_area_top
        + theme.layout.plot_min_inner_top_gap_px
        + (inner_available_h - plot_height) / 2;
    let plot_bottom = plot_top + plot_height;

    let subtitle = format!(
        "escala vertical: {} px = R$ 0,01 • pernas importantes: ≥ {} • detectadas: {}",
        px_per_cent,
        format_brl_cent(theme.movement.important_delta_cents),
        moves.len()
    );
    let subtitle_bbox = metrics.measure(
        &subtitle,
        theme.typography.subtitle_font_px,
        TextRotation::None,
        &theme.typography,
    );
    let subtitle_pos = PositionedText {
        text: subtitle,
        x: plot_left,
        y: preliminary_subtitle_y,
        bbox: subtitle_bbox,
    };

    let mut y_ticks = Vec::with_capacity(y_tick_cents.len());
    for (cents, label) in y_tick_cents.into_iter().zip(y_tick_labels.into_iter()) {
        let y = y_of_cents(cents, y_min_cent, px_per_cent, plot_bottom);
        let bbox = metrics.measure(
            &label,
            theme.typography.axis_tick_font_px,
            TextRotation::None,
            &theme.typography,
        );
        y_ticks.push(YTickLayout {
            cents,
            y,
            label: PositionedText {
                text: label,
                x: plot_left - theme.layout.y_tick_label_to_axis_gap_px - bbox.width,
                y: y - bbox.height / 2,
                bbox,
            },
        });
    }

    let mut x_ticks = Vec::with_capacity(x_indices.len());
    for idx in x_indices {
        let x = x_of_index(idx, quotes.len(), plot_left, plot_width);
        let label = quotes[idx].date.format("%d/%m").to_string();
        let bbox = metrics.measure(
            &label,
            theme.typography.axis_tick_font_px,
            TextRotation::Rotate270,
            &theme.typography,
        );
        x_ticks.push(XTickLayout {
            idx,
            x,
            label: PositionedText {
                text: label,
                x: x - bbox.width / 2,
                y: plot_bottom
                    + theme.axis.x_tick_length_px
                    + theme.layout.x_tick_to_label_gap_px
                    + bbox.height,
                bbox,
            },
        });
    }

    let axis_label_y = h
        - theme.layout.outer_bottom_px
        - theme.layout.axis_label_to_canvas_edge_gap_px
        - axis_label_height;
    let mut axis_labels = Vec::new();
    if !theme.axis.y_label.is_empty() {
        axis_labels.push(PositionedText {
            text: theme.axis.y_label.to_string(),
            x: theme.layout.outer_left_px,
            y: axis_label_y,
            bbox: y_axis_label_bbox,
        });
    }
    if !theme.axis.x_label.is_empty() {
        axis_labels.push(PositionedText {
            text: theme.axis.x_label.to_string(),
            x: plot_right - x_axis_label_bbox.width,
            y: axis_label_y,
            bbox: x_axis_label_bbox,
        });
    }

    Ok(ChartLayout {
        width,
        height,
        plot_left,
        plot_right,
        plot_top,
        plot_bottom,
        plot_width,
        plot_height,
        y_min_cent,
        y_max_cent,
        px_per_cent,
        title: title_pos,
        subtitle: subtitle_pos,
        y_ticks,
        x_ticks,
        axis_labels,
    })
}

pub fn x_of_index(idx: usize, n: usize, plot_left: i32, plot_width: i32) -> i32 {
    plot_left + ((idx as f64 / (n - 1) as f64) * plot_width as f64).round() as i32
}

pub fn y_of_price(price: f64, y_min_cent: i32, px_per_cent: i32, plot_bottom: i32) -> i32 {
    let cents_from_min = price * 100.0 - y_min_cent as f64;
    plot_bottom - (cents_from_min * px_per_cent as f64).round() as i32
}

fn y_of_cents(cents: i32, y_min_cent: i32, px_per_cent: i32, plot_bottom: i32) -> i32 {
    plot_bottom - ((cents - y_min_cent) * px_per_cent)
}

fn build_y_tick_cents(y_min_cent: i32, y_max_cent: i32) -> Vec<i32> {
    let span = (y_max_cent - y_min_cent).max(1);
    let tick_cents = nice_cent_tick(span);
    let first_tick = div_ceil(y_min_cent, tick_cents) * tick_cents;

    let mut out = Vec::new();
    let mut c = first_tick;
    while c <= y_max_cent {
        out.push(c);
        c += tick_cents;
    }
    out
}

fn choose_evenly_spaced_indices(
    n: usize,
    max_labels: usize,
    plot_width: i32,
    min_gap_px: i32,
) -> Vec<usize> {
    if n == 0 {
        return Vec::new();
    }
    if n == 1 {
        return vec![0];
    }

    let by_gap = ((plot_width / min_gap_px.max(1)) + 1).max(2) as usize;
    let target = max_labels.max(2).min(by_gap).min(n);
    if target == 2 {
        return vec![0, n - 1];
    }

    let mut out = Vec::with_capacity(target);
    for k in 0..target {
        let idx = ((k as f64 * (n - 1) as f64) / (target - 1) as f64).round() as usize;
        if out.last().copied() != Some(idx) {
            out.push(idx);
        }
    }
    if out.first().copied() != Some(0) {
        out.insert(0, 0);
    }
    if out.last().copied() != Some(n - 1) {
        out.push(n - 1);
    }
    out
}

fn nice_cent_tick(span: i32) -> i32 {
    match span {
        0..=20 => 2,
        21..=50 => 5,
        51..=120 => 10,
        121..=250 => 25,
        _ => 50,
    }
}

fn div_ceil(a: i32, b: i32) -> i32 {
    if a >= 0 { (a + b - 1) / b } else { a / b }
}

pub fn format_brl_cent(cents: i32) -> String {
    let sign = if cents < 0 { "-" } else { "" };
    let abs = cents.abs();
    format!("{}R$ {}.{:02}", sign, abs / 100, abs % 100).replace('.', ",")
}
