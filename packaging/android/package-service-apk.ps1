param(
    [string]$InputApk = "target\release\apk\wallpaper-rs.apk",
    [string]$OutputApk = "target\release\apk\wallpaper-rs-android-video.apk"
)

$ErrorActionPreference = "Stop"
$repoRoot = Resolve-Path (Join-Path $PSScriptRoot "..\..")
if (-not (Test-Path $InputApk)) {
    throw "找不到输入 APK：$InputApk（先运行 `cargo apk build --release --lib` 构建）"
}
$sdk = if ($env:ANDROID_HOME) { $env:ANDROID_HOME } elseif ($env:ANDROID_SDK_ROOT) { $env:ANDROID_SDK_ROOT } else { "C:\Android\sdk" }
$buildTools = Join-Path $sdk "build-tools\34.0.0"
$aapt = Join-Path $buildTools "aapt2.exe"
$aapt1 = Join-Path $buildTools "aapt.exe"
$platform = Join-Path $sdk "platforms\android-34\android.jar"
$workDir = Join-Path $repoRoot "work\android-package"
$asciiWork = "C:\Android\wallpaper-rs-build"
$javaDir = Join-Path $asciiWork "java"
$resDir = Join-Path $asciiWork "res"
$manifest = Join-Path $asciiWork "AndroidManifest-service.xml"

Remove-Item $workDir -Recurse -Force -ErrorAction SilentlyContinue
Remove-Item $asciiWork -Recurse -Force -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force "$workDir\classes", "$workDir\compiled-res", "$workDir\apk", `
    (Split-Path $javaDir), $resDir | Out-Null
Copy-Item (Join-Path $PSScriptRoot "java") $javaDir -Recurse -Force
Copy-Item (Join-Path $PSScriptRoot "res\*") $resDir -Recurse -Force
Copy-Item (Join-Path $repoRoot "assets\res\*") $resDir -Recurse -Force

# 合并清单的版本号必须与 base APK 完全一致（系统按 versionCode 判断升级安装），
# 这里从 base APK 里解析后替换模板占位符。
$badging = (& $aapt1 dump badging $InputApk) -join "`n"
$versionCode = [regex]::Match($badging, "versionCode='(\d+)'").Groups[1].Value
$versionName = [regex]::Match($badging, "versionName='([^']+)'").Groups[1].Value
if (-not $versionCode -or -not $versionName) {
    throw "无法从 $InputApk 解析 versionCode / versionName"
}
$template = [System.IO.File]::ReadAllText(
    (Join-Path $PSScriptRoot "AndroidManifest-service.xml"),
    [System.Text.Encoding]::UTF8)
$template = $template.Replace("__VERSION_CODE__", $versionCode).Replace("__VERSION_NAME__", $versionName)
[System.IO.File]::WriteAllText($manifest, $template, (New-Object System.Text.UTF8Encoding($true)))
Write-Host "合并清单版本：versionCode=$versionCode versionName=$versionName"

& javac -encoding UTF-8 --release 8 -cp $platform -d "$workDir\classes" `
    (Get-ChildItem $javaDir -Recurse -Filter *.java).FullName
if ($LASTEXITCODE -ne 0) { throw "javac failed" }

& "$buildTools\d8.bat" --release --lib $platform --min-api 24 --output "$workDir\classes" `
    (Get-ChildItem "$workDir\classes" -Recurse -Filter *.class).FullName
if ($LASTEXITCODE -ne 0) { throw "d8 failed" }

