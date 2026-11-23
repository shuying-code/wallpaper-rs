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

$dexPath = Join-Path $workDir "classes\classes.dex"
$merged = "$workDir\apk\merged.apk"
$repackerClasses = "$workDir\repacker-classes"
$javac = if ($env:JAVA_HOME) { Join-Path $env:JAVA_HOME "bin\javac.exe" } else { "javac" }
$java = if ($env:JAVA_HOME) { Join-Path $env:JAVA_HOME "bin\java.exe" } else { "java" }

# Android 11+ requires resources.arsc to be STORED and 4-byte aligned. .NET's
# CompressionLevel.NoCompression still writes a DEFLATE stream, so use the JDK
# repacker to produce a genuinely STORED resource table.
New-Item -ItemType Directory -Force "$repackerClasses" | Out-Null
& $javac -encoding UTF-8 --release 8 -d $repackerClasses `
    (Join-Path $PSScriptRoot "ApkRepacker.java")
if ($LASTEXITCODE -ne 0) { throw "apk repacker build failed" }
& $java -cp $repackerClasses ApkRepacker $InputApk `
    "$workDir\compiled-res\service-res.apk" $dexPath $merged
if ($LASTEXITCODE -ne 0) { throw "apk repacker failed" }

& (Join-Path $buildTools "zipalign.exe") -f -p 4 $merged "$workDir\apk\aligned.apk"
if ($LASTEXITCODE -ne 0) { throw "zipalign failed" }
$alignment = & (Join-Path $buildTools "zipalign.exe") -c -v -p 4 "$workDir\apk\aligned.apk"
if ($LASTEXITCODE -ne 0) { throw "zipalign verification failed" }
if ($alignment -match "resources\.arsc.*compressed") {
    throw "resources.arsc is compressed; Android 11+ will reject this APK"
}

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
