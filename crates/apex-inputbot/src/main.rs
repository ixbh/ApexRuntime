//! apex-inputbot — plays a scripted sequence of key presses, mouse turns and
//! wheel notches into the OFFLINE R5Flowstate game, for repeatable capture
//! sessions (sweeps like "slide N frames, then jump" and turns of a known
//! angle, which the MOVE-TRACE log cannot record).
//!
//! Safety (project rule: never automate the live game):
//! * it only sends input while the foreground window belongs to an exe named
//!   `r5apex*` inside `flowstate_root` (default `F:\R5Flowstate`);
//! * it refuses paths that look like a retail Apex install (Steam / EA / Origin
//!   folders, "Apex Legends");
//! * it stops on F12 or as soon as the game loses focus, and releases every
//!   key it holds.
//!
//! usage: apex-inputbot <script.json> [--flowstate-root DIR] [--dry-run]
//! Script format: see `Script` below and research/bot/*.py generators.

#[cfg(windows)]
mod win;

use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde::Deserialize;

#[derive(Deserialize)]
struct Script {
    name: String,
    /// Apex mouse sensitivity (degrees per count = 0.022 × sens).
    sens: f64,
    /// Seconds to switch to the game window before the first attempt.
    #[serde(default = "default_countdown")]
    countdown_s: f64,
    /// Pause between attempts.
    #[serde(default = "default_gap")]
    gap_ms: f64,
    attempts: Vec<Attempt>,
}

fn default_countdown() -> f64 {
    5.0
}
fn default_gap() -> f64 {
    800.0
}

#[derive(Deserialize)]
struct Attempt {
    label: String,
    steps: Vec<Step>,
}

/// One timed action, `at_ms` after the attempt starts.
#[derive(Deserialize)]
struct Step {
    at_ms: f64,
    #[serde(default)]
    down: Option<String>,
    #[serde(default)]
    up: Option<String>,
    /// Press and release after `tap_ms` (default 12 ms ≈ 2 frames at 165).
    #[serde(default)]
    tap: Option<String>,
    #[serde(default)]
    tap_ms: Option<f64>,
    /// Wheel notches (+ up, − down), one per ~1 ms.
    #[serde(default)]
    wheel: Option<i32>,
    /// Turn the view right by this many degrees (negative = left) …
    #[serde(default)]
    turn_right_deg: Option<f64>,
    /// … and down by this many (negative = up) …
    #[serde(default)]
    turn_down_deg: Option<f64>,
    /// … spread evenly over this long (0 = one flick).
    #[serde(default)]
    over_ms: f64,
}

#[derive(Debug, Clone, Copy)]
enum Action {
    Key(u16, bool),
    Wheel(i32),
    Mouse(i32, i32),
}

/// US-layout scancodes for the keys a capture needs.
fn scancode(name: &str) -> Option<u16> {
    Some(match name.to_ascii_lowercase().as_str() {
        "w" => 0x11,
        "a" => 0x1E,
        "s" => 0x1F,
        "d" => 0x20,
        "c" => 0x2E,
        "l" => 0x26,
        "e" => 0x12,
        "q" => 0x10,
        "space" => 0x39,
        "ctrl" | "lctrl" => 0x1D,
        "shift" | "lshift" => 0x2A,
        "f9" => 0x43,
        "f10" => 0x44,
        "f11" => 0x57,
        _ => return None,
    })
}

/// Expand an attempt into time-sorted actions (ms from the attempt start).
fn expand(a: &Attempt, deg_per_count: f64) -> Result<Vec<(f64, Action)>, String> {
    let mut out = Vec::new();
    let key = |n: &str| scancode(n).ok_or(format!("unknown key '{n}' in '{}'", a.label));
    for s in &a.steps {
        if let Some(k) = &s.down {
            out.push((s.at_ms, Action::Key(key(k)?, true)));
        }
        if let Some(k) = &s.up {
            out.push((s.at_ms, Action::Key(key(k)?, false)));
        }
        if let Some(k) = &s.tap {
            let c = key(k)?;
            out.push((s.at_ms, Action::Key(c, true)));
            out.push((s.at_ms + s.tap_ms.unwrap_or(12.0), Action::Key(c, false)));
        }
        if let Some(n) = s.wheel {
            for i in 0..n.unsigned_abs() {
                out.push((s.at_ms + f64::from(i), Action::Wheel(n.signum())));
            }
        }
        let (yaw, pitch) = (
            s.turn_right_deg.unwrap_or(0.0),
            s.turn_down_deg.unwrap_or(0.0),
        );
        if yaw != 0.0 || pitch != 0.0 {
            // Counts, spread over 1 ms slices with exact rounding so the
            // total turn is exact (sum of counts × deg_per_count).
            let (cx, cy) = (yaw / deg_per_count, pitch / deg_per_count);
            let slices = (s.over_ms.max(0.0).round() as usize).max(1);
            let (mut sent_x, mut sent_y) = (0i64, 0i64);
            for i in 1..=slices {
                let f = i as f64 / slices as f64;
                let (tx, ty) = ((cx * f).round() as i64, (cy * f).round() as i64);
                let (dx, dy) = (tx - sent_x, ty - sent_y);
                if dx != 0 || dy != 0 {
                    let at = s.at_ms + s.over_ms * (i - 1) as f64 / slices as f64;
                    out.push((at, Action::Mouse(dx as i32, dy as i32)));
                }
                (sent_x, sent_y) = (tx, ty);
            }
        }
    }
    out.sort_by(|a, b| a.0.total_cmp(&b.0));
    Ok(out)
}

