//! `apex-replay` — run an input script against the built-in proving ground
//! and write a per-tick movement trace.
//!
//! ```text
//! cargo run -p apex-telemetry --bin apex-replay -- scripts/inputs/bhop.json
//! cargo run -p apex-telemetry --bin apex-replay -- scripts/inputs/bhop.json --out traces/bhop.csv
//! cargo run -p apex-telemetry --bin apex-replay -- scripts/inputs/bhop.json --out traces/bhop.json
//! cargo run -p apex-telemetry --bin apex-replay -- recorded.input.json --flat   # endless floor
//! ```

use std::fs;
use std::path::PathBuf;
use std::process::ExitCode;

use apex_collision::Brush;
use apex_collision::maps::{flat, proving_ground};
use apex_core::MovementConfig;
use apex_math::Vec3;
use apex_telemetry::{InputScript, write_csv};

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    let mut script_path: Option<PathBuf> = None;
    let mut out: Option<PathBuf> = None;
    let mut flat_world = false;
    // --block X H: flat world plus a block whose near face is at x = X, height H.
    let mut block: Option<(f32, f32)> = None;
    while let Some(a) = args.next() {
        match a.as_str() {
            "--out" | "-o" => out = args.next().map(PathBuf::from),
            "--flat" => flat_world = true,
            "--block" => {
                let x = args.next().and_then(|v| v.parse().ok());
                let h = args.next().and_then(|v| v.parse().ok());
                match (x, h) {
                    (Some(x), Some(h)) => block = Some((x, h)),
                    _ => {
                        eprintln!("--block needs X and H");
                        return ExitCode::FAILURE;
                    }
                }
            }
            "--help" | "-h" => {
                eprintln!(
                    "usage: apex-replay <script.json> [--out trace.csv|trace.json] [--flat | --block X H]"
                );
                return ExitCode::SUCCESS;
            }
            _ => script_path = Some(PathBuf::from(a)),
        }
    }
    let Some(script_path) = script_path else {
        eprintln!(
            "usage: apex-replay <script.json> [--out trace.csv|trace.json] [--flat | --block X H]"
        );
        return ExitCode::FAILURE;
    };

    let text = match fs::read_to_string(&script_path) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("cannot read {}: {e}", script_path.display());
            return ExitCode::FAILURE;
        }
    };
    let script = match InputScript::from_json(&text) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("bad script {}: {e}", script_path.display());
            return ExitCode::FAILURE;
        }
    };

    let world = match block {
        Some((x, h)) => {
            let mut w = flat();
            w.add(Brush::cuboid(
                Vec3::new(x, -100_000.0, 0.0),
                Vec3::new(x + 400.0, 100_000.0, h),
            ));
            w
        }
        None if flat_world => flat(),
        None => proving_ground(),
    };
    let rows = match script.run(&world, &MovementConfig::default()) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::FAILURE;
        }
    };

    let Some(last) = rows.last() else {
        eprintln!("script has zero ticks");
        return ExitCode::FAILURE;
    };
    let peak_speed = rows.iter().map(|r| r.speed_h).fold(0.0_f32, f32::max);
    let jumps = rows.iter().filter(|r| r.jumped).count();
    println!(
        "script      {}",
        if script.name.is_empty() {
            "(unnamed)"
        } else {
            &script.name
        }
    );
    println!("ticks       {}  ({:.3} s simulated)", rows.len(), last.time);
    println!(
        "final pos   ({:.3}, {:.3}, {:.3})",
        last.pos_x, last.pos_y, last.pos_z
    );
    println!(
        "final vel   ({:.3}, {:.3}, {:.3})  |h| = {:.3}",
        last.vel_x, last.vel_y, last.vel_z, last.speed_h
    );
    println!("peak |h|    {peak_speed:.3} u/s");
    println!("jumps       {jumps}");
    println!("mode        {}", last.mode);

    if let Some(out) = out {
        if let Some(dir) = out.parent().filter(|d| !d.as_os_str().is_empty()) {
            let _ = fs::create_dir_all(dir);
        }
        let result = if out
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("json"))
        {
            serde_json::to_vec_pretty(&rows)
                .map_err(std::io::Error::other)
                .and_then(|b| fs::write(&out, b))
        } else {
            fs::File::create(&out).and_then(|f| write_csv(&rows, std::io::BufWriter::new(f)))
        };
        if let Err(e) = result {
            eprintln!("cannot write {}: {e}", out.display());
            return ExitCode::FAILURE;
        }
        println!("trace       {}", out.display());
    }
    ExitCode::SUCCESS
}
