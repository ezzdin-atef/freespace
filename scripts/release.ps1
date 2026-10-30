<#
.SYNOPSIS
  Bumps the version, commits, tags vX.Y.Z and pushes. The tag starts the GitHub release workflow
  (.github/workflows/release.yml), which builds the installer and publishes it with the updater manifest.

.EXAMPLE
  npm run release                     # asks for the next version
  npm run release -- -Version 0.2.0   # no prompt for the version
  npm run release -- -DryRun          # validate only, change nothing
#>
param(
  [string]$Version,
  # Validate and show what would happen without editing, committing or pushing.
  [switch]$DryRun,
  # Skip cargo test, npm test and npm run check.
  [switch]$SkipChecks,
  # Do not ask before committing and pushing.
  [switch]$Yes
)

$ErrorActionPreference = "Stop"
Set-Location (Join-Path $PSScriptRoot "..")

$Branch = "main"
$Remote = "origin"
# Every file that carries the app version. Cargo.lock and package-lock.json follow their manifests.
$VersionFiles = @("tauri.conf.json", "Cargo.toml", "Cargo.lock", "package.json", "package-lock.json")
$Utf8 = New-Object System.Text.UTF8Encoding $false

function Fail([string]$msg) {
  Write-Host "error: $msg" -ForegroundColor Red
  exit 1
}

function Step([string]$msg) {
  Write-Host "> $msg" -ForegroundColor Cyan
}

