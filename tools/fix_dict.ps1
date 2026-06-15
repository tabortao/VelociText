param([string]$File)

$content = Get-Content $File -Raw
$newContent = "#`n" + $content.TrimEnd() + "`n "
Set-Content $File -Value $newContent -NoNewline
Write-Host "Fixed: $File"
Write-Host "First 3 lines:"
Get-Content $File | Select-Object -First 3
Write-Host "Last 3 lines:"
Get-Content $File | Select-Object -Last 3