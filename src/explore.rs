/* -----------------------------------------------------------------------------
 * Interactive Diff Explorer
 *
 * Provides a ncurses-style TUI for navigating filesystem changes between
 * two snapshots. Uses a virtual directory tree with cursor state, allowing
 * users to drill down into specific changed paths.
 * -------------------------------------------------------------------------- */

use std::{collections::HashMap, io::Write, path::Path};

fn children<'a>(
    diff: &'a HashMap<String, (i64, u64)>,
    parent: Option<&str>,
) -> Vec<(&'a str, i64, u64)> {
    let prefix = match parent {
        Some(p) => format!("{}/", p),
        None => "/".to_string(),
    };
    let mut out: Vec<(&'a str, i64, u64)> = diff
        .iter()
        .filter_map(|(path, &(d, cur))| {
            if crate::constants::is_excluded(path) {
                return None;
            }
            let rest = path.strip_prefix(&prefix)?;
            if rest.contains('/') || rest.is_empty() {
                return None;
            }
            Some((path.as_str(), d, cur))
        })
        .collect();
    out.sort_by(|a, b| b.1.abs().cmp(&a.1.abs()));
    out
}

/* --- main command --- */

pub fn cmd_explore(idx_a: usize, idx_b: usize) {
    let files = crate::snapshot::cmd_list(false);
    crate::utils::check_indices(&files, &[idx_a, idx_b]);
    let diff = crate::snapshot::build_diff(&files[idx_a], &files[idx_b]);
    if diff.is_empty() {
        println!("No differences found.");
        return;
    }

    let mut stack: Vec<Option<String>> = vec![None];
    let mut cursors: HashMap<Option<String>, usize> = HashMap::new();

    loop {
        let parent = stack.last().unwrap().clone();
        let rows = children(&diff, parent.as_deref());
        let max_idx = rows.len().saturating_sub(1);
        let cur_idx = cursors.get(&parent).copied().unwrap_or(0).min(max_idx);

        crate::terminal::clear();
        println!();
        println!("  PATH : {}", parent.as_deref().unwrap_or("/"));
        println!("  {:<14}  {:<12}  NAME", "CHANGE", "CURRENT");
        println!("  {}", "─".repeat(56));

        if rows.is_empty() {
            println!("  (no changed sub-folders)");
        }

        // Store total change for root path
        let total_change = rows.iter().map(|(_, d, _)| d).sum::<i64>();

        for (i, (path, d, cur)) in rows.iter().enumerate() {
            let name = Path::new(path)
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            let sign = if *d > 0 { '+' } else { '-' };
            let color = if *d > 0 { "\x1b[92m" } else { "\x1b[91m" };
            let reset = "\x1b[0m";
            let sel = if i == cur_idx { "\x1b[7m" } else { "" };
            println!(
                "  {}{}{}{:<13}{}  {:<12}  {}{}",
                sel,
                color,
                sign,
                crate::terminal::fmt_size(d.unsigned_abs() as f64),
                reset,
                crate::terminal::fmt_size(*cur as f64),
                name,
                reset,
            );
        }

        println!("  {}", "─".repeat(56));
        
        // if path is root, show total change
        if parent.is_none() {
            let color = if total_change > 0 { "\x1b[92m" } else { "\x1b[91m" };
            let reset = "\x1b[0m";
            println!("  {}Total change:                 {}{}", color, crate::terminal::fmt_size(total_change as f64), reset);
            println!("  {}", "─".repeat(56));
        }

        println!("  {}", crate::constants::HELP);


        let _ = std::io::stdout().flush();

        match crate::terminal::getch().as_str() {
            "q" | "Q" | "\x03" => break,
            "\x1b[A" | "k" => {
                cursors.insert(parent, cur_idx.saturating_sub(1));
            }
            "\x1b[B" | "j" => {
                cursors.insert(parent, (cur_idx + 1).min(max_idx));
            }
            "\x1b[C" | "\r" | "\n" | "l" if !rows.is_empty() => {
                cursors.insert(parent.clone(), cur_idx);
                stack.push(Some(rows[cur_idx].0.to_string()));
            }
            "\x1b[D" | "b" | "h" | "\x7f" if stack.len() > 1 => {
                cursors.insert(parent, cur_idx);
                stack.pop();
            }
            _ => {
                cursors.insert(parent, cur_idx);
            }
        }
    }
}
