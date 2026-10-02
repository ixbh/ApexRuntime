//! `flowstate-trace` — turn R5Flowstate `[MOVE-TRACE]` log lines into CSV and
//! print a quick movement summary.
//!
//! ```text
//! cargo run -p apex-telemetry --bin flowstate-trace -- <message.log> [--out traces/fs.csv]
//!       [--side client|server] [--ent N] [--no-replays]
//! ```

use std::path::PathBuf;
use std::process::ExitCode;

use apex_telemetry::flowstate::{TraceSide, parse_log, write_csv};

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    let (mut input, mut out, mut side, mut ent, mut no_replays) = (None, None, None, None, false);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--out" | "-o" => out = args.next().map(PathBuf::from),
            "--side" => {
                side = match args.next().as_deref() {
                    Some("client") => Some(TraceSide::Client),
                    Some("server") => Some(TraceSide::Server),
                    other => {
                        eprintln!("--side must be client or server, got {other:?}");
                        return ExitCode::FAILURE;
                    }
                }
            }
            "--ent" => ent = args.next().and_then(|v| v.parse::<i64>().ok()),
            "--no-replays" => no_replays = true,
            _ => input = Some(PathBuf::from(a)),
        }
    }
    let Some(input) = input else {
        eprintln!(
            "usage: flowstate-trace <message.log> [--out file.csv] [--side client|server] [--ent N] [--no-replays]"
        );
        return ExitCode::FAILURE;
    };
    let text = match std::fs::read(&input) {
        Ok(b) => String::from_utf8_lossy(&b).into_owned(),
        Err(e) => {
            eprintln!("cannot read {}: {e}", input.display());
            return ExitCode::FAILURE;
        }
    };

    let (mut lines, bad) = parse_log(&text);
    let total = lines.len();
    lines.retain(|l| {
        side.is_none_or(|s| l.side == s)
            && ent.is_none_or(|e| l.ent == e)
            && !(no_replays && l.replay)
    });

    println!("file        {}", input.display());
    println!(
        "parsed      {total} lines ({bad} unparseable), {} after filters",
        lines.len()
    );
    if lines.is_empty() {
        println!(
            "nothing to summarise. Is bridge_move_trace on, and the game started with -devsdk?"
        );
        return ExitCode::SUCCESS;
    }
    let (c, s, u) = lines.iter().fold((0, 0, 0), |(c, s, u), l| match l.side {
        TraceSide::Client => (c + 1, s, u),
        TraceSide::Server => (c, s + 1, u),
        TraceSide::Unknown => (c, s, u + 1),
    });
    println!("sides       client {c}, server {s}, unknown {u}");
    let mut ents: Vec<i64> = lines.iter().map(|l| l.ent).collect();
    ents.sort_unstable();
    ents.dedup();
    println!("entities    {ents:?}");
    println!(
        "cmds        {} .. {}",
        lines.first().map_or(0, |l| l.cmd),
        lines.last().map_or(0, |l| l.cmd)
    );
    let peak = lines
        .iter()
        .map(|l| l.post_speed_h())
        .fold(0.0_f32, f32::max);
    let grounded = lines.iter().filter(|l| l.ground != 0);
    let peak_ground = grounded.map(|l| l.post_speed_h()).fold(0.0_f32, f32::max);
    let peak_vz = lines
        .iter()
        .map(|l| l.post_velocity[2])
        .fold(f32::MIN, f32::max);
    println!("peak |h|    {peak:.3} u/s (on ground {peak_ground:.3})");
    println!("peak v_z    {peak_vz:.3} u/s");
    let mut max_speeds: Vec<i64> = lines.iter().map(|l| l.max_speed.round() as i64).collect();
    max_speeds.sort_unstable();
    max_speeds.dedup();
    println!("max speeds  {max_speeds:?} (the game's ms= field)");

    if let Some(out) = out {
        if let Some(dir) = out.parent().filter(|d| !d.as_os_str().is_empty()) {
            let _ = std::fs::create_dir_all(dir);
        }
        let res =
            std::fs::File::create(&out).and_then(|f| write_csv(&lines, std::io::BufWriter::new(f)));
        if let Err(e) = res {
            eprintln!("cannot write {}: {e}", out.display());
            return ExitCode::FAILURE;
        }
        println!("csv         {}", out.display());
    }
    ExitCode::SUCCESS
}