# Runs a native command and stops on a non-zero exit code. Returns its trimmed output.
function Run([string]$exe, [string[]]$argv) {
  # Windows PowerShell turns redirected stderr into errors under "Stop"; exit codes decide here.
  $ErrorActionPreference = "Continue"
  $out = & $exe @argv 2>&1
  if ($LASTEXITCODE -ne 0) { Fail "$exe $($argv -join ' ') failed:`n$($out -join "`n")" }
  return ($out -join "`n").Trim()
}

function Read-Text([string]$path) { [System.IO.File]::ReadAllText((Resolve-Path $path), $Utf8) }
function Write-Text([string]$path, [string]$text) { [System.IO.File]::WriteAllText((Resolve-Path $path), $text, $Utf8) }

# Replaces the first match of $pattern (which must capture the prefix and suffix around the version) in $path.
function Set-VersionIn([string]$path, [string]$pattern, [string]$v) {
  $text = Read-Text $path
  $re = [regex]$pattern
  if (-not $re.IsMatch($text)) { Fail "no version found in $path" }
  Write-Text $path ($re.Replace($text, "`${1}$v`${2}", 1))
}

function Get-VersionIn([string]$path, [string]$pattern) {
  $m = [regex]::Match((Read-Text $path), $pattern)
  if (-not $m.Success) { Fail "no version found in $path" }
  return $m.Groups[2].Value
}

# Patterns: group 1 = text before the version, group 2 = version (for reading) / text after (for writing).
$JsonRead = '("version"\s*:\s*")([^"]+)"'
$JsonWrite = '("version"\s*:\s*")[^"]+(")'
$TomlRead = '(?m)(^version\s*=\s*")([^"]+)"'
$TomlWrite = '(?m)(^version\s*=\s*")[^"]+(")'
$LockWrite = '(name = "freespace"\r?\nversion = ")[^"]+(")'

$SemVer = '^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)(?:-([0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*))?$'

# -1, 0 or 1, following semver precedence (a pre-release sorts before its release).
function Compare-SemVer([string]$a, [string]$b) {
  $ma = [regex]::Match($a, $SemVer); $mb = [regex]::Match($b, $SemVer)
  for ($i = 1; $i -le 3; $i++) {
    $x = [int64]$ma.Groups[$i].Value; $y = [int64]$mb.Groups[$i].Value
    if ($x -ne $y) { if ($x -lt $y) { return -1 } else { return 1 } }
  }
  $pa = $ma.Groups[4].Value; $pb = $mb.Groups[4].Value
  if ($pa -eq $pb) { return 0 }
  if (-not $pa) { return 1 }
  if (-not $pb) { return -1 }
  $xa = $pa.Split("."); $xb = $pb.Split(".")
  for ($i = 0; $i -lt [Math]::Min($xa.Length, $xb.Length); $i++) {
    $na = $xa[$i] -match '^\d+$'; $nb = $xb[$i] -match '^\d+$'
    if ($na -and $nb) { $c = ([int64]$xa[$i]).CompareTo([int64]$xb[$i]) }
    elseif ($na) { $c = -1 }
    elseif ($nb) { $c = 1 }
    else { $c = [string]::CompareOrdinal($xa[$i], $xb[$i]) }
    if ($c -ne 0) { return [Math]::Sign($c) }
  }
  return $xa.Length.CompareTo($xb.Length)
}

function Bump([string]$v, [string]$part) {
  $m = [regex]::Match($v, $SemVer)
  $maj = [int64]$m.Groups[1].Value; $min = [int64]$m.Groups[2].Value; $pat = [int64]$m.Groups[3].Value
  # 1.2.0-beta.1 -> 1.2.0 for a patch bump: finish the pre-release instead of skipping it.
  if ($m.Groups[4].Value -and $part -eq "patch") { return "$maj.$min.$pat" }
  switch ($part) {
    "major" { return "$($maj + 1).0.0" }
    "minor" { return "$maj.$($min + 1).0" }
    default { return "$maj.$min.$($pat + 1)" }
  }
}

# ------------------------------------------------------------------ repository state

Step "Checking the repository"
foreach ($tool in "git", "npm", "cargo") {
  if (-not (Get-Command $tool -ErrorAction SilentlyContinue)) { Fail "$tool is not on PATH" }
}

$current = Run git @("rev-parse", "--abbrev-ref", "HEAD")
if ($current -ne $Branch) { Fail "releases are made from '$Branch', you are on '$current'" }

$dirty = Run git @("status", "--porcelain")
if ($dirty) { Fail "the working tree has uncommitted changes:`n$dirty" }

$remotes = (Run git @("remote")) -split "`n"
if ($remotes -notcontains $Remote) { Fail "no '$Remote' remote. Add it: git remote add $Remote https://github.com/ezzdin-atef/freespace.git" }

Run git @("fetch", $Remote, "--tags", "--quiet") | Out-Null
$upstream = Run git @("ls-remote", "--heads", $Remote, $Branch)
if ($upstream) {
  $behind = Run git @("rev-list", "--count", "HEAD..$Remote/$Branch")
  if ([int]$behind -gt 0) { Fail "'$Branch' is $behind commit(s) behind $Remote/$Branch. Pull first." }
}

# ------------------------------------------------------------------ current version

Step "Checking the version files agree"
$versions = [ordered]@{
  "tauri.conf.json" = Get-VersionIn "tauri.conf.json" $JsonRead
  "Cargo.toml"      = Get-VersionIn "Cargo.toml" $TomlRead
  "package.json"    = Get-VersionIn "package.json" $JsonRead
}
$distinct = @($versions.Values | Select-Object -Unique)
if ($distinct.Count -ne 1) {
  $list = ($versions.GetEnumerator() | ForEach-Object { "  $($_.Key): $($_.Value)" }) -join "`n"
  Fail "the version files disagree:`n$list"
}
$now = $distinct[0]
if ($now -notmatch $SemVer) { Fail "current version '$now' is not semver" }

$pubkey = Read-Text "tauri.conf.json"
if ($pubkey -match 'REPLACE_WITH_CONTENTS_OF') {
  Fail "the updater public key in tauri.conf.json is still a placeholder. See README > Releases."
}

$lastTag = (Run git @("tag", "--list", "v*", "--sort=-v:refname")) -split "`n" | Where-Object { $_ } | Select-Object -First 1
Write-Host "  current version: $now"
Write-Host "  latest tag:      $(if ($lastTag) { $lastTag } else { '(none)' })"

# ------------------------------------------------------------------ next version

if (-not $Version) {
  $patch = Bump $now "patch"; $minor = Bump $now "minor"; $major = Bump $now "major"
  Write-Host ""
  Write-Host "  [1] patch  $patch"
  Write-Host "  [2] minor  $minor"
  Write-Host "  [3] major  $major"
  Write-Host "  or type a version (e.g. 1.0.0-beta.1)"
  $answer = (Read-Host "Next version").Trim()
  switch ($answer) {
    "1" { $Version = $patch }
    "2" { $Version = $minor }
    "3" { $Version = $major }
    default { $Version = $answer }
  }
}
$Version = $Version.Trim().TrimStart("v")
$Tag = "v$Version"

Step "Validating $Tag"
if ($Version -notmatch $SemVer) { Fail "'$Version' is not a valid semver version (MAJOR.MINOR.PATCH[-pre])" }
if ((Compare-SemVer $Version $now) -le 0) { Fail "$Version must be greater than the current version $now" }
if ($lastTag -and $lastTag.TrimStart("v") -match $SemVer -and (Compare-SemVer $Version $lastTag.TrimStart("v")) -le 0) {
  Fail "$Version must be greater than the latest tag $lastTag"
}
& git rev-parse -q --verify "refs/tags/$Tag" *> $null
if ($LASTEXITCODE -eq 0) { Fail "tag $Tag already exists locally" }
if (Run git @("ls-remote", "--tags", $Remote, "refs/tags/$Tag")) { Fail "tag $Tag already exists on $Remote" }

if ($DryRun) {
  Write-Host ""
  Write-Host "Dry run: would set $now -> $Version in $($VersionFiles -join ', '), commit 'Release $Tag', tag $Tag and push to $Remote." -ForegroundColor Yellow
  exit 0
}

# ------------------------------------------------------------------ bump

function Restore-Files {
  $ErrorActionPreference = "Continue"
  & git checkout -- @VersionFiles 2>&1 | Out-Null
}

Step "Writing $Version"
Set-VersionIn "tauri.conf.json" $JsonWrite $Version
Set-VersionIn "Cargo.toml" $TomlWrite $Version
Set-VersionIn "Cargo.lock" $LockWrite $Version
# Updates package.json and both version fields in package-lock.json.
Run npm @("version", $Version, "--no-git-tag-version", "--allow-same-version") | Out-Null

foreach ($f in "tauri.conf.json", "package.json") {
  if ((Get-VersionIn $f $JsonRead) -ne $Version) { Restore-Files; Fail "$f was not updated" }
}
if ((Get-VersionIn "Cargo.toml" $TomlRead) -ne $Version) { Restore-Files; Fail "Cargo.toml was not updated" }

if (-not $SkipChecks) {
  $checks = @(
    @("cargo", @("test", "--quiet")),
    @("npm", @("test", "--silent")),
    @("npm", @("run", "check", "--silent"))
  )
  foreach ($c in $checks) {
    Step "$($c[0]) $($c[1] -join ' ')"
    & $c[0] @($c[1])
    if ($LASTEXITCODE -ne 0) { Restore-Files; Fail "$($c[0]) $($c[1] -join ' ') failed; version files restored" }
  }
}

Write-Host ""
& git --no-pager diff --stat
Write-Host ""

if (-not $Yes) {
  $ok = Read-Host "Commit, tag $Tag and push to $Remote? [y/N]"
  if ($ok -notmatch '^(y|yes)$') { Restore-Files; Write-Host "Aborted; version files restored."; exit 1 }
}

# ------------------------------------------------------------------ commit, tag, push

Step "Committing and tagging"
Run git (@("add", "--") + $VersionFiles) | Out-Null
Run git @("commit", "--quiet", "-m", "Release $Tag") | Out-Null
Run git @("tag", "-a", $Tag, "-m", "Freespace $Tag") | Out-Null

Step "Pushing $Branch and $Tag"
# Atomic: either both the release commit and the tag land, or neither does.
Run git @("push", "--atomic", $Remote, $Branch, $Tag) | Out-Null

$url = (Run git @("remote", "get-url", $Remote)) -replace '\.git$', '' -replace '^git@github\.com:', 'https://github.com/'
Write-Host ""
Write-Host "Released $Tag. The build is running at $url/actions" -ForegroundColor Green
