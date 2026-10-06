# One-shot gun sound recording: start offline Flowstate, play every gun
# while recording the PC's audio output, close Flowstate, cut the result
# into per-gun files for IW4L. Pause all other audio first (music, Discord,
# other games): the recording takes everything the PC plays. ~20 minutes.
#
#   .\research\weapons\record_sounds.ps1 [-Guns "r97,wingman"]
param([string]$Guns = "")
$ErrorActionPreference = "Stop"
$here = $PSScriptRoot
& "$here\launch_flowstate.ps1"
$extra = @()
if ($Guns) { $extra = @("--guns", $Guns) }
python -u "$here\sound_capture.py" @extra
$code = $LASTEXITCODE
Get-Process r5apex* -ErrorAction SilentlyContinue | Stop-Process -Force -Confirm:$false
if ($code -ne 0) { exit $code }
python "$here\sound_slice.py"
