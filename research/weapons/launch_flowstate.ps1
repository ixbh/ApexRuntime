# Start offline Flowstate (dedicated server + windowed client) on the Firing
# Range with the dev agent link, for research/weapons/capture.py.
param([string]$Map = "mp_rr_canyonlands_staging_mu1", [int]$Width = 1600, [int]$Height = 900)
$root = 'F:\R5Flowstate'
Get-Process r5apex* -ErrorAction SilentlyContinue | Stop-Process -Force -Confirm:$false
Start-Sleep 3
$ds = "-dedicated -port 37015 +sv_allowSendTableTransmitToClients 1 +stringtable_compress 1 -devsdk -dev +net_usesocketsforloopback 1 +script_show_output 2 +script_show_warning 2 -offline +sv_onlineAuthEnable 0 +spire_host_visibility 0 +sv_cheats 1 +launchplaylist survival_dev +map $Map +sv_modPolicy 0"
Start-Process -FilePath "$root\r5apex_ds.exe" -ArgumentList $ds -WorkingDirectory $root
Start-Sleep 15
$cl = "-nodiscord -allowmultiple -devsdk -dev +net_usesocketsforloopback 1 +script_show_output 2 +script_show_warning 2 -offline +cl_onlineAuthEnable 0 +timeout_during_load 600 -language english +fps_max 165 +miles_language english -windowed -noborder -width $Width -height $Height +connect 127.0.0.1:37015"
Start-Process -FilePath "$root\r5apex_dx12.exe" -ArgumentList $cl -WorkingDirectory $root
"Flowstate starting ($Map)"
