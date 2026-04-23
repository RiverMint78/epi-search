use rayon::prelude::*;
use std::sync::Arc;

pub const ONE: f64 = 1_f64;
pub const E: f64 = std::f64::consts::E;
pub const PI: f64 = std::f64::consts::PI;
pub const PHI: f64 = std::f64::consts::GOLDEN_RATIO;
pub const SQRT2: f64 = std::f64::consts::SQRT_2;
pub const LN2: f64 = std::f64::consts::LN_2;
pub const EULER: f64 = 0.5772156649015329_f64;
pub const CATALAN: f64 = 0.915965594177219_f64;
pub const APERY: f64 = 1.2020569031595942_f64;
pub const GLAISHER: f64 = 1.2824271291006226_f64;
pub const FST_FEIGENBAUM: f64 = 4.66920160910299_f64;
pub const SND_FEIGENBAUM: f64 = 2.5029078750958926_f64;

pub const CONSTANT_SPECS: [(&str, &str, f64); 12] = [
    ("1", "1", ONE),
    ("e", "e", E),
    ("pi", "pi", PI),
    ("phi", "phi", PHI),
    ("sqrt2", "sqrt2", SQRT2),
    ("ln2", "ln2", LN2),
    ("gamma", "gamma", EULER),
    ("c", "C", CATALAN),
    ("zeta3", "zeta3", APERY),
    ("a", "A", GLAISHER),
    ("delta", "delta", FST_FEIGENBAUM),
    ("alpha", "alpha", SND_FEIGENBAUM),
];

#[derive(Clone, Debug)]
pub enum ExprTree {
    Leaf(&'static str),
    Node(char, Arc<MathNode>, Arc<MathNode>),
}

#[derive(Clone, Debug)]
pub struct MathNode {
    pub tree: ExprTree,
    pub val: f64,
    pub complexity: usize,
    pub id: u32,
}

impl MathNode {
    pub fn to_string(&self) -> String {
        match &self.tree {
            ExprTree::Leaf(s) => s.to_string(),
            ExprTree::Node(op, left, right) => {
                format!("({} {} {})", left.to_string(), op, right.to_string())
            }
        }
    }
}

pub struct SearchEnv {
    pub target: f64,
    pub levels: Vec<Vec<Arc<MathNode>>>,
    pub best_match: Arc<MathNode>,
    pub best_diff: f64,
    pub max_nodes: Vec<usize>,
    pub max_gen_nodes: usize,
    pub next_id: u32,
}

impl SearchEnv {
    /// Init search env
    pub fn new(
        target: f64,
        init_nodes: Vec<Arc<MathNode>>,
        max_nodes: Vec<usize>,
        max_gen_nodes: usize,
    ) -> Self {
        let best_match = init_nodes[0].clone();
        let best_diff = (best_match.val - target).abs();

        let mut next_id = 0;
        for node in &init_nodes {
            if node.id >= next_id {
                next_id = node.id + 1;
            }
        }

        Self {
            target,
            levels: vec![init_nodes],
            best_match,
            best_diff,
            max_nodes,
            max_gen_nodes,
            next_id,
        }
    }

    /// Est. total combinations for next level (adjusted by limit)
    pub fn next_level_combinations(&self) -> usize {
        let k = self.levels.len();
        let mut total = 0;
        for i in 0..k {
            let (left_nodes, right_nodes) = self.get_input_slices(i, k - 1 - i);
            total += left_nodes.len() * right_nodes.len();
        }
        total
    }

    /// Get restricted input slices
    fn get_input_slices(&self, i: usize, j: usize) -> (&[Arc<MathNode>], &[Arc<MathNode>]) {
        let k = self.levels.len();
        let left_nodes_all = &self.levels[i];
        let right_nodes_all = &self.levels[j];

        if self.max_gen_nodes > 0 {
            let quota = self.max_gen_nodes / k;
            let pair_limit = (quota / 5).max(1);

            let n_target = (pair_limit as f64).sqrt() as usize;

            let n_left_tmp = left_nodes_all.len().min(n_target);
            let n_right = right_nodes_all.len().min(pair_limit / n_left_tmp.max(1));

            let n_left = left_nodes_all.len().min(pair_limit / n_right.max(1));

            (&left_nodes_all[..n_left], &right_nodes_all[..n_right])
        } else {
            (left_nodes_all.as_slice(), right_nodes_all.as_slice())
        }
    }

