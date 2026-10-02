//! Parser for R5Flowstate's `[MOVE-TRACE]` log lines.
//!
//! Enabled in-game with `bridge_move_trace 1` (requires launching with
//! `-devsdk`). One line per usercmd, sampled around `CGameMovement::FullWalkMove`:
//!
//! ```text
//! [MOVE-TRACE] cmd=%d ent=%d ms=%.3f spr=%d duck=%d slide=%d ge=%d fwd=%.3f sd=%.3f
//!   b=%08X msc=%.4f o=x y z v=x y z po=x y z pv=x y z[ rp=1]
//! ```
//!
//! `o`/`v` are origin/velocity *before* the move, `po`/`pv` *after*. `ms` is
//! max speed, `msc` the move scale, `ge` the ground-entity flag, `rp=1` marks
//! a client re-prediction. The format is defined in s21-r5sdk
//! `src/game/{server,client}/move_sim_trace.cpp` (format facts only, no code).
//!
//! Not logged (limits what these traces can validate): view angles and the
//! per-command frame time.

use serde::{Deserialize, Serialize};

pub const MARKER: &str = "[MOVE-TRACE]";

/// Which half of the bridge emitted the line, when the log prefix says so.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum TraceSide {
    Client,
    Server,
    Unknown,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MoveTraceLine {
    pub side: TraceSide,
    pub cmd: i64,
    pub ent: i64,
    pub max_speed: f32,
    pub sprint: i32,
    pub duck: i32,
    pub slide: i32,
    pub ground: i32,
    pub forward: f32,
    pub side_move: f32,
    pub buttons: u32,
    pub move_scale: f32,
    pub origin: [f32; 3],
    pub velocity: [f32; 3],
    pub post_origin: [f32; 3],
    pub post_velocity: [f32; 3],
    pub replay: bool,
}

impl MoveTraceLine {
    pub fn post_speed_h(&self) -> f32 {
        let [x, y, _] = self.post_velocity;
        (x * x + y * y).sqrt()
    }
}

#[derive(Debug, PartialEq)]
pub enum ParseError {
    NoMarker,
    Missing(&'static str),
    Bad(&'static str, String),
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoMarker => write!(f, "not a [MOVE-TRACE] line"),
            Self::Missing(k) => write!(f, "missing field {k}"),
            Self::Bad(k, v) => write!(f, "bad value for {k}: {v:?}"),
        }
    }
}

impl std::error::Error for ParseError {}

/// Parse one log line. Leading timestamps / channel tags are tolerated.
pub fn parse_line(line: &str) -> Result<MoveTraceLine, ParseError> {
    let at = line.find(MARKER).ok_or(ParseError::NoMarker)?;
    let prefix = line[..at].to_ascii_uppercase();
    let side = if prefix.contains("CLIENT") || prefix.contains("[CL") {
        TraceSide::Client
    } else if prefix.contains("SERVER") || prefix.contains("[SV") {
        TraceSide::Server
    } else {
        TraceSide::Unknown
    };

    let tokens: Vec<&str> = line[at + MARKER.len()..].split_whitespace().collect();
    // Find "key=first" and return the first value plus following tokens.
    let field = |key: &'static str| -> Result<(usize, &str), ParseError> {
        let pat = format!("{key}=");
        tokens
            .iter()
            .position(|t| t.starts_with(&pat))
            .map(|i| (i, &tokens[i][pat.len()..]))
            .ok_or(ParseError::Missing(key))
    };
    let num = |key: &'static str, s: &str| -> Result<f32, ParseError> {
        s.parse::<f32>()
            .map_err(|_| ParseError::Bad(key, s.to_owned()))
    };
    let int = |key: &'static str| -> Result<i64, ParseError> {
        let (_, v) = field(key)?;
        v.parse::<i64>()
            .map_err(|_| ParseError::Bad(key, v.to_owned()))
    };
    let float = |key: &'static str| -> Result<f32, ParseError> {
        let (_, v) = field(key)?;
        num(key, v)
    };
    let vec3 = |key: &'static str| -> Result<[f32; 3], ParseError> {
        let (i, first) = field(key)?;
        let y = tokens.get(i + 1).ok_or(ParseError::Missing(key))?;
        let z = tokens.get(i + 2).ok_or(ParseError::Missing(key))?;
        Ok([num(key, first)?, num(key, y)?, num(key, z)?])
    };

    let (_, b) = field("b")?;
    let buttons = u32::from_str_radix(b, 16).map_err(|_| ParseError::Bad("b", b.to_owned()))?;
    let replay = field("rp").map(|(_, v)| v == "1").unwrap_or(false);

    Ok(MoveTraceLine {
        side,
        cmd: int("cmd")?,
        ent: int("ent")?,
        max_speed: float("ms")?,
        sprint: int("spr")? as i32,
        duck: int("duck")? as i32,
        slide: int("slide")? as i32,
        ground: int("ge")? as i32,
        forward: float("fwd")?,
        side_move: float("sd")?,
        buttons,
        move_scale: float("msc")?,
        origin: vec3("o")?,
        velocity: vec3("v")?,
        post_origin: vec3("po")?,
        post_velocity: vec3("pv")?,
        replay,
    })
}

