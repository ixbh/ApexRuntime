//! `trace-diff` — overlap a reference trace with a port trace (Spreadsheet
//! Method §7.5): matching / divergent / missing / extra, first divergence first.
//!
//! ```text
//! trace-diff <reference.csv> <port.csv> [--key tick] [--port-key cmd]
//!            [--cols pos_x=po_x,vel_x=pv_x] [--tol 0.01] [--tol-col vel_x=0.1] [--show 10]
//! ```
//! Without `--cols`, every column name present in both files (except the
//! keys) is compared. Exit code 1 if anything diverges, is missing or extra.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::process::ExitCode;

use apex_tables::{ColumnSpec, Table, overlap};

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    let mut files = Vec::new();
    let (mut key, mut port_key, mut cols, mut tol, mut show) = (
        "tick".to_owned(),
        None::<String>,
        None::<String>,
        1e-3_f64,
        10usize,
    );
    let mut tol_cols = BTreeMap::<String, f64>::new();
    while let Some(a) = args.next() {
        match a.as_str() {
            "--key" => key = args.next().unwrap_or_default(),
            "--port-key" => port_key = args.next(),
            "--cols" => cols = args.next(),
            "--tol" => tol = args.next().and_then(|v| v.parse().ok()).unwrap_or(tol),
            "--tol-col" => {
                for kv in args.next().unwrap_or_default().split(',') {
                    if let Some((k, v)) = kv.split_once('=')
                        && let Ok(v) = v.parse()
                    {
                        tol_cols.insert(k.to_owned(), v);
                    }
                }
            }
            "--show" => show = args.next().and_then(|v| v.parse().ok()).unwrap_or(show),
            _ => files.push(PathBuf::from(a)),
        }
    }
    let [ref_path, port_path] = files.as_slice() else {
        eprintln!(
            "usage: trace-diff <reference> <port> [--key K] [--port-key K] [--cols a=b,..] [--tol T] [--tol-col c=T,..] [--show N]"
        );
        return ExitCode::FAILURE;
    };
    let load = |p: &PathBuf| Table::load(p).map_err(|e| format!("{}: {e}", p.display()));
    let (reference, port) = match (load(ref_path), load(port_path)) {
        (Ok(r), Ok(p)) => (r, p),
        (Err(e), _) | (_, Err(e)) => {
            eprintln!("{e}");
            return ExitCode::FAILURE;
        }
    };
    let port_key = port_key.unwrap_or_else(|| key.clone());

    let pairs: Vec<(String, String)> = match &cols {
        Some(list) => list
            .split(',')
            .map(|c| match c.split_once('=') {
                Some((a, b)) => (a.to_owned(), b.to_owned()),
                None => (c.to_owned(), c.to_owned()),
            })
            .collect(),
        None => reference
            .headers
            .iter()
            .filter(|h| **h != key && **h != port_key && port.col(h).is_some())
            .map(|h| (h.clone(), h.clone()))
            .collect(),
    };
    let specs: Vec<ColumnSpec> = pairs
        .into_iter()
        .map(|(r, p)| ColumnSpec {
            tolerance: *tol_cols.get(&r).unwrap_or(&tol),
            reference: r,
            port: p,
        })
        .collect();

    let report = match overlap(&reference, &key, &port, &port_key, &specs) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::FAILURE;
        }
    };

    println!(
        "reference   {} ({} rows, key {key})",
        ref_path.display(),
        reference.rows.len()
    );
    println!(
        "port        {} ({} rows, key {port_key})",
        port_path.display(),
        port.rows.len()
    );
    println!(
        "missing     {} keys (in reference only = unimplemented)",
        report.missing.len()
    );
    println!(
        "extra       {} keys (in port only = spurious)",
        report.extra.len()
    );
    println!(
        "\n{:<16} {:>9} {:>9}  tol",
        "column", "matching", "divergent"
    );
    for s in &specs {
        let (m, d) = report
            .per_column
            .get(&s.reference)
            .copied()
            .unwrap_or_default();
        println!("{:<16} {m:>9} {d:>9}  {}", s.reference, s.tolerance);
    }
    match report.first_divergence() {
        None => println!("\nno divergence"),
        Some(_) => {
            println!("\nfirst divergences (fix the first; later ones are usually cascade):");
            for d in report.divergent.iter().take(show) {
                println!(
                    "  {key}={:<8} {:<12} ref {:>12.4}  port {:>12.4}  Δ {:+.4}",
                    d.key,
                    d.column,
                    d.reference,
                    d.port,
                    d.port - d.reference
                );
            }
        }
    }
    if report.is_clean() {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}
