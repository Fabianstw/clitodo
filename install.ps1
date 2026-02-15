$repo = "Fabianstw/clitodo"
$bin = "todo.exe"

$release = "latest"
$asset = "todo-x86_64-pc-windows-msvc.tar.gz"

$url = "https://github.com/$repo/releases/$release/download/$asset"

$tmp = New-Item -ItemType Directory -Force -Path "$env:TEMP\clitodo"
$archive = "$tmp\$asset"

Write-Host "Downloading $url"
Invoke-WebRequest $url -OutFile $archive

tar -xzf $archive -C $tmp

$target = "$env:USERPROFILE\.local\bin"
New-Item -ItemType Directory -Force -Path $target | Out-Null

Copy-Item "$tmp\$bin" "$target\$bin" -Force

Write-Host "Installed to $target\$bin"
Write-Host "Add this to PATH if needed:"
Write-Host "$target"