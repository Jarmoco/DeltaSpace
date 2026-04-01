/* -----------------------------------------------------------------------------
 * explore/mod.rs
 * Orchestrates the interactive diff explorer, dispatching to chart, tree,
 * and deletion sub-modules.
 * -------------------------------------------------------------------------- */

pub mod chart;
pub mod deletion;
pub mod tree;

use std::{collections::HashMap, io::Write};

/* --- Main --- */

pub fn cmd_explore(baseline_index: usize, mut comparison_index: usize) {
    let mut files = crate::snapshot::cmd_list(false);
    crate::utils::check_indices(&files, &[baseline_index, comparison_index]);

    let mut pending_deletions: Vec<String> = Vec::new();

    loop {
        let diff = crate::snapshot::build_diff(&files[baseline_index], &files[comparison_index]);
        if diff.is_empty() {
            println!("No differences found.");
            break;
        }

        let mut snapshots: Vec<HashMap<String, u64>> = Vec::new();
        let mut snapshot_dates: Vec<Option<(i32, u32, u32, u32, u32)>> = Vec::new();
        for i in baseline_index..=comparison_index {
            snapshots.push(crate::snapshot::load_flat(&files[i]));
            snapshot_dates.push(tree::parse_snapshot_datetime(&files[i]));
        }

        let mut stack: Vec<Option<String>> = vec![None];
        let mut cursors: HashMap<Option<String>, usize> = HashMap::new();
        let mut scroll_offsets: HashMap<Option<String>, usize> = HashMap::new();
        let mut chart_visible = true;

        loop {
            let parent = stack.last().unwrap().clone();
            let parent_str = parent.as_deref().unwrap_or("");
            let rows = tree::children(&diff, parent.as_deref());
            let max_idx = rows.len().saturating_sub(1);
            let cursor_index = cursors.get(&parent).copied().unwrap_or(0).min(max_idx);

            let terminal_height = crate::terminal::get_height();
            let header_lines = 4;
            let separator_after_rows = 1;
            let total_change_lines = if parent.is_none() { 2 } else { 0 };
            let chart_block_lines = chart::CHART_ROWS + 2;
            let chart_separator = 1;
            let help_lines = 1;

            let available_rows_no_chart = terminal_height
                .saturating_sub(
                    header_lines + separator_after_rows + total_change_lines + help_lines,
                )
                .max(1);
            let rows_count = rows.len().max(1);
            let actual_rows = available_rows_no_chart.min(rows_count);

            let min_chart_lines = chart_block_lines + chart_separator + help_lines;
            let min_table_lines =
                header_lines + separator_after_rows + actual_rows + total_change_lines + help_lines;
            let show_chart = chart_visible && terminal_height >= min_table_lines + min_chart_lines;

            let bottom_lines = if show_chart {
                min_chart_lines
            } else {
                help_lines
            };
            let available_rows = terminal_height
                .saturating_sub(
                    header_lines + separator_after_rows + total_change_lines + bottom_lines,
                )
                .max(1);

            let scroll_offset = tree::compute_scroll_offset(&scroll_offsets, &parent, max_idx);
            let (has_more_above, has_more_below, data_rows) =
                tree::compute_visible_rows(rows.len(), scroll_offset, available_rows);
            let scroll_offset = scroll_offset.min(rows.len().saturating_sub(data_rows));

            crate::terminal::clear();
            println!();
            println!("  PATH : {}", parent_str);
            println!("  {:<14}  {:<12}  NAME", "CHANGE", "CURRENT");
            println!("  {}", "─".repeat(tree::table_width()));

            let total_change = rows.iter().map(|(_, d, _)| d).sum::<i64>();

            tree::render_table_rows(
                &rows,
                scroll_offset,
                data_rows,
                cursor_index,
                &pending_deletions,
                has_more_above,
                has_more_below,
            );

            println!("  {}", "─".repeat(tree::table_width()));

            if parent.is_none() {
                let color = if total_change > 0 {
                    "\x1b[92m"
                } else {
                    "\x1b[91m"
                };
                let reset = "\x1b[0m";
                println!(
                    "  {}Total change:                 {}{}",
                    color,
                    crate::terminal::fmt_size(total_change as f64),
                    reset
                );
                println!("  {}", "─".repeat(tree::table_width()));
            }

            if show_chart {
                let lines_used = header_lines
                    + separator_after_rows
                    + rows
                        .iter()
                        .enumerate()
                        .skip(scroll_offset)
                        .take(available_rows)
                        .count()
                    + total_change_lines
                    + if rows.is_empty() { 1 } else { 0 };
                let filler = terminal_height.saturating_sub(lines_used + bottom_lines);
                for _ in 0..filler {
                    println!();
                }

                let chart_path = if !rows.is_empty() {
                    rows[cursor_index].0
                } else {
                    parent_str
                };
                let size_over_time = chart::folder_size_over_time(&snapshots, chart_path);
                let interpolated = chart::interpolate(&size_over_time, chart::chart_cols());
                chart::render_chart(&interpolated, &size_over_time, chart_path, &snapshot_dates);
                println!("  {}", "─".repeat(tree::table_width()));
            }

            let pending_count = pending_deletions.len();
            let (help_base, help_extra): (String, Option<String>) = if !show_chart && chart_visible
            {
                let base = if pending_count > 0 {
                    format!("↑↓ move  → drill  ← back  g toggle chart   d: queue delete   x: delete({pending_count})   q quit")
                } else {
                    "↑↓ move  → drill  ← back  g toggle chart   d: queue delete   q quit"
                        .to_string()
                };
                (base, Some("Terminal too small for the chart".to_string()))
            } else if pending_count > 0 {
                (
                    format!("↑↓ move  → drill  ← back  g toggle chart   d: queue delete   x: delete({pending_count})   q quit"),
                    None,
                )
            } else {
                (
                    "↑↓ move  → drill  ← back  g toggle chart   d: queue delete   q quit"
                        .to_string(),
                    None,
                )
            };
            println!("  {help_base}");
            if let Some(extra) = help_extra {
                println!("  \x1b[90m{}\x1b[0m", extra);
            }

            let _ = std::io::stdout().flush();

            match crate::terminal::getch().as_str() {
                "q" | "Q" | "\x03" => {
                    return;
                }
                "\x1b[A" | "k" => {
                    let new_cursor = cursor_index.saturating_sub(1);
                    cursors.insert(parent.clone(), new_cursor);
                    if new_cursor < scroll_offset {
                        scroll_offsets.insert(parent, new_cursor);
                    }
                }
                "\x1b[B" | "j" => {
                    let new_cursor = (cursor_index + 1).min(max_idx);
                    cursors.insert(parent.clone(), new_cursor);

                    let after_more_below = scroll_offset + available_rows < rows.len();
                    let after_indicator = 1 + after_more_below as usize;
                    let after_data = available_rows.saturating_sub(after_indicator);

                    if new_cursor >= scroll_offset + after_data {
                        let new_scroll = new_cursor.saturating_sub(after_data) + 1;
                        scroll_offsets.insert(
                            parent,
                            new_scroll.min(rows.len().saturating_sub(after_data)),
                        );
                    }
                }
                "\x1b[C" | "\r" | "\n" | "l" if !rows.is_empty() => {
                    cursors.insert(parent.clone(), cursor_index);
                    stack.push(Some(rows[cursor_index].0.to_string()));
                }
                "\x1b[D" | "b" | "h" | "\x7f" if stack.len() > 1 => {
                    cursors.insert(parent, cursor_index);
                    stack.pop();
                }
                "g" | "G" => {
                    chart_visible = !chart_visible;
                }
                "d" | "D" if !rows.is_empty() => {
                    let selected_path = rows[cursor_index].0;
                    if !pending_deletions.iter().any(|p| p == selected_path) {
                        pending_deletions.push(selected_path.to_string());
                    }
                }
                "x" | "X" if !pending_deletions.is_empty() => {
                    files = crate::snapshot::cmd_list(false);
                    if comparison_index != files.len() - 1 {
                        println!(
                            "\n  \x1b[93mCan only delete when comparing to the latest snapshot.\x1b[0m"
                        );
                        println!(
                            "  Currently viewing snapshot {} of {}.",
                            comparison_index + 1,
                            files.len()
                        );
                        crate::utils::pause();
                        continue;
                    }

                    deletion::render_deletion_prompt(&pending_deletions);

                    if !deletion::run_deletion_confirmation() {
                        println!("\n  Cancelled.");
                        crate::utils::pause();
                        continue;
                    }

                    let all_succeeded = deletion::execute_deletions(&pending_deletions);

                    if all_succeeded {
                        println!("\n  Creating new snapshot with deletions applied...");
                        let _ = std::io::stdout().flush();
                        if deletion::apply_and_snapshot(
                            &mut comparison_index,
                            &pending_deletions,
                            &mut files,
                        ) {
                            pending_deletions.clear();
                            crate::utils::pause();
                            break;
                        } else {
                            crate::utils::pause();
                        }
                    } else {
                        crate::utils::pause();
                    }
                }
                _ => {
                    cursors.insert(parent, cursor_index);
                }
            }
        }
    }
}