/// Parse every `[MOVE-TRACE]` line in a log, skipping everything else.
/// Returns the parsed lines and the number of marker lines that failed to parse.
pub fn parse_log(text: &str) -> (Vec<MoveTraceLine>, usize) {
    let mut out = Vec::new();
    let mut bad = 0;
    for line in text.lines().filter(|l| l.contains(MARKER)) {
        match parse_line(line) {
            Ok(l) => out.push(l),
            Err(_) => bad += 1,
        }
    }
    (out, bad)
}

pub const CSV_HEADER: &str = "side,cmd,ent,max_speed,sprint,duck,slide,ground,fwd,side_move,buttons,move_scale,o_x,o_y,o_z,v_x,v_y,v_z,po_x,po_y,po_z,pv_x,pv_y,pv_z,speed_h,replay";

pub fn write_csv(lines: &[MoveTraceLine], mut out: impl std::io::Write) -> std::io::Result<()> {
    writeln!(out, "{CSV_HEADER}")?;
    for l in lines {
        let side = match l.side {
            TraceSide::Client => "client",
            TraceSide::Server => "server",
            TraceSide::Unknown => "unknown",
        };
        writeln!(
            out,
            "{side},{},{},{},{},{},{},{},{},{},{:08X},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{}",
            l.cmd,
            l.ent,
            l.max_speed,
            l.sprint,
            l.duck,
            l.slide,
            l.ground,
            l.forward,
            l.side_move,
            l.buttons,
            l.move_scale,
            l.origin[0],
            l.origin[1],
            l.origin[2],
            l.velocity[0],
            l.velocity[1],
            l.velocity[2],
            l.post_origin[0],
            l.post_origin[1],
            l.post_origin[2],
            l.post_velocity[0],
            l.post_velocity[1],
            l.post_velocity[2],
            l.post_speed_h(),
            l.replay as u8
        )?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    // Built from the exact printf format in move_sim_trace.cpp.
    const SERVER: &str = "[2026-10-02 10:00:00.123] [SERVER] [MOVE-TRACE] cmd=1234 ent=1 ms=300.000 spr=1 duck=0 slide=0 ge=1 fwd=1.000 sd=0.000 b=00020004 msc=1.0000 o=10.000 -20.500 0.031 v=250.000 0.000 0.000 po=14.167 -20.500 0.031 pv=255.000 0.000 0.000";
    const CLIENT_RP: &str = "[MOVE-TRACE] cmd=99 ent=1 ms=300.000 spr=0 duck=1 slide=1 ge=0 fwd=0.000 sd=-1.000 b=0000000A msc=0.5000 o=1.000 2.000 3.000 v=4.000 5.000 6.000 po=7.000 8.000 9.000 pv=-1.000 -2.000 -3.000 rp=1";

    #[test]
    fn parses_server_line() {
        let l = parse_line(SERVER).unwrap();
        assert_eq!(l.side, TraceSide::Server);
        assert_eq!(l.cmd, 1234);
        assert_eq!(l.buttons, 0x0002_0004);
        assert_eq!(l.origin, [10.0, -20.5, 0.031]);
        assert_eq!(l.post_velocity, [255.0, 0.0, 0.0]);
        assert!(!l.replay);
        assert!((l.post_speed_h() - 255.0).abs() < 1e-3);
    }

    #[test]
    fn parses_replay_flag_and_negatives() {
        let l = parse_line(CLIENT_RP).unwrap();
        assert!(l.replay);
        assert_eq!(l.side_move, -1.0);
        assert_eq!(l.post_velocity, [-1.0, -2.0, -3.0]);
        assert_eq!(l.slide, 1);
    }

    #[test]
    fn log_skips_noise_and_counts_bad_lines() {
        let log = format!("hello\n{SERVER}\n[MOVE-TRACE] cmd=oops\n{CLIENT_RP}\n");
        let (lines, bad) = parse_log(&log);
        assert_eq!(lines.len(), 2);
        assert_eq!(bad, 1);
    }
}
