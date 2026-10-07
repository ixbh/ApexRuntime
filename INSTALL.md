# Install: Apex movement and guns in MW2 (IW4L)

This project is two repositories that sit **next to each other**:

| repo | what it is |
|---|---|
| **ApexRuntime** (this one) | Apex Legends movement and weapon behaviour, re-implemented from scratch in Rust from measurements. No game code or assets. |
| **iw4l-apex** | A fork of [IW4L](https://github.com/vladtrc/iw4L) (open-source MW2 multiplayer runtime) with ApexRuntime plugged in. Branch `apex-movement`. |

Nothing from Apex or MW2 is in either repo. You bring your own copy of MW2
and, for the Apex look/sound, your own export from an offline
[R5Flowstate](https://r5reloaded.com) install.

What you get:

- **Level 1, movement only:** MW2 maps with Apex movement (sprint, slide,
  lurch, tap-strafe, mantle, superglide, wall climb, bhop...). Needs only MW2.
- **Level 2, the full Apex kit:** Apex gun models, arms and animations in
  first person, Apex guns on the floor and in hands, Apex recoil, gun sounds,
  reload rules, shared ammo, the Apex HUD and inventory. Needs a personal
  export from R5Flowstate (below). Advanced; expect some fiddling.

Windows 10/11 x64 only so far.

---

## 1. Tools (once)

Install with `winget` in PowerShell, or from the vendors' sites:

```powershell
winget install Git.Git
winget install Rustlang.Rustup
winget install Microsoft.VisualStudio.2022.BuildTools --override "--wait --passive --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended"
winget install Python.Python.3.11
```

Open a **new** terminal afterwards so `cargo` is on PATH. The Rust version is
pinned in `rust-toolchain.toml`; rustup fetches it on the first build.

A GPU with Vulkan or DX12 drivers is needed (any recent NVIDIA/AMD/Intel).

## 2. Get the code (side by side)

Fork both repos on GitHub (or just clone them). They must be in the **same
parent folder** with these folder names, because iw4l-apex builds
ApexRuntime from `../ApexRuntime`:

```powershell
mkdir C:\Dev; cd C:\Dev
git clone https://github.com/<you>/ApexRuntime.git
git clone -b apex-movement https://github.com/<you>/iw4l-apex.git
```

```
C:\Dev\
  ApexRuntime\
  iw4l-apex\
```

## 3. MW2 (Level 1)

You need **Call of Duty: Modern Warfare 2 (2009) multiplayer** installed from
Steam. IW4L only reads its files.

In `iw4l-apex`, copy `.env.example` to `.env` and set `IW4L_GAMES` to the MW2
folder (the deploy/master lines can be left alone for local play):

```
IW4L_GAMES=D:\SteamLibrary\steamapps\common\Call of Duty Modern Warfare 2
```

## 4. Build and play

```powershell
cd C:\Dev\iw4l-apex
.\run-apex.ps1 -Build            # first build takes a while (10-20 min)
.\run-apex.ps1                   # later starts: mp_rust, Apex movement
.\run-apex.ps1 -Map mp_terminal -Bots 6
.\run-apex.ps1 -Classic          # same build, original MW2 movement
```

Keys beyond MW2's: **V** mantle boost, **B** Apex loadout menu (Level 2),
**Tab** Apex inventory (Level 2), **E** pick up.

To try the movement alone without MW2, the sandbox in this repo needs
nothing else:

```powershell
cd C:\Dev\ApexRuntime
cargo run -p apex-sandbox --release -- --hz 165 --fov 110
```

---

## 5. The Apex kit (Level 2, optional)

Everything here lands in **your own content folder** outside both repos
(e.g. `F:\ApexContent`). It is personal: never commit it, never share it.
Both repos' `.gitignore` files block the usual formats.

Tell both repos where it is, in `iw4l-apex\.env` **and** as a user
environment variable (the Python tools read it too):

```
IW4L_APEX_CONTENT=F:\ApexContent
```

Other variables the tools read (defaults in brackets):

| variable | meaning |
|---|---|
| `R5_FLOWSTATE` | your offline Flowstate install [`F:\R5Flowstate`] |
| `APEX_RSX` | `rsx_nogui.exe` from RSX [`F:\Tools\rsx_2.3.0\rsx_nogui.exe`] |
| `APEX_PAKS` | Flowstate's `paks\Win64` [`F:\R5Flowstate\paks\Win64`] |
| `BSPCONV_DIR` | a clone of [R5Flowstate/bspconv](https://github.com/R5Flowstate/bspconv), used only for its asset-name hash [`F:\Dev\GameReferences\bspconv`] |

### 5a. R5Flowstate and RSX

1. Install R5Flowstate with its launcher, **including the HD texture pack**
   (RSX crashes on textures without it).
2. Download [RSX](https://github.com/r-ex/rsx/releases) (2.3.0 tested) and
   unzip it somewhere outside the repos.
3. **Never** pass a game file to `rsx_nogui --list`: that argument is the
   *output* file and it gets overwritten. If it happens, the Flowstate
   launcher's verify repairs it.

The retail Apex install is never used by anything here (it is anti-cheat
protected). Everything comes from offline Flowstate.

### 5b. Export guns, arms and UI

```powershell
cd C:\Dev\ApexRuntime
pip install numpy pyaudiowpatch
```

1. Weapon definitions: RSX `wepn` export of `mp_weapon_*` (and
   `viewkick_patterns`) into `<content>\game` (see
   [`docs/ASSET-PIPELINE.md`](docs/ASSET-PIPELINE.md) for the exact flags).
2. Gun models, textures and animations, plus `guns.json`:
   `python tools/export_guns.py`
3. First-person arms (Wattson): from `root_lgnd_skins_wattson_classic.rpak`,
   export `pov_pilot_light_wattson` as Cast to
   `<content>\characters\wattson\` (see ASSET-PIPELINE.md).
4. UI images: RSX export of `uiia` into `<content>\ui\uiia`, then
   `python tools/name_ui_images.py` names them for the HUD.

### 5c. Measure Flowstate (recoil, rules, sounds)

These scripts drive an **offline** Flowstate through its developer console
link (`-devsdk`; no mouse/keyboard automation) and write measurements into
`<content>\captures`.

1. Install the logging mod: copy `research\weapons\flowstate_mod` to
   `<Flowstate>\mods\ApexRuntime.WeaponLog` and add it to
   `<Flowstate>\mods\mods.vdf`.
2. Close the Flowstate launcher (it fights the scripts over the game), then:

```powershell
powershell -File research\weapons\launch_flowstate.ps1 -Width 1920 -Height 1080
python research\weapons\capture.py            # fire every gun, log it (long)
python research\weapons\recoil.py             # -> captures\recoil.json
python research\weapons\behaviour_capture.py  # reload/swap/sprint/inspect rules
python research\weapons\behaviour_summary.py --rules   # -> captures\weapon_rules.json
```

3. Gun sounds: pause **all** other audio on the PC (the recording takes
   everything that plays), then
   `.\research\weapons\record_sounds.ps1` (~20 min; starts and closes
   Flowstate itself, cuts per-gun files into `captures\sound\guns`).

Then `.\run-apex.ps1` in iw4l-apex picks everything up. Press **B** for the
loadout menu.

---

## Troubleshooting

- **`cargo` not found:** open a new terminal after installing rustup.
- **Link errors / `link.exe` missing:** the VS Build Tools C++ workload is
  missing.
- **"Set IW4L_GAMES in .env":** step 3.
- **Apex guns missing, MW2 guns shown:** `IW4L_APEX_CONTENT` unset or
  `guns.json` not exported (5b).
- **Flowstate stuck on a black loading screen during captures:** close the
  Flowstate launcher and any running `r5apex*` processes, start again.
- Toggles: `IW4L_APEX_HUD=0` (MW2 HUD), `IW4L_APEX_SFX_VOLUME=0.5` (gun
  volume), `IW4L_APEX_MOVEMENT=0` (MW2 movement).

## Ground rules for forks

- Never commit or share exported Apex content, captures, RPaks, or game
  files. The code is MIT/Apache; the content is not ours.
- No online use: no anti-cheat bypass, nothing for live Apex or ranked play.
  Flowstate measurements are offline only.
- Reference repos are read, not copied (see
  [`docs/LICENSE-REFERENCES.md`](docs/LICENSE-REFERENCES.md)).

Apex Legends is a trademark of Electronic Arts / Respawn Entertainment;
Call of Duty is a trademark of Activision. This project is not affiliated
with or endorsed by them.
