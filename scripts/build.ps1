# Test and build quick-video-transcoder.
#
# The checklist a developer runs before committing, and the one CI runs: one script, so the two cannot
# drift apart.
#
#   .\scripts\build.ps1            # test (the default)
#   .\scripts\build.ps1 build      # test, then the release build
#   .\scripts\build.ps1 release    # test, release build, zip into dist\ (from a clean tree outside CI)
#
# Never pipe it into Select-Object -First: that stops the pipeline mid-run, and a truncated run can look
# like a success. Read the whole output, or the last line.

param(
    [ValidateSet('test', 'build', 'release')]
    [string] $Task = 'test'
)

$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent (Split-Path -Parent $MyInvocation.MyCommand.Path)
Set-Location $root

$script:step = 0

function Step([string] $title) {
    $script:step++
    Write-Host ""
    Write-Host "[$script:step] $title" -ForegroundColor Cyan
}

function Fail([string] $message) {
    Write-Host ""
    Write-Host "FAILED: $message" -ForegroundColor Red
    exit 1
}

# Runs a native command and stops the script if it returns non-zero. PowerShell does not on its own.
function Run([string] $command, [string[]] $commandArgs) {
    & $command @commandArgs
    if ($LASTEXITCODE -ne 0) { Fail "$command $($commandArgs -join ' ') exited $LASTEXITCODE" }
}

function Invoke-Tests {
    Step "Formatting"
    Run 'cargo' @('fmt', '--all', '--check')
    Write-Host "    clean"

    Step "Clippy, warnings are errors"
    Run 'cargo' @('clippy', '--all-targets', '--locked', '--', '-D', 'warnings')

    # Doc comments are code too: a link to a private item or a stray [bracket] is a warning rustdoc would
    # print to whoever reads the API.
    Step "Doc comments build without warnings"
    $env:RUSTDOCFLAGS = '-D warnings'
    try { Run 'cargo' @('doc', '--no-deps', '--locked', '--quiet') } finally { Remove-Item Env:\RUSTDOCFLAGS -ErrorAction SilentlyContinue }

    Step "Tests (the end-to-end one needs FFmpeg on the PATH)"
    Run 'cargo' @('test', '--locked')

    Step "Documentation links resolve"
    # Sections too: a link to a renamed heading lands at the top of the page, which looks fine and is not.
    # GitHub's anchors: lower case, punctuation dropped, spaces to hyphens, a repeated heading numbered.
    $anchorsOf = @{}
    function Get-Anchors([string] $path) {
        if (-not $anchorsOf.ContainsKey($path)) {
            $seen = @{}
            $anchors = @{}
            $inCode = $false
            foreach ($line in Get-Content $path) {
                if ($line -match '^```') { $inCode = -not $inCode; continue }
                if ($inCode -or $line -notmatch '^#{1,6}\s+(.*)$') { continue }
                $text = $Matches[1] -replace '\[([^\]]*)\]\([^)]*\)', '$1' -replace '[`*]', ''
                $slug = ($text.ToLowerInvariant() -replace '[^\w\- ]', '') -replace ' ', '-'
                $n = [int]$seen[$slug]
                $seen[$slug] = $n + 1
                $anchors[$(if ($n) { "$slug-$n" } else { $slug })] = $true
            }
            $anchorsOf[$path] = $anchors
        }
        $anchorsOf[$path]
    }
    $broken = @()
    $pages = @('README.md', 'CONTRIBUTING.md', 'AGENTS.md', 'CHANGELOG.md' | ForEach-Object { Join-Path $root $_ }) +
             (Get-ChildItem (Join-Path $root 'docs') -Recurse -Filter '*.md' | ForEach-Object { $_.FullName })
    foreach ($page in $pages) {
        $dir = Split-Path -Parent $page
        foreach ($match in [regex]::Matches((Get-Content $page -Raw), '\]\(([^)]+)\)')) {
            $link = $match.Groups[1].Value
            if ($link -match '^https?:') { continue }
            $target, $anchor = $link -split '#', 2
            $path = if ($target) { Join-Path $dir $target } else { $page }
            if (-not (Test-Path $path)) {
                $broken += "$($page.Substring($root.Length + 1)) -> $link"
                continue
            }
            if (-not $anchor) { continue }
            if ($path -like '*.md' -and -not (Get-Anchors $path).ContainsKey($anchor)) {
                $broken += "$($page.Substring($root.Length + 1)) -> $link (no such section)"
            }
        }
    }
    if ($broken) {
        $broken | ForEach-Object { Write-Host "    broken: $_" -ForegroundColor Red }
        Fail "$($broken.Count) broken documentation link(s)"
    }
    Write-Host "    $($pages.Count) pages, every link resolves"

    # A public repository: no video (a real capture) and no personal path may be published. .gitignore
    # blocks video extensions; this catches a file added with --force, and a user folder written in text.
    Step "No video and no personal path is tracked"
    $tracked = & git ls-files
    if ($LASTEXITCODE -ne 0) { Fail "git ls-files exited $LASTEXITCODE" }
    $videos = @($tracked | Where-Object { $_ -match '\.(mp4|mkv|mov|webm|avi|m4v|ts|m2ts|qvt-partial)$' -or $_ -match '\.local(\.[^/]+)?$' })
    if ($videos) {
        $videos | ForEach-Object { Write-Host "    tracked: $_" -ForegroundColor Red }
        Fail "videos or private notes are tracked by git"
    }
    # A real user folder: C:\Users\<name>\ or /home/<name>/ with a name that is not a placeholder.
    $pattern = '(?i)([a-z]:\\users\\|/home/|/Users/)(?!<|\.\.\.|you\b|name\b|me\b|runner\b|public\b)[a-z0-9._-]+[\\/]'
    $leaks = @()
    foreach ($file in $tracked) {
        if (-not (Test-Path $file -PathType Leaf)) { continue }
        if ($file -eq 'scripts/build.ps1') { continue }
        $hits = Select-String -Path $file -Pattern $pattern -AllMatches -ErrorAction SilentlyContinue
        foreach ($hit in $hits) { $leaks += "$($file):$($hit.LineNumber)" }
    }
    if ($leaks) {
        $leaks | ForEach-Object { Write-Host "    personal path: $_" -ForegroundColor Red }
        Fail "a tracked file names a real user folder"
    }
    Write-Host "    $(@($tracked).Count) tracked files, no video, no personal path"
}

