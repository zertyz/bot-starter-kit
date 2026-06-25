use anyhow::Result;
use std::time::{Duration, Instant};

#[derive(Default, Debug)]
pub struct Timings {
    rows: Vec<(String, Duration)>,
}

impl Timings {
    pub fn measure<T, F>(&mut self, name: impl Into<String>, f: F) -> Result<T>
    where
        F: FnOnce() -> Result<T>,
    {
        let name = name.into();
        let start = Instant::now();
        let out = f();
        self.rows
            .push((name, start.elapsed()));
        out
    }

    pub fn push(&mut self, name: impl Into<String>, duration: Duration) {
        self.rows
            .push((name.into(), duration));
    }

    pub fn print_with_wall(&self, wall: Duration) {
        eprintln!("timings:");
        for (name, duration) in &self.rows {
            eprintln!("  {:<24} {:>9.3} ms", name, duration.as_secs_f64() * 1000.0);
        }
        let measured_sum: Duration = self
            .rows
            .iter()
            .map(|(_, d)| *d)
            .sum();
        eprintln!("  {:<24} {:>9.3} ms", "measured sum", measured_sum.as_secs_f64() * 1000.0);
        eprintln!("  {:<24} {:>9.3} ms", "wall total", wall.as_secs_f64() * 1000.0);
    }
}