/// The workspace root (two levels above this crate).
fn workspace_root() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
}

/// `p` as given if it exists, else relative to the workspace root, else
/// just the file name under research/bot/scripts.
fn resolve(p: &str) -> String {
    let given = std::path::Path::new(p);
    let candidates = [
        given.to_path_buf(),
        workspace_root().join(p),
        workspace_root()
            .join("research/bot/scripts")
            .join(given.file_name().unwrap_or_default()),
    ];
    candidates
        .iter()
        .find(|c| c.exists())
        .unwrap_or(&candidates[0])
        .to_string_lossy()
        .into_owned()
}

fn arg_value(args: &[String], name: &str) -> Option<String> {
    args.iter()
        .position(|a| a == name)
        .and_then(|i| args.get(i + 1).cloned())
}

/// The foreground exe must be r5apex* under `root`, and never a retail path.
fn is_offline_flowstate(path: &str, root: &str) -> Result<(), String> {
    let p = path.replace('/', "\\").to_ascii_lowercase();
    let r = root
        .replace('/', "\\")
        .trim_end_matches('\\')
        .to_ascii_lowercase();
    for bad in [
        "steamapps",
        "apex legends",
        "ea games",
        "electronic arts",
        "origin games",
    ] {
        if p.contains(bad) {
            return Err(format!(
                "refusing: '{path}' looks like a retail Apex install"
            ));
        }
    }
    let file = p.rsplit('\\').next().unwrap_or("");
    if !p.starts_with(&format!("{r}\\")) || !file.starts_with("r5apex") {
        return Err(format!(
            "refusing: foreground window is '{path}', not the offline Flowstate game under {root}"
        ));
    }
    Ok(())
}

fn now_unix_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0)
}

