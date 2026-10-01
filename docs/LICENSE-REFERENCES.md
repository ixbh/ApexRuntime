# Reference repositories — licences and how we may use them

Audited 2026-10-01 against the commits listed. Re-audit before reusing anything
from a newer commit. Clones live in `C:\Dev\GameReferences` (linked as the
git-ignored `research/references/`) and are **never vendored** into this repo.

| repository | commit | licence | direct code reuse? | how we use it |
|---|---|---|---|---|
| [vladtrc/iw4L](https://github.com/vladtrc/iw4L) | `bfe3c2a` (2026-10-01) | Apache-2.0 (+ OFL / GPL-with-font-exception fonts) | **Yes, with attribution.** Keep the copyright notice, state changes, carry `NOTICE` text for anything we copy. Do not copy the bundled fonts without their licence files. | Architecture reference (fixed-step `sim::step` funnel, `CollisionBackend`, crate layering, `.env` + owned-install model). Future integration target. Nothing copied so far. |
| [chasmlol/2010-rust-rewrite-mashup](https://github.com/chasmlol/2010-rust-rewrite-mashup) | `ab43b8a` (2026-10-01) | Apache-2.0 (fork of IW4L) | Same as IW4L. `skate/` comes from SK8-ENGINE/skate-3-rust-engine; check that upstream separately before reusing it. `third_party/minecraftoss` has its own licence. | Example of composing a foreign movement controller (Skate 3) into IW4L through an adapter. Nothing copied. |
| [Mauler125/r5sdk](https://github.com/Mauler125/r5sdk) | `c49e0d2` (2025-06-22) | Valve **Source 1 SDK License** (`license/LICENSE`) + third-party notices | **No.** The Source SDK licence limits use to Valve-engine products and forbids redistribution outside its terms. It is also a detour layer over Respawn's binary, and Respawn/EA own the names and structures. | Technical reference only: class and function names (`CPlayerMove::RunCommand`, `CGameMovement`, `CMoveData`, `CUserCmd`), player field names, engine architecture. Season 3. |
| [R5Flowstate/s21-r5sdk](https://github.com/R5Flowstate/s21-r5sdk) | `71d365a` (2026-09-26) | Valve Source 1 SDK License for the Valve-derived tree. `NOTICE` says Respawn/EA material is "not ours to license". No separate grant for R5F additions in the root. | **No.** | Technical reference only: S21 vs S3 movement differences documented by its bridge detours (tap-strafe, mantle boost, wall climb, slide super-jump, …). |
| [R5Flowstate/s21-platform](https://github.com/R5Flowstate/s21-platform) | `db0d226` (2026-09-26) | AGPL-3.0 for R5F additions. Respawn/EA script structure, names, localisation and datatable schemas are excluded (`NOTICE`). | **No.** AGPL would infect the runtime, and most content is Respawn's. | Reference for Squirrel-side movement hooks, playlists and cfg (`cfg/speed.cfg`). Never copy script text. |
| [R5Flowstate/bspconv](https://github.com/R5Flowstate/bspconv) | `b8cc35b` (2026-09-27) | AGPL-3.0 | **No** (AGPL). It may be run as a separate tool on the user's own files. | Future: understanding rBSP layout for map/collision import. |
| R5Flowstate/rsx | not cloned | AGPL-3.0 | No; run as an external tool only. | Future: asset inspection. |
| R5Flowstate/rmdlconv, R5-AnimConv | not cloned | GPL-3.0 | No; run as external tools only. | Future: models / animations (not needed for movement). |
| R5Flowstate/RePak, launcher | not cloned | MIT | Yes, with attribution. | Probably not needed. RePak builds paks; we only read. |

## Rules that follow from this

1. **Movement logic is written independently**, from public behaviour,
   measurements and general Source/Quake knowledge (e.g. the published Quake
   III source is GPL, so we do not copy it either; we implement the
   well-known algorithms ourselves).
2. Constants learned from R5 material are **facts** (numbers), not code.
   Record them in `MOVEMENT-MEASUREMENTS.md` with their source. Never paste
   code blocks.
3. Apache-2.0 code from IW4L may be adapted later, but only with a header
   comment naming the source file and commit, plus an entry in our own `NOTICE`.
4. Nothing proprietary (Apex assets, binaries, rpaks, dumps, script text) is
   committed. See `.gitignore`.
5. Our own licence is `MIT OR Apache-2.0`. That is compatible with
   Apache-2.0 inflows and lets future adapters embed the core anywhere.
