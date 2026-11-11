$ErrorActionPreference = 'Stop'
$root = 'Z:\workspace'
$mirror = 'C:\Users\gfhfyjbr\Projects\Rust\gpuimm'
$link = Get-Item -LiteralPath $mirror -ErrorAction SilentlyContinue
if ($link -and $link.LinkType -eq 'Junction' -and $link.Target -contains $root) {
    [System.IO.Directory]::Delete($mirror)
} elseif ($link -and $link.LinkType) {
    throw 'Refusing to change an unexpected debug path link'
}
$source = "$root\crates\gpui_windows\src"
$destination = "$mirror\crates\gpui_windows\src"
New-Item -ItemType Directory -Path $destination -Force | Out-Null
Copy-Item "$source\*.hlsl" $destination -Force
Get-ChildItem "$root\target\glimm-windows\x86_64-pc-windows-gnu\debug\build\gpui_windows-*\out\*.hlsl" | ForEach-Object {
    $local = Join-Path $mirror $_.FullName.Substring($root.Length + 1)
    New-Item -ItemType Directory -Path (Split-Path $local) -Force | Out-Null
    Copy-Item $_.FullName $local -Force
}
Set-Location C:\
'Windows-local HLSL files staged for the cross-compiled debug build.'
