# Development environment (Windows)

Audited and set up 2026-10-01 on Windows 11 Pro 10.0.26200.

| tool | status | version / location |
|---|---|---|
| Git | already installed | 2.55.0.windows.5 |
| Python | already installed | 3.11.9 |
| VS Code | already installed | 1.135.0 (`F:\Microsoft VS Code`) |
| GitHub CLI | already installed (not logged in; not needed) | 2.98.0 |
| winget | already installed | – |
| Vulkan runtime | already installed | `vulkan-1.dll`. GPUs: NVIDIA RTX 3080 Ti (driver 32.0.16.1088) + AMD Radeon iGPU |
| **rustup** | **installed** (winget `Rustlang.Rustup`) | 1.29.1, `%USERPROFILE%\.cargo\bin` |
| **Rust** | **installed** | stable-x86_64-pc-windows-msvc, rustc/cargo 1.99.0. The repo pins 1.99.0 in `rust-toolchain.toml`. |
| **VS 2022 Build Tools** | **installed** (winget, VCTools workload) | `C:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools` (MSVC x64 toolset) |
| **Windows SDK** | **installed** with the Build Tools | 10.0.26100.0 |
| **CMake** | **installed** (winget) | 4.4.3. Not needed yet; useful for R5 C++ tools. |
| Ninja | not installed | not needed |

## Game installs (read-only inputs)

| | |
|---|---|
| Apex Legends (Steam) | `E:\SteamLibrary\steamapps\common\Apex Legends`, build `25522636`, `R5pc_r5-301_J44_CL11595978_FSv30_1_2026_09_23_12_37`, gameversion v3.0.2.44 |
| R5Reloaded / R5Flowstate | not found |

Paths are recorded in the git-ignored `tools/local-env.ps1`. Re-detect with
`tools/detect-apex.ps1`. **ApexRuntime never writes to game folders.**

## Reference repos

Cloned to `C:\Dev\GameReferences` (outside the repo). `research/references`
is a junction to it and is git-ignored.
See [`LICENSE-REFERENCES.md`](LICENSE-REFERENCES.md).

## Pinned Bevy source

`C:\Dev\GameReferences\bevy-0.19.1` is a checkout of tag `v0.19.1`
(`b56fc29`), the exact version in `Cargo.lock`. Look Bevy APIs up there
instead of guessing (they change every release). Re-clone at the new tag
whenever Bevy is bumped.

## Everyday commands

```powershell
cd C:\Dev\ApexRuntime
cargo test --workspace                       # all tests (core tests need no GPU)
cargo run -p apex-sandbox --release          # first-person sandbox window
cargo run -p apex-telemetry --bin apex-replay -- scripts/inputs/bhop.json --out traces/bhop.csv
cargo run -p apex-tables --bin preflight      # sheet completeness + constants/config agreement
cargo run -p apex-tables --bin trace-diff -- ref.csv port.csv   # golden-trace overlap
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all
```

A terminal opened **before** the installs finished will not see `cargo` on
PATH. Open a new terminal (or restart VS Code).

## Recommended VS Code extensions

`rust-lang.rust-analyzer` (essential) and `tamasfe.even-better-toml`. Both
are listed in `.vscode/extensions.json`, so VS Code offers them.
