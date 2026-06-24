use super::data::Quote;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MoveKind {
    Rise,
    Fall,
}

#[derive(Clone, Debug)]
pub struct ImportantMove {
    pub start_idx: usize,
    pub end_idx: usize,
    pub delta: f64,
    pub kind: MoveKind,
}

pub fn detect_important_moves(quotes: &[Quote], threshold_cents: i32) -> Vec<ImportantMove> {
    if quotes.len() < 2 {
        return Vec::new();
    }

    let threshold = threshold_cents as f64 / 100.0;
    let eps = 0.000_000_1;
    let mut out = Vec::new();

    let mut run_start = 0usize;
    let mut run_delta = 0.0f64;
    let mut direction = 0i8;

    for i in 1..quotes.len() {
        let step = quotes[i].usd_brl - quotes[i - 1].usd_brl;
        let step_direction = if step > eps {
            1
        } else if step < -eps {
            -1
        } else {
            0
        };

        if step_direction == 0 {
            maybe_push_move(&mut out, run_start, i - 1, run_delta, threshold, eps);
            run_start = i;
            run_delta = 0.0;
            direction = 0;
            continue;
        }

        if direction == 0 {
            run_start = i - 1;
            run_delta = step;
            direction = step_direction;
            continue;
        }

        if step_direction == direction {
            run_delta += step;
        } else {
            maybe_push_move(&mut out, run_start, i - 1, run_delta, threshold, eps);
            run_start = i - 1;
            run_delta = step;
            direction = step_direction;
        }
    }

    maybe_push_move(
        &mut out,
        run_start,
        quotes.len() - 1,
        run_delta,
        threshold,
        eps,
    );

    out
}

fn maybe_push_move(
    out: &mut Vec<ImportantMove>,
    start_idx: usize,
    end_idx: usize,
    delta: f64,
    threshold: f64,
    eps: f64,
) {
    if end_idx <= start_idx || delta.abs() + eps < threshold {
        return;
    }

    out.push(ImportantMove {
        start_idx,
        end_idx,
        delta,
        kind: if delta >= 0.0 {
            MoveKind::Rise
        } else {
            MoveKind::Fall
        },
    });
}
