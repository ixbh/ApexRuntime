//! `preflight` — the Spreadsheet Method's mandatory check, sized for this
//! project. Run it before every commit:
//!
//! ```text
//! cargo run -p apex-tables --bin preflight            # report
//! cargo run -p apex-tables --bin preflight -- --strict # also fail on unfinished mechanics
//! ```
//!
//! 1. `sheets/mechanics.tsv`: every cell must be `y`, `n` or `-`. Each `n`
//!    is an unfinished row (unimplemented, untested, unmeasured, …).
//! 2. `sheets/constants.tsv` overlapped with `apex-core/src/config.rs`:
//!    `used=yes` rows must be read by the config, `used=no` rows must not.
//! 3. Confidence summary: which numbers the movement currently rests on.

use std::path::PathBuf;
use std::process::ExitCode;

use apex_tables::Table;

const MECH_COLS: [&str; 6] = ["doc", "config", "impl", "tests", "telemetry", "measured"];

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    let mut root = PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../.."));
    let mut strict = false;
    while let Some(a) = args.next() {
        match a.as_str() {
            "--strict" => strict = true,
            "--root" => root = args.next().map(PathBuf::from).unwrap_or(root),
            _ => {}
        }
    }
    let mut errors = Vec::<String>::new();

    // --- 1. Mechanics coverage -------------------------------------------
    let mech = match Table::load(&root.join("sheets/mechanics.tsv")) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("cannot read sheets/mechanics.tsv: {e}");
            return ExitCode::FAILURE;
        }
    };
    for c in MECH_COLS {
        if mech.col(c).is_none() {
            errors.push(format!("mechanics.tsv: missing column {c}"));
        }
    }
    let mut unfinished = 0;
    let mut open_cells = 0;
    println!(
        "MECHANICS  ({} rows; n = unchecked, - = not applicable)\n",
        mech.rows.len()
    );
    println!(
        "{:<24} {}",
        "id",
        MECH_COLS.map(|c| format!("{c:<10}")).concat()
    );
    for row in &mech.rows {
        let id = mech.cell(row, "id").unwrap_or("?");
        let mut line = format!("{id:<24} ");
        let mut missing = 0;
        for c in MECH_COLS {
            let v = mech.cell(row, c).unwrap_or("");
            match v {
                "y" => line.push_str("✓         "),
                "-" => line.push_str("·         "),
                "n" => {
                    line.push_str("✗         ");
                    missing += 1;
                }
                other => {
                    line.push_str("?         ");
                    errors.push(format!("mechanics.tsv {id}.{c}: {other:?} is not y/n/-"));
                }
            }
        }
        if missing > 0 {
            unfinished += 1;
            open_cells += missing;
        }
        println!("{line}");
    }
    let done = mech.rows.len() - unfinished;
    println!(
        "\n{done}/{} mechanics complete, {open_cells} open cells\n",
        mech.rows.len()
    );

    // --- 2. Constants ↔ config overlap -----------------------------------
    let consts = Table::load(&root.join("sheets/constants.tsv")).unwrap_or_default();
    let config =
        std::fs::read_to_string(root.join("crates/apex-core/src/config.rs")).unwrap_or_default();
    let mut by_conf = std::collections::BTreeMap::<String, Vec<String>>::new();
    for row in &consts.rows {
        let id = consts.cell(row, "id").unwrap_or("?");
        let used = consts.cell(row, "used") == Some("yes");
        let referenced = config.contains(&format!("sheet::{}", id.to_ascii_uppercase()));
        if used && !referenced {
            errors.push(format!(
                "constants.tsv {id}: used=yes but config.rs never reads sheet::{}",
                id.to_ascii_uppercase()
            ));
        }
        if !used && referenced {
            errors.push(format!(
                "constants.tsv {id}: used=no but config.rs reads it"
            ));
        }
        if used {
            let conf = consts.cell(row, "confidence").unwrap_or("?").to_owned();
            by_conf.entry(conf).or_default().push(id.to_owned());
        }
    }
    println!("CONSTANTS IN USE BY CONFIDENCE");
    for conf in ["high", "medium", "low", "none"] {
        let ids = by_conf.get(conf).map(|v| v.join(", ")).unwrap_or_default();
        println!(
            "  {conf:<7} {:>2}  {ids}",
            by_conf.get(conf).map_or(0, Vec::len)
        );
    }

    // --- Verdict ----------------------------------------------------------
    println!();
    if !errors.is_empty() {
        println!("PREFLIGHT FAILED ({} errors)", errors.len());
        for e in &errors {
            println!("  ✗ {e}");
        }
        return ExitCode::FAILURE;
    }
    if strict && unfinished > 0 {
        println!("PREFLIGHT FAILED (--strict): {unfinished} unfinished mechanics");
        return ExitCode::FAILURE;
    }
    println!("PREFLIGHT OK (sheets well-formed, constants and config agree)");
    ExitCode::SUCCESS
}