    /// Parallel generate next level expressions
    pub fn search_next_level<F, S>(&mut self, progress_cb: F, status_cb: S) -> (usize, usize)
    where
        F: Fn(u64) + Sync + Send,
        S: Fn(&str),
    {
        let k = self.levels.len();
        let target = self.target;
        let progress_cb = Arc::new(progress_cb);

        status_cb("generating");

        // Get thread count for chunking
        let num_threads = rayon::current_num_threads();

        let mut current_level: Vec<Arc<MathNode>> = (0..k)
            .into_par_iter()
            .flat_map(|i| {
                let (left_nodes, right_nodes) = self.get_input_slices(i, k - 1 - i);
                let progress_cb = Arc::clone(&progress_cb);

                let chunk_size = (left_nodes.len() / (num_threads * 8)).max(64);

                left_nodes.par_chunks(chunk_size).flat_map(move |chunk| {
                    let estimated_cap = (chunk.len() * right_nodes.len() * 4).min(32768);
                    let mut local_nodes = Vec::with_capacity(estimated_cap);
                    for l in chunk {
                        for r in right_nodes {
                            let is_ordered = l.id <= r.id;

                            macro_rules! push_node {
                                ($op:expr, $val:expr) => {
                                    let val = $val;
                                    if val.is_finite() {
                                        local_nodes.push(Arc::new(MathNode {
                                            tree: ExprTree::Node($op, Arc::clone(l), Arc::clone(r)),
                                            val,
                                            complexity: l.complexity + r.complexity + 1,
                                            id: 0, // place holder
                                        }));
                                    }
                                };
                            }

                            if is_ordered {
                                push_node!('+', l.val + r.val);
                                push_node!('*', l.val * r.val);
                            }

                            push_node!('-', l.val - r.val);

                            if r.val.abs() > 1e-24_f64 {
                                push_node!('/', l.val / r.val);
                            }

                            if l.val > 0.0_f64 {
                                push_node!('^', l.val.powf(r.val));
                            }
                        }
                    }
                    progress_cb((chunk.len() * right_nodes.len()) as u64);
                    local_nodes
                })
            })
            .collect();

        let generated = current_level.len();

        // Sort and truncate
        let limit = self.max_nodes.get(k - 1).cloned().unwrap_or(usize::MAX);

        status_cb("sorting");
        let cmp_func = |a: &Arc<MathNode>, b: &Arc<MathNode>| {
            let diff_a = (a.val - target).abs();
            let diff_b = (b.val - target).abs();

            if (diff_a - diff_b).abs() < 1e-12_f64 {
                a.complexity.cmp(&b.complexity)
            } else {
                diff_a
                    .partial_cmp(&diff_b)
                    .unwrap_or(std::cmp::Ordering::Equal)
            }
        };

        if current_level.len() > limit {
            current_level.select_nth_unstable_by(limit, cmp_func);
            current_level.truncate(limit);
            current_level.par_sort_unstable_by(cmp_func);
        } else {
            current_level.par_sort_unstable_by(cmp_func);
        }

        for (i, node_arc) in current_level.iter_mut().enumerate() {
            if let Some(node) = Arc::get_mut(node_arc) {
                node.id = self.next_id + i as u32;
            } else {
                panic!("Unexpected Arc sharing detected during id assignment!");
            }
        }

        self.next_id += current_level.len() as u32;

        // Update best match
        if let Some(level_best) = current_level.first() {
            let level_diff = (level_best.val - target).abs();
            if level_diff < self.best_diff {
                self.best_diff = level_diff;
                self.best_match = Arc::clone(level_best);
            }
        }

        let kept = current_level.len();
        self.levels.push(current_level);
        (generated, kept)
    }

    pub fn get_last_level(&self) -> &[Arc<MathNode>] {
        self.levels.last().map(|v| v.as_slice()).unwrap_or(&[])
    }
}