/// Sleep until `t`, then spin the last ~1.5 ms for accuracy.
fn wait_until(t: Instant) {
    let spin = Duration::from_micros(1500);
    let now = Instant::now();
    if t > now + spin {
        std::thread::sleep(t - now - spin);
    }
    while Instant::now() < t {
        std::hint::spin_loop();
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some(path) = args.first() else {
        eprintln!("usage: apex-inputbot <script.json> [--flowstate-root DIR] [--dry-run]");
        std::process::exit(2);
    };
    let root = arg_value(&args, "--flowstate-root").unwrap_or_else(|| r"F:\R5Flowstate".into());
    let dry = args.iter().any(|a| a == "--dry-run");
    // Paths may be given relative to the project root from any folder.
    let path = &resolve(path);
    let script: Script = match std::fs::read_to_string(path)
        .map_err(|e| e.to_string())
        .and_then(|t| serde_json::from_str(&t).map_err(|e| e.to_string()))
    {
        Ok(s) => s,
        Err(e) => {
            eprintln!("cannot read {path}: {e}");
            std::process::exit(2);
        }
    };
    let deg_per_count = 0.022 * script.sens;
    let mut plans = Vec::new();
    for a in &script.attempts {
        match expand(a, deg_per_count) {
            Ok(p) => plans.push(p),
            Err(e) => {
                eprintln!("{e}");
                std::process::exit(2);
            }
        }
    }
    let total_s: f64 = plans
        .iter()
        .map(|p| p.last().map_or(0.0, |x| x.0) + script.gap_ms)
        .sum::<f64>()
        / 1000.0;
    println!(
        "{}: {} attempts, ~{:.0} s. Switch to the game now; F12 stops.",
        script.name,
        plans.len(),
        total_s
    );
    if dry {
        for (a, p) in script.attempts.iter().zip(&plans) {
            println!("  {}: {} actions", a.label, p.len());
        }
        return;
    }
    run(&script, &plans, &root);
}

#[cfg(not(windows))]
fn run(_: &Script, _: &[Vec<(f64, Action)>], _: &str) {
    eprintln!("apex-inputbot only runs on Windows");
}

#[cfg(windows)]
fn run(script: &Script, plans: &[Vec<(f64, Action)>], root: &str) {
    std::thread::sleep(Duration::from_secs_f64(script.countdown_s));
    let game = win::foreground();
    let exe = win::exe_path(game).unwrap_or_default();
    if let Err(e) = is_offline_flowstate(&exe, root) {
        eprintln!("{e}");
        std::process::exit(1);
    }
    println!("target: {exe}");

    let mut held: Vec<u16> = Vec::new();
    let mut log = Vec::new();
    let mut stopped = None;
    'attempts: for (a, plan) in script.attempts.iter().zip(plans) {
        let start = Instant::now();
        let wall = now_unix_ms();
        println!("{wall}  {}", a.label);
        for &(at, act) in plan {
            wait_until(start + Duration::from_secs_f64(at / 1000.0));
            if win::f12_down() {
                stopped = Some("F12");
                break 'attempts;
            }
            if win::foreground() != game {
                stopped = Some("the game lost focus");
                break 'attempts;
            }
            match act {
                Action::Key(c, down) => {
                    win::key(c, down);
                    held.retain(|&h| h != c);
                    if down {
                        held.push(c);
                    }
                }
                Action::Wheel(n) => win::wheel(n),
                Action::Mouse(dx, dy) => win::mouse_move(dx, dy),
            }
        }
        for c in held.drain(..) {
            win::key(c, false);
        }
        log.push(serde_json::json!({ "label": a.label, "start_unix_ms": wall }));
        wait_until(Instant::now() + Duration::from_secs_f64(script.gap_ms / 1000.0));
    }
    for c in held.drain(..) {
        win::key(c, false);
    }
    if let Some(why) = stopped {
        println!("stopped: {why} (all keys released)");
    }
    // Attempt start times, to line attempts up with the game's message.log.
    let dir = workspace_root().join("traces/raw");
    let out = dir
        .join(format!(
            "bot-{}-{}.json",
            script.name.replace(' ', "_"),
            now_unix_ms()
        ))
        .to_string_lossy()
        .into_owned();
    let _ = std::fs::create_dir_all(&dir);
    match std::fs::write(&out, serde_json::to_vec_pretty(&log).unwrap_or_default()) {
        Ok(()) => println!("attempt log: {out}"),
        Err(e) => eprintln!("could not write {out}: {e}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_offline_flowstate_is_accepted() {
        let root = r"F:\R5Flowstate";
        assert!(is_offline_flowstate(r"F:\R5Flowstate\r5apex_dx12.exe", root).is_ok());
        assert!(is_offline_flowstate(r"f:/r5flowstate/R5Apex.exe", root).is_ok());
        assert!(
            is_offline_flowstate(
                r"E:\SteamLibrary\steamapps\common\Apex Legends\r5apex_dx12.exe",
                root
            )
            .is_err()
        );
        assert!(is_offline_flowstate(r"C:\Program Files\EA Games\Apex\r5apex.exe", root).is_err());
        assert!(is_offline_flowstate(r"F:\R5FlowstateX\r5apex.exe", root).is_err());
        assert!(is_offline_flowstate(r"F:\R5Flowstate\notepad.exe", root).is_err());
        // A retail-looking folder inside the root is refused too.
        assert!(is_offline_flowstate(r"F:\R5Flowstate\Apex Legends\r5apex.exe", root).is_err());
    }

    #[test]
    fn turns_are_exact_in_counts() {
        let a = Attempt {
            label: "t".into(),
            steps: vec![Step {
                at_ms: 0.0,
                down: None,
                up: None,
                tap: None,
                tap_ms: None,
                wheel: None,
                turn_right_deg: Some(90.0),
                turn_down_deg: None,
                over_ms: 300.0,
            }],
        };
        let dpc = 0.022 * 1.63;
        let plan = expand(&a, dpc).unwrap();
        let total: i32 = plan
            .iter()
            .map(|(_, act)| match act {
                Action::Mouse(dx, _) => *dx,
                _ => 0,
            })
            .sum();
        assert_eq!(total, (90.0 / dpc).round() as i32);
        assert!(plan.last().unwrap().0 < 300.0);
    }
}
