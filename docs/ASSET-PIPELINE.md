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
