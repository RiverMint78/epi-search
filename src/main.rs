use clap::Parser;
use indicatif::{ProgressBar, ProgressStyle};
use rayon::prelude::*;
use std::time::Instant;

/// Format a number with comma separators
fn fmt_num(n: usize) -> String {
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

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Target value T to approximate
    #[arg(short = 'V', long, default_value_t = 20.0)]
    target: f64,

    /// Maximum number of operators per term
    #[arg(short, long, default_value_t = 4)]
    max_ops: usize,

    /// Number of terms to combine (iterations)
    #[arg(short = 'n', long, default_value_t = 3)]
    terms: usize,

    /// Number of expressions to keep in the pool
    #[arg(short, long, default_value_t = 20)]
    top_k: usize,

    /// Workspace size for each block search
    #[arg(short, long, default_value_t = 5000000)]
    workspace_size: usize,

    /// Max generated expressions per level in block search
    #[arg(short, long, default_value_t = 50000000)]
    gen_limit: usize,

    /// Initial constants to use (e, pi, phi, sqrt2, ln2)
    #[arg(
        short,
        long,
        value_delimiter = ',',
        default_value = "e,pi,phi,sqrt2,ln2"
    )]
    constants: Vec<String>,

    /// Number of results to show
    #[arg(short, long, default_value_t = 5)]
    results: usize,
}

/// Run a block search
fn run_block_search(
    target: f64,
    init_nodes: Vec<std::sync::Arc<epi_search::MathNode>>,
    max_ops: usize,
    workspace_size: usize,
    gen_limit: usize,
    label: &str,
    silent: bool,
) -> Vec<std::sync::Arc<epi_search::MathNode>> {
    let mut limits = Vec::with_capacity(max_ops);
    let sum_weights: usize = (1..=max_ops).sum();
    for k in 1..=max_ops {
        let weight = max_ops - k + 1;
        let limit = (workspace_size as f64 * weight as f64 / sum_weights as f64) as usize;
        limits.push(limit.max(1));
    }

    let mut env = epi_search::SearchEnv::new(target, init_nodes, limits, gen_limit);

    if !silent {
        for k in 1..=max_ops {
            let total_combinations = env.next_level_combinations();
            let pb = ProgressBar::new(total_combinations as u64);
            pb.set_style(
                ProgressStyle::default_bar()
                    .template(&format!(
                        "  [{label}] depth {{msg}} [{{bar:30}}] {{pos}}/{{len}}"
                    ))
                    .unwrap()
                    .progress_chars("=> "),
            );
            pb.set_message(format!("{k}/{max_ops}"));

            env.search_next_level(
                |inc| pb.inc(inc),
                |_status| {
                    pb.finish_and_clear();
                },
            );
        }
    } else {
        for _ in 1..=max_ops {
            env.search_next_level(|_| (), |_| ());
        }
    }

    let mut all_candidates = Vec::new();
    for level in &env.levels {
        all_candidates.extend(level.iter().cloned());
    }

    all_candidates.par_sort_unstable_by(|a, b| {
        let diff_a = (a.val - target).abs();
        let diff_b = (b.val - target).abs();
        if (diff_a - diff_b).abs() < 1e-18 {
            a.complexity.cmp(&b.complexity)
        } else {
            diff_a
                .partial_cmp(&diff_b)
                .unwrap_or(std::cmp::Ordering::Equal)
        }
    });

    all_candidates.dedup_by(|a, b| (a.val - b.val).abs() < 1e-18);
    all_candidates
}

