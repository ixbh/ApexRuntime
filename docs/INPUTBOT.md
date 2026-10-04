# apex-inputbot: automated captures (offline R5Flowstate only)

Plays scripted key presses, wheel notches and exact mouse turns into the
**offline** Flowstate game so sweeps are repeatable and view angles are known
(the MOVE-TRACE log has no view angles).

## Safety (hard rules, enforced in code)
- Sends input only while the foreground window is an `r5apex*` exe inside the
  Flowstate folder (`--flowstate-root`, default `F:\R5Flowstate`).
- Refuses any path that looks like a retail Apex install (Steam, EA, Origin,
  "Apex Legends"). Never use input automation in live Apex.
- Stops on **F12** or when the game loses focus; releases every held key.

## Use
1. Launch Flowstate (same launch args as session 6), load Firing Range, stand
   in a long flat lane facing along it. Auto-sprint on, sensitivity 1.63.
2. `python research/bot/make_scripts.py --crouch c --sens 1.63`
3. `cargo run --release -p apex-inputbot -- research/bot/scripts/deadslide.json`
   then click into the game within 5 s and let go of the mouse/keyboard.
4. Copy the logs as usual. The bot writes `traces/raw/bot-<name>-<time>.json`
   with each attempt's start time (wall clock, to match message.log).

Scripts: `deadslide` (jump 0-40 frames after the slide starts, 2 runs each),
`lurch_angles` (W notch after flicking the view 15-120° right at 150/300/400
ms). Each attempt ends with a 180° turn to run back along the lane.
Timing: Windows can't hit exact frames (~1 ms jitter vs 6 ms frames); the log
shows which frame each press landed on, which is what the analysis uses.