function Invoke-Build {
    Step "Release build"
    Run 'cargo' @('build', '--release', '--locked')
}

# A release ships what a commit holds, so that commit must hold everything. CI checks out a commit.
function Assert-CleanTree {
    if ($env:CI) { return }
    Step "The working tree is clean"
    $changes = & git -C $root status --porcelain
    if ($LASTEXITCODE -ne 0) { Fail "git status exited $LASTEXITCODE" }
    if ($changes) {
        $changes | ForEach-Object { Write-Host "    $_" -ForegroundColor Red }
        Fail "commit or stash first: a release must name a commit that contains what it ships"
    }
    Write-Host "    clean"
}

function Invoke-Zip {
    Step "Zip"
    $version = (Select-String -Path (Join-Path $root 'Cargo.toml') -Pattern '^version\s*=\s*"([^"]+)"').Matches[0].Groups[1].Value
    $dist = Join-Path $root 'dist'
    $stage = Join-Path $dist "quick-video-transcoder-$version"
    if (Test-Path $stage) { Remove-Item -Recurse -Force $stage }
    New-Item -ItemType Directory -Force $stage | Out-Null
    Copy-Item (Join-Path $root 'target\release\quick-video-transcoder.exe') $stage
    Copy-Item (Join-Path $root 'README.md'), (Join-Path $root 'LICENSE'), (Join-Path $root 'CHANGELOG.md') $stage
    Copy-Item (Join-Path $root 'scripts\install-sendto.ps1') $stage
    $zip = Join-Path $dist "quick-video-transcoder-$version-windows-x64.zip"
    if (Test-Path $zip) { Remove-Item -Force $zip }
    Compress-Archive -Path (Join-Path $stage '*') -DestinationPath $zip
    Remove-Item -Recurse -Force $stage
    Write-Host "    $zip"
}

if ($Task -eq 'release') { Assert-CleanTree }
Invoke-Tests
if ($Task -in 'build', 'release') { Invoke-Build }
if ($Task -eq 'release') { Invoke-Zip }
Write-Host ""
Write-Host "$Task OK" -ForegroundColor Green
