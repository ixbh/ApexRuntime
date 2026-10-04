# Asset pipeline (personal use only)

Apex content is exported by the user from the **offline Flowstate install**
(`F:\R5Flowstate\paks\Win64`) into `F:\ApexContent` and loaded at runtime.
Nothing exported is ever committed (both repos ignore `ApexContent/`, `*.cast`,
`*.rpak`, ...). The Steam install is not used; if something is only there,
copy that one file out with the game closed and export from the copy.

## Tool: RSX (reSource Xtractor)

- <https://github.com/r-ex/rsx>, AGPL-3.0, actively maintained (2.3.0,
  2026-09-15: Season 30.1 models/animations). Used as a standalone tool
  only; nothing of it is linked or copied into our code.
- Reads RPak containers: models (`mdl_`), animation rigs (`arig`) and
  sequences (`aseq`), materials/textures, UI images, weapon definitions
  (`wepn`), datatables, settings.
- Model/animation export format we use: **Cast** (open spec,
  <https://github.com/dtzxporter/cast>). IW4L gets a Cast loader, so no
  Blender step.
- Command line (docs/CLI.md in the RSX repo): `-nogui`, `-export`,
  `--exportdir`, `--exporttypes`, `-exportdependencies`,
  `-exportrigsequences`, `-matltextures`, `--list <file> --listformat csv`.
- Do **not** use RSX's "Game Finder" / "Skin Finder": they look for the
  Steam install. Always open files from `F:\R5Flowstate\paks\Win64`.

## What works (2026-10-04, RSX 2.3.0 at `F:\Tools\rsx_2.3.0`; Claude runs it)

The user asked Claude to run RSX. Use `rsx_nogui.exe` from its own folder.
Gotchas found the hard way:

- Pass `--loadwhitelist <types>` or nothing is parsed: names stay hashes
  and exports silently write nothing.
- `--list` lists whatever the export filter left (an empty filter empties
  it), so list without `-export`:
  `rsx_nogui -nogui --loadwhitelist mdl_,arig,aseq,wepn,matl --list out.csv --listformat csv <pak>`
- Exports default to Cast for `mdl_`/`arig`/`aseq`. Name filter:
  `--exportfilter <substring>` (ImGui filter, commas = OR).
- **Textures do not export from the CLI** (any `txtr`, or a model with its
  materials loaded, exits 1). The texture step is the GUI (`rsx.exe`).
- `pc_all.opt.starpak` (Flowstate's optional high-res textures) is not
  installed: lower-res textures only.

Exported so far (`F:\ApexContent`, never committed):

- `weapons\r99\mdl\Weapons\r97\ptpov_r97_LOD0.cast`: R-99 viewmodel (the
  R-99 is `r97` internally), 114 bones, 23 meshes, 13 PBR materials whose
  textures are referenced as `ptpov_r97/<name>.png`.
- `weapons\r99\animrig\...\ptpov_r97.cast` + `animseq\...\ptpov_r97\*.cast`:
  rig and 163 viewmodel sequences (30 fps), e.g. `reload_seq`,
  `sprintslide`, `inspect_basic`, `fire_02`, `ads_in`, `draw`.
- `characters\wattson\mdl\Weapons\arms\pov_pilot_light_wattson_LOD0.cast`
  (84 bones, all in the R-99 skeleton: the gun's animations drive them) and
  `mdl\Humans\class\light\pilot_light_wattson_LOD0-3.cast` (body).
  Source pak: `root_lgnd_skins_wattson_classic.rpak`.

`tools/cast_dump.py FILE.cast [--values]` prints a Cast file's node tree.

## Setup (user, once)

1. Download `rsx_2.3.0.zip` from
   <https://github.com/r-ex/rsx/releases/tag/2.3.0>.
2. Extract it to `F:\Tools\rsx\` (outside both repos).

## Step 1: asset list (find the R-99 and the arms)

Run (PowerShell) — RSX only reads the pak, writes a CSV:

```
& "F:\Tools\rsx\rsx.exe" -nogui --list "F:\ApexContent\lists\common.csv" --listformat csv "F:\R5Flowstate\paks\Win64\common.rpak"
```

Columns: `type,guid,file_name,asset_name`. Claude reads the CSV and picks
the exact asset names (first-person R-99 model, arms, its animation rig
and sequences) for step 2.

## Step 2: export the R-99 (names filled in from step 1)

In the RSX GUI: File > Open `common.rpak`, search the asset names from
step 1, set the model/animation export format to **Cast** (Settings), and
export to `F:\ApexContent\weapons\r99\` with dependencies (materials,
textures, rig sequences).

## Step 3 (Claude): Cast loader in IW4L

Cast model + skeleton + animation -> Bevy mesh/skin/animation clips,
replacing the MW2 viewmodel; the Apex feel layer drives its placement.