& $aapt compile --dir $resDir -o "$workDir\resources.zip"
if ($LASTEXITCODE -ne 0) { throw "aapt2 compile failed" }
& $aapt link -I $platform --manifest $manifest `
    -o "$workDir\compiled-res\service-res.apk" "$workDir\resources.zip"
if ($LASTEXITCODE -ne 0) { throw "aapt2 link failed" }

Copy-Item $InputApk "$workDir\apk\base.apk" -Force
$dexPath = Join-Path $workDir "classes\classes.dex"
$merged = "$workDir\apk\merged.apk"
Remove-Item $merged -ErrorAction SilentlyContinue

# Rebuild the APK as a brand-new archive (Create mode). .NET's Update mode does
# not honour CompressionLevel::NoCompression here, so resources.arsc ended up
# deflated and Android 11+ rejected the install with:
#   "-124: ... resources.arsc ... uncompressed and aligned".
# Writing a fresh archive guarantees resources.arsc is STORED; zipalign then
# fixes its 4-byte alignment.
Add-Type -AssemblyName System.IO.Compression.FileSystem
$baseZip = [System.IO.Compression.ZipFile]::OpenRead("$workDir\apk\base.apk")
$svcZip = [System.IO.Compression.ZipFile]::OpenRead("$workDir\compiled-res\service-res.apk")
$outFs = [System.IO.File]::Open($merged, "Create")
$outZip = New-Object System.IO.Compression.ZipArchive($outFs, "Create")
try {
    $svcNames = @{}
    foreach ($e in $svcZip.Entries) { $svcNames[$e.FullName] = $true }

    # (1) resources.arsc first, stored uncompressed.
    $arsc = $svcZip.GetEntry("resources.arsc")
    $ne = $outZip.CreateEntry("resources.arsc", [System.IO.Compression.CompressionLevel]::NoCompression)
    $os = $ne.Open(); $arsc.Open().CopyTo($os); $os.Close()

    # (2) merged AndroidManifest.xml + service resources (from the linked APK).
    foreach ($e in $svcZip.Entries) {
        if ($e.FullName -eq "resources.arsc") { continue }
        $ne = $outZip.CreateEntry($e.FullName, [System.IO.Compression.CompressionLevel]::Optimal)
        $os = $ne.Open(); $e.Open().CopyTo($os); $os.Close()
    }

    # (3) native libraries and the rest of the cargo-apk base (skip replaced entries).
    foreach ($e in $baseZip.Entries) {
        if ($e.FullName -eq "resources.arsc") { continue }
        if ($e.FullName -eq "classes.dex") { continue }
        if ($svcNames.ContainsKey($e.FullName)) { continue }
        $ne = $outZip.CreateEntry($e.FullName, [System.IO.Compression.CompressionLevel]::Optimal)
        $os = $ne.Open(); $e.Open().CopyTo($os); $os.Close()
    }

    # (4) the live-wallpaper service bytecode.
    $ne = $outZip.CreateEntry("classes.dex", [System.IO.Compression.CompressionLevel]::Optimal)
    $os = $ne.Open(); $dex = [System.IO.File]::OpenRead($dexPath); $dex.CopyTo($os); $dex.Close(); $os.Close()
} finally {
    $outZip.Dispose()
    $outFs.Close()
    $baseZip.Dispose()
    $svcZip.Dispose()
}

& (Join-Path $buildTools "zipalign.exe") -f -p 4 $merged "$workDir\apk\aligned.apk"
if ($LASTEXITCODE -ne 0) { throw "zipalign failed" }

# 签名密钥放在固定位置（work/ 下，不随每次构建的临时目录一起删除）：
# 每次重新生成密钥会导致「覆盖安装」失败（签名不一致）。
$keystore = Join-Path $repoRoot "work\android-debug.keystore"
if (-not (Test-Path $keystore)) {
    $keytool = if ($env:JAVA_HOME) { Join-Path $env:JAVA_HOME "bin\keytool.exe" } else { "keytool" }
    & $keytool -genkeypair -keystore $keystore -storepass android -keypass android `
        -alias androiddebugkey -keyalg RSA -keysize 2048 -validity 10000 `
        -dname "CN=Android Debug,O=Android,C=US" | Out-Null
    if ($LASTEXITCODE -ne 0) { throw "keytool failed" }
}

& (Join-Path $buildTools "apksigner.bat") sign --ks $keystore --ks-pass pass:android `
    --key-pass pass:android --out $OutputApk "$workDir\apk\aligned.apk"
if ($LASTEXITCODE -ne 0) { throw "apksigner failed" }
& (Join-Path $buildTools "apksigner.bat") verify --verbose $OutputApk
& (Join-Path $buildTools "aapt.exe") dump badging $OutputApk | Select-String -Pattern "native-code|sdkVersion|uses-permission"
Write-Host "Packaged: $OutputApk"