fn main() {
    let args = Args::parse();
    let target = args.target;
    let max_ops = args.max_ops;
    let terms_count = args.terms;
    let top_k = args.top_k;
    let workspace_size = args.workspace_size;
    let gen_limit = args.gen_limit;
    let results = args.results;

    println!();
    println!("  epi-search (Targeted Iterative Search)");
    println!("  -------------------------------------");
    println!("  target:    {}", target);
    println!("  ops/term:  {}", max_ops);
    println!("  terms:     {}", terms_count);
    println!("  top_k:     {}", top_k);
    println!("  results:   {}", results);
    println!(
        "  workspace: {} (per block search)",
        fmt_num(workspace_size)
    );
    println!("  constants: {:?}", args.constants);
    println!();

    let start_time = Instant::now();

    // Helper to build init nodes
    let get_init_nodes = |names: &[String]| {
        let mut nodes = Vec::new();
        for name in names {
            match name.to_lowercase().as_str() {
                "e" => nodes.push(std::sync::Arc::new(epi_search::MathNode {
                    tree: epi_search::ExprTree::Leaf("e"),
                    val: epi_search::E_VAL,
                    complexity: 0,
                })),
                "pi" => nodes.push(std::sync::Arc::new(epi_search::MathNode {
                    tree: epi_search::ExprTree::Leaf("pi"),
                    val: epi_search::PI_VAL,
                    complexity: 0,
                })),
                "phi" => nodes.push(std::sync::Arc::new(epi_search::MathNode {
                    tree: epi_search::ExprTree::Leaf("phi"),
                    val: epi_search::PHI_VAL,
                    complexity: 0,
                })),
                "sqrt2" => nodes.push(std::sync::Arc::new(epi_search::MathNode {
                    tree: epi_search::ExprTree::Leaf("sqrt2"),
                    val: epi_search::SQRT2_VAL,
                    complexity: 0,
                })),
                "ln2" => nodes.push(std::sync::Arc::new(epi_search::MathNode {
                    tree: epi_search::ExprTree::Leaf("ln2"),
                    val: epi_search::LN2_VAL,
                    complexity: 0,
                })),
                _ => {}
            }
        }
        if nodes.is_empty() {
            // Fallback
            nodes.push(std::sync::Arc::new(epi_search::MathNode {
                tree: epi_search::ExprTree::Leaf("e"),
                val: epi_search::E_VAL,
                complexity: 0,
            }));
        }
        nodes
    };

    let init_nodes = get_init_nodes(&args.constants);

    // Initial pool
    println!("  Generating initial pool...");
    let mut pool = run_block_search(
        target,
        init_nodes.clone(),
        max_ops,
        workspace_size,
        gen_limit,
        "Initial",
        false,
    );
    if pool.len() > top_k {
        pool.truncate(top_k);
    }

    let mut best_match = pool[0].clone();
    let mut best_err = (best_match.val - target).abs();

    // Iterative refinement
    for step in 2..=terms_count {
        println!("\n  Iterative step {}/{} ...", step, terms_count);

        use rayon::prelude::*;

        let pb = ProgressBar::new(pool.len() as u64);
        pb.set_style(
            ProgressStyle::default_bar()
                .template("  [Refining] {bar:30} {pos}/{len} ({eta})")
                .unwrap()
                .progress_chars("=> "),
        );

        // For each expression in the pool, search for a targeted correction
        let candidates: Vec<std::sync::Arc<epi_search::MathNode>> = pool
            .par_iter()
            .flat_map(|expr| {
                let mut local_results = Vec::with_capacity(top_k * 2);

                let sub_workspace = workspace_size / rayon::current_num_threads();
                let sub_gen_limit = gen_limit / rayon::current_num_threads();

                // Target 1: T approx target - expr (for expr + T)
                let residual_add = target - expr.val;
                let add_corrections = run_block_search(
                    residual_add,
                    init_nodes.clone(),
                    max_ops,
                    sub_workspace,
                    sub_gen_limit,
                    "",
                    true,
                );

                for t in add_corrections.iter().take(top_k) {
                    let new_val = expr.val + t.val;
                    if new_val.is_finite() {
                        local_results.push(std::sync::Arc::new(epi_search::MathNode {
                            tree: epi_search::ExprTree::Node('+', expr.clone(), t.clone()),
                            val: new_val,
                            complexity: expr.complexity + t.complexity + 1,
                        }));
                    }
                }

                // Target 2: T approx expr - target (for expr - T)
                let residual_sub = expr.val - target;
                let sub_corrections = run_block_search(
                    residual_sub,
                    init_nodes.clone(),
                    max_ops,
                    sub_workspace,
                    sub_gen_limit,
                    "",
                    true,
                );

                for t in sub_corrections.iter().take(top_k) {
                    let new_val = expr.val - t.val;
                    if new_val.is_finite() {
                        local_results.push(std::sync::Arc::new(epi_search::MathNode {
                            tree: epi_search::ExprTree::Node('-', expr.clone(), t.clone()),
                            val: new_val,
                            complexity: expr.complexity + t.complexity + 1,
                        }));
                    }
                }

                pb.inc(1);
                local_results
            })
            .collect();

        pb.finish_and_clear();

        // Update pool with best candidates
        let mut combined_pool = candidates;
        combined_pool.par_sort_unstable_by(|a, b| {
            let diff_a = (a.val - target).abs();
            let diff_b = (b.val - target).abs();
            if (diff_a - diff_b).abs() < 1e-18 {
                a.complexity.cmp(&b.complexity)
            } else {
                diff_a
                    .partial_cmp(&diff_b)
                    .unwrap_or(std::cmp::Ordering::Equal)
            }
        });

        // Deduplicate by value
        combined_pool.dedup_by(|a, b| (a.val - b.val).abs() < 1e-18);

        combined_pool.truncate(top_k);
        pool = combined_pool;

        if let Some(best) = pool.first() {
            let err = best.val - target;
            let abs_err = err.abs();
            if abs_err < best_err {
                best_err = abs_err;
                best_match = best.clone();
            }
            println!("  Best error after step {}: {:+.e}", step, err);
            println!("  Expr: {}", best_match.to_string());
        }
    }

    let elapsed = start_time.elapsed();

    // Final sort: prefer val >= target, then by abs error
    pool.sort_by(|a, b| {
        let a_gt = a.val >= target;
        let b_gt = b.val >= target;
        if a_gt != b_gt {
            // Prefer true (>= target) over false (< target)
            b_gt.cmp(&a_gt)
        } else {
            // If both same side, prefer smaller absolute error
            (a.val - target)
                .abs()
                .partial_cmp(&(b.val - target).abs())
                .unwrap_or(std::cmp::Ordering::Equal)
        }
    });

    println!();
    println!("  result (Top results, prioritized positive error)");
    println!("  -----------------------------------------------");
    println!("  time: {:.2?}", elapsed);
    println!();

    for (idx, node) in pool.iter().enumerate().take(results) {
        let err = node.val - target;
        println!("  #{} [{:+.e}]", idx + 1, err);
        println!("  Value: {}", node.val);
        println!("  Expr:  {}", node.to_string());
        println!();
    }

    std::process::exit(0);
}
