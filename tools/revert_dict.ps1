param([string]$File)

$lines = Get-Content $File
# Remove first line (which is # we added)
# Remove last line (which is space we added)
$lines = $lines[1..($lines.Count - 2)]
$lines | Set-Content $File
Write-Host "Reverted: $File"
Write-Host "First 3 lines:"
Get-Content $File | Select-Object -First 3
Write-Host "Last 3:"
Get-Content $File | Select-Object -Last 3