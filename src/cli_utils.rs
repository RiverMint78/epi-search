use std::io::{self, Write};
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

pub fn fmt_num(n: usize) -> String {
    let s = n.to_string();
    let mut result = String::new();
    for (i, ch) in s.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 {
            result.push(',');
        }
        result.push(ch);
    }
    result.chars().rev().collect()
}

fn fmt_duration(d: Duration) -> String {
    let total_secs = d.as_secs();
    let hours = total_secs / 3600;
    let minutes = (total_secs % 3600) / 60;
    let seconds = total_secs % 60;

    if hours > 0 {
        format!("{hours:02}:{minutes:02}:{seconds:02}")
    } else {
        format!("{minutes:02}:{seconds:02}")
    }
}

pub fn get_max_ops(max_ops: &[usize], step: usize) -> usize {
    let idx = step - 1;
    if max_ops.len() == 1 {
        max_ops[0]
    } else if idx < max_ops.len() {
        max_ops[idx]
    } else {
        *max_ops.last().unwrap_or(&4)
    }
}

pub fn get_init_nodes(names: &[String]) -> Vec<Arc<epi_search::MathNode>> {
    let mut nodes = Vec::new();
    let mut current_id = 0u32;

    for name in names {
        let key = name.to_ascii_lowercase();
        if let Some((_, leaf, value)) = epi_search::CONSTANT_SPECS
            .iter()
            .find(|(lookup, _, _)| *lookup == key.as_str())
        {
            nodes.push(Arc::new(epi_search::MathNode {
                tree: epi_search::ExprTree::Leaf(leaf),
                val: *value,
                complexity: 0,
                id: current_id,
            }));
            current_id += 1;
        }
    }

    if nodes.is_empty() {
        nodes.push(Arc::new(epi_search::MathNode {
            tree: epi_search::ExprTree::Leaf("e"),
            val: epi_search::E,
            complexity: 0,
            id: 0,
        }));
    }

    nodes
}

pub struct StdProgress {
    prefix: String,
    total: u64,
    rate_unit: &'static str,
    current: AtomicU64,
    start: Instant,
    next_draw_ms: AtomicU64,
    draw_lock: Mutex<()>,
}

impl StdProgress {
    pub fn new(prefix: String, total: u64, rate_unit: &'static str) -> Self {
        Self {
            prefix,
            total,
            rate_unit,
            current: AtomicU64::new(0),
            start: Instant::now(),
            next_draw_ms: AtomicU64::new(0),
            draw_lock: Mutex::new(()),
        }
    }

    pub fn inc(&self, delta: u64) {
        if delta == 0 {
            return;
        }
        let pos = self.current.fetch_add(delta, Ordering::Relaxed) + delta;
        self.maybe_draw(pos, false);
    }

    pub fn finish(&self) {
        let pos = self.current.load(Ordering::Relaxed).min(self.total);
        self.draw(pos, true);
    }

    fn maybe_draw(&self, pos: u64, force: bool) {
        if force {
            self.draw(pos, true);
            return;
        }

        if pos == 0 {
            return;
        }

        let elapsed_ms = self.start.elapsed().as_millis() as u64;
        let next = self.next_draw_ms.load(Ordering::Relaxed);
        if elapsed_ms < next {
            return;
        }

        if self
            .next_draw_ms
            .compare_exchange(next, elapsed_ms + 120, Ordering::Relaxed, Ordering::Relaxed)
            .is_ok()
        {
            self.draw(pos, false);
        }
    }

    fn draw(&self, pos: u64, done: bool) {
        let _guard = self.draw_lock.lock().unwrap();

        let total = self.total.max(1);
        let clamped = pos.min(self.total);
        let pct = clamped as f64 / total as f64;
        let bar_width = 30usize;
        let filled = ((pct * bar_width as f64).round() as usize).min(bar_width);
        let bar = format!("{}{}", "#".repeat(filled), "-".repeat(bar_width - filled));

        let elapsed = self.start.elapsed();
        let eta = if clamped > 0 && clamped < self.total {
            let sec_per_item = elapsed.as_secs_f64() / clamped as f64;
            Duration::from_secs_f64(sec_per_item * (self.total - clamped) as f64)
        } else {
            Duration::ZERO
        };
        let rate = if elapsed.as_secs_f64() > 0.0 {
            clamped as f64 / elapsed.as_secs_f64()
        } else {
            0.0
        };

        print!(
            "\r  {} [{}] {:>6.2}% {}/{} eta {} {:>8.1} {}",
            self.prefix,
            bar,
            pct * 100.0,
            fmt_num(clamped as usize),
            fmt_num(self.total as usize),
            fmt_duration(eta),
            rate,
            self.rate_unit,
        );

        if done {
            println!();
        }

        let _ = io::stdout().flush();
    }
}
