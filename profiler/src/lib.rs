use std::{
    collections::HashMap,
    sync::{Mutex, OnceLock},
    time::{Duration, Instant},
};

use tracing::trace;

#[derive(Default)]
struct Stat {
    last: Option<Instant>,
    interval_sum: Duration,
    interval_deltas: u64,
    duration_sum: Duration,
    duration_count: u64,
    triggers: u64,
}

struct Profiler {
    stats: Mutex<HashMap<String, Stat>>,
    last_report: Mutex<Instant>,
}

static GLOBAL: OnceLock<Profiler> = OnceLock::new();

fn global() -> &'static Profiler {
    GLOBAL.get_or_init(Profiler::default)
}

impl Default for Profiler {
    fn default() -> Self {
        Self {
            stats: Mutex::new(HashMap::new()),
            last_report: Mutex::new(Instant::now()),
        }
    }
}

#[derive(Debug)]
pub struct SpanGuard {
    event: String,
    start: Instant,
}

impl Drop for SpanGuard {
    fn drop(&mut self) {
        record_duration(&self.event, self.start.elapsed());
    }
}

pub fn record(event: &str) {
    let profiler = global();
    let now = Instant::now();
    let mut stats = profiler.stats.lock().expect("profiler stats lock poisoned");
    let entry = stats.entry(event.to_string()).or_default();
    entry.triggers = entry.triggers.saturating_add(1);
    if let Some(last) = entry.last {
        entry.interval_sum += now.saturating_duration_since(last);
        entry.interval_deltas = entry.interval_deltas.saturating_add(1);
    }
    entry.last = Some(now);
}

pub fn record_duration(event: &str, duration: Duration) {
    let profiler = global();
    let mut stats = profiler.stats.lock().expect("profiler stats lock poisoned");
    let entry = stats.entry(event.to_string()).or_default();
    entry.triggers = entry.triggers.saturating_add(1);
    entry.duration_sum += duration;
    entry.duration_count = entry.duration_count.saturating_add(1);
}

pub fn span(event: &str) -> SpanGuard {
    SpanGuard {
        event: event.to_string(),
        start: Instant::now(),
    }
}

pub fn report_if_due() {
    let profiler = global();
    let now = Instant::now();
    let mut last_report = profiler.last_report.lock().expect("profiler report lock poisoned");
    let elapsed = now.saturating_duration_since(*last_report);
    if elapsed < Duration::from_secs(1) {
        return;
    }
    let elapsed_secs = elapsed.as_secs_f64();

    let mut stats = profiler.stats.lock().expect("profiler stats lock poisoned");
    for (event, stat) in stats.iter_mut() {
        if stat.triggers == 0 {
            continue;
        }
        let hz = stat.triggers as f64 / elapsed_secs;
        let avg_interval_ms = if stat.interval_deltas > 0 {
            stat.interval_sum.as_secs_f64() * 1000.0 / stat.interval_deltas as f64
        } else {
            0.0
        };
        let avg_duration_ms = if stat.duration_count > 0 {
            stat.duration_sum.as_secs_f64() * 1000.0 / stat.duration_count as f64
        } else {
            0.0
        };
        if stat.interval_deltas > 0 && stat.duration_count > 0 {
            trace!(event = %event, avg_interval_ms, avg_duration_ms, hz, "profiler");
        } else if stat.duration_count > 0 {
            trace!(event = %event, avg_duration_ms, hz, "profiler");
        } else if stat.interval_deltas > 0 {
            trace!(event = %event, avg_interval_ms, hz, "profiler");
        } else {
            trace!(event = %event, hz, "profiler");
        }
        stat.interval_sum = Duration::ZERO;
        stat.interval_deltas = 0;
        stat.duration_sum = Duration::ZERO;
        stat.duration_count = 0;
        stat.triggers = 0;
    }

    *last_report = now;
}
