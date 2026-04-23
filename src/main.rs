#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

mod cli_utils;

use clap::Parser;
use cli_utils::{
    StdProgress, clap_styles, cli_good, cli_label, cli_title, fmt_num, get_init_nodes, get_max_ops,
};
use rayon::prelude::*;
use std::sync::Arc;
use std::time::Instant;

#[derive(Parser)]
#[command(
    author,
    version,
    about = "Targeted iterative expression search",
    long_about = "Searches mathematical expressions that approximate a target value by iterative residual correction.\n\nAt each iteration, the solver keeps the top-K best expressions and tries additive/subtractive corrections from a fresh block search. This tends to improve approximation quality while controlling complexity.",
    after_long_help = "Examples:\n  epi-search -V 1.41421356237 --max-ops 4 --terms 3 --constants e,pi,sqrt2\n  epi-search -V 0.915965 --max-ops 3,4,5 --terms 4 --top-k 30 --results 10\n\nTips:\n  - Use comma-separated --max-ops for per-step depth control (e.g. 3,4,5).\n  - Increase --workspace-size and --gen-limit for quality; lower them for speed.\n  - Add more --constants for a richer search basis.",
    next_line_help = true,
    color = clap::ColorChoice::Auto,
    styles = clap_styles()
)]
struct Args {
    /// Target value T to approximate
    #[arg(short = 'T', long)]
    target: f64,

    /// Maximum number of operators per term (comma separated for each iteration)
    #[arg(short = 'm', long, value_delimiter = ',', default_value = "4")]
    max_ops: Vec<usize>,

    /// Number of terms to combine (iterations)
    #[arg(short = 'n', long, default_value_t = 3)]
    terms: usize,

    /// Number of expressions to keep in the pool
    #[arg(short = 'k', long, default_value_t = 20)]
    top_k: usize,

    /// Workspace size for each block search
    #[arg(short = 'w', long, default_value_t = 5_000_000)]
    workspace_size: usize,

    /// Max generated expressions per level in block search
    #[arg(short = 'g', long, default_value_t = 100_000_000)]
    gen_limit: usize,

    /// Initial constants to use (e, pi, phi, sqrt2, ln2, gamma, C, zeta3, A, delta, alpha, 1)
    #[arg(short = 'c', long, value_delimiter = ',', default_value = "e,pi")]
    constants: Vec<String>,

    /// Number of results to show
    #[arg(short = 'r', long, default_value_t = 5)]
    result_cnt: usize,
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
        let weight = k;
        let limit = (workspace_size as f64 * weight as f64 / sum_weights as f64) as usize;
        limits.push(limit.max(1));
    }

    let mut env = epi_search::SearchEnv::new(target, init_nodes, limits, gen_limit);

    if !silent {
        for k in 1..=max_ops {
            let total_combinations = env.next_level_combinations();
            let progress = StdProgress::new(
                format!("[{label}] depth {k}/{max_ops}"),
                total_combinations as u64,
                "comb/s",
            );

            env.search_next_level(|inc| progress.inc(inc as u64), |_status| ());
            progress.finish();
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
    let terms_count = args.terms;
    let top_k = args.top_k;
    let workspace_size = args.workspace_size;
    let gen_limit = args.gen_limit;
    let result_cnt = args.result_cnt;

    println!();
    println!("  {}", cli_title("epi-search (Targeted Iterative Search)"));
    println!("  {}", cli_title("-------------------------------------"));
    println!("  {} {}", cli_label("target:"), target);
    println!("  {} {:?}", cli_label("ops/term:"), args.max_ops);
    println!("  {} {}", cli_label("terms:"), terms_count);
    println!("  {} {}", cli_label("top_k:"), top_k);
    println!("  {} {}", cli_label("results:"), result_cnt);
    println!(
        "  {} {} (per block search)",
        cli_label("workspace:"),
        fmt_num(workspace_size)
    );
    println!("  {} {:?}", cli_label("constants:"), args.constants);
    println!();

    let start_time = Instant::now();

    let init_nodes = get_init_nodes(&args.constants);

    // Initial pool
    println!("  {}", cli_title("Generating initial pool..."));
    let mut pool = run_block_search(
        target,
        init_nodes.clone(),
        get_max_ops(&args.max_ops, 1),
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
        println!(
            "\n  {} {}/{} ...",
            cli_title("Iterative step"),
            step,
            terms_count
        );

        use rayon::prelude::*;

        let progress = Arc::new(StdProgress::new(
            "[Refining]".to_string(),
            pool.len() as u64,
            "expr/s",
        ));

        // For each expression in the pool, search for a targeted correction
        let candidates: Vec<std::sync::Arc<epi_search::MathNode>> = pool
            .par_iter()
            .flat_map(|expr| {
                let mut local_results = Vec::with_capacity(top_k * 2);

                let sub_workspace = workspace_size / rayon::current_num_threads();
                let sub_gen_limit = gen_limit / rayon::current_num_threads();

                let current_max_ops = get_max_ops(&args.max_ops, step);

                // Target 1: T approx target - expr (for expr + T)
                let residual_add = target - expr.val;
                let add_corrections = run_block_search(
                    residual_add,
                    init_nodes.clone(),
                    current_max_ops,
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
                            id: 0,
                        }));
                    }
                }

                // Target 2: T approx expr - target (for expr - T)
                let residual_sub = expr.val - target;
                let sub_corrections = run_block_search(
                    residual_sub,
                    init_nodes.clone(),
                    current_max_ops,
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
                            id: 0,
                        }));
                    }
                }

                progress.inc(1);
                local_results
            })
            .collect();

        progress.finish();

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
            println!(
                "  {} {}: {}",
                cli_label("Best error after step"),
                step,
                cli_good(&format!("{:+.e}", err))
            );
            println!("  {} {}", cli_label("Expr:"), best_match.to_string());
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
    println!("  {}", cli_title("result (positive first)"));
    println!("  {}", cli_title("-----------------------------------------------"));
    println!("  {} {:.2?}", cli_label("time:"), elapsed);
    println!();

    for (idx, node) in pool.iter().enumerate().take(result_cnt) {
        let err = node.val - target;
        println!("  {} [{}]", cli_title(&format!("#{}", idx + 1)), cli_good(&format!("{:+.e}", err)));
        println!("  {} {}", cli_label("Value:"), node.val);
        println!("  {}  {}", cli_label("Expr:"), node.to_string());
        println!();
    }

    std::process::exit(0);
}
