# Test and build quick-video-transcoder.
#
# The checklist a developer runs before committing, and the one CI runs: one script, so the two cannot
# drift apart.
#
#   .\scripts\build.ps1            # test (the default)
#   .\scripts\build.ps1 build          # test, then an optimized build with the release-fast profile (thin LTO)
#   .\scripts\build.ps1 release        # test, notices, the published build (fat LTO), the .exe and its checksum in dist\
#   .\scripts\build.ps1 release -Fast  # the same with release-fast: pull request CI
#
# A release is built from a clean tree outside CI, always without -Fast.
#
# Never pipe it into Select-Object -First: that stops the pipeline mid-run, and a truncated run can look
# like a success. Read the whole output, or the last line.

param(
    [ValidateSet('test', 'build', 'release')]
    [string] $Task = 'test',
    # release only: build with the release-fast profile (thin LTO, every core) instead of the published
    # one. For pull request CI, which checks the packaging without the minutes of a fat LTO.
    [switch] $Fast
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

function Get-Version {
    (Select-String -Path (Join-Path $root 'Cargo.toml') -Pattern '^version\s*=\s*"([^"]+)"').Matches[0].Groups[1].Value
}

# The changelog section of a version, which is the release's notes: everything under
# "## [<version>] - YYYY-MM-DD" up to the next "## [" heading. $null when there is no dated section.
# The pattern is built from single-quoted pieces: in a double-quoted string PowerShell would run
# "$(.*?)" as a subexpression.
function Get-ChangelogSection([string] $text, [string] $version) {
    $pattern = '(?ms)^## \[' + [regex]::Escape($version) + '\] - \d{4}-\d{2}-\d{2}[^\r\n]*\r?\n(.*?)(?=^## \[|\z)'
    $match = [regex]::Match($text, $pattern)
    if ($match.Success) { $match.Groups[1].Value.Trim() } else { $null }
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

    # The release takes its notes from the changelog; a broken extraction would only show on a tag.
    Step "Release notes come from the changelog"
    $sample = "# Changelog`n`n## [Unreleased]`n`n- next`n`n## [1.2.0] - 2026-01-02`n`n### Added`n`n- one`n- two`n`n## [1.1.0] - 2025-12-01`n`n- old`n"
    $notes = Get-ChangelogSection $sample '1.2.0'
    if ($notes -ne "### Added`n`n- one`n- two") { Fail "the notes of a sample changelog came out as: $notes" }
    if ($null -ne (Get-ChangelogSection $sample '1.3.0')) { Fail "an undated version must have no notes" }
    if ($null -ne (Get-ChangelogSection $sample 'Unreleased')) { Fail "[Unreleased] must have no notes" }
    $version = Get-Version
    $section = Get-ChangelogSection (Get-Content (Join-Path $root 'CHANGELOG.md') -Raw) $version
    Write-Host "    $(if ($section) { "$version has a dated section" } else { "$version has no dated section yet (needed only for its release)" })"

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

# A release build embeds the third-party notices: generated here by cargo-about into target\, handed to
# build.rs through QVT_NOTICES. A plain build embeds a placeholder.
function Invoke-Build([string] $CargoProfile, [switch] $WithNotices) {
    if ($WithNotices) {
        Step "Third-party notices"
        if (-not (Get-Command cargo-about -ErrorAction SilentlyContinue)) {
            Fail "cargo-about is needed: cargo install cargo-about --locked --features cli"
        }
        $notices = Join-Path $root 'target\THIRD-PARTY-NOTICES.html'
        New-Item -ItemType Directory -Force (Split-Path -Parent $notices) | Out-Null
        Run 'cargo' @('about', 'generate', '--locked', '--fail', '--output-file', $notices, (Join-Path $root 'about.hbs'))
        Write-Host "    $([math]::Round((Get-Item $notices).Length / 1KB)) KB"
        $env:QVT_NOTICES = $notices
    }
    Step "Optimized build ($CargoProfile profile)"
    try { Run 'cargo' @('build', '--profile', $CargoProfile, '--locked') } finally { Remove-Item Env:\QVT_NOTICES -ErrorAction SilentlyContinue }
}

# Starts the built executable with one argument and returns what it printed. A windowed program:
# PowerShell does not wait for it when capturing its output, so it is waited for explicitly, its output
# going to a file in target\.
function Invoke-Executable([string] $exe, [string] $argument) {
    $output = Join-Path $root "target\qvt-output-$([guid]::NewGuid().ToString('N')).txt"
    $process = Start-Process -FilePath $exe -ArgumentList $argument -RedirectStandardOutput $output -NoNewWindow -Wait -PassThru
    $text = Get-Content $output -Raw
    Remove-Item $output
    if ($process.ExitCode -ne 0) { Fail "quick-video-transcoder $argument exited $($process.ExitCode)" }
    $text
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

# A release is one file: quick-video-transcoder.exe, with the third-party notices and the links to its
# documentation inside (About, --licenses, --version). Its name carries no version, so replacing the file
# updates it and the Send to shortcut keeps working. Next to it in dist\: SHA256SUMS.txt, and notes.md
# (the changelog section) when the version has a dated one.
function Invoke-Package([string] $CargoProfile) {
    $version = Get-Version
    $dist = Join-Path $root 'dist'
    $exe = Join-Path $root "target\$CargoProfile\quick-video-transcoder.exe"

    # Asked of the executable rather than of git: it is what users run.
    Step "The executable names its commit and carries its notices"
    $stamp = Invoke-Executable $exe '--version'
    $commit = [regex]::Match($stamp, '(?m)^commit:\s+(\S+)').Groups[1].Value
    $docLink = [regex]::Match($stamp, '(?m)^documentation:\s+(\S+)').Groups[1].Value
    $head = (& git -C $root rev-parse HEAD).Trim()
    if ($commit -ne $head) { Fail "the executable says commit '$commit', the checkout is $head (rebuild, and commit first)" }
    $repository = $docLink -replace '/blob/.*$', ''
    $notices = Invoke-Executable $exe '--licenses'
    if ($notices -notmatch 'SIL Open Font License' -or $notices.Length -lt 50KB) {
        Fail "the executable carries no third-party notices (build it with build.ps1 release)"
    }
    Write-Host "    $commit, notices $([math]::Round($notices.Length / 1KB)) KB"

    Step "Checksum and notes"
    if (Test-Path $dist) { Remove-Item -Recurse -Force $dist }
    New-Item -ItemType Directory -Force $dist | Out-Null
    $published = Join-Path $dist 'quick-video-transcoder.exe'
    Copy-Item $exe $published
    $hash = (Get-FileHash $published -Algorithm SHA256).Hash.ToLowerInvariant()
    "$hash  quick-video-transcoder.exe" | Set-Content -NoNewline -Encoding ascii (Join-Path $dist 'SHA256SUMS.txt')
    $section = Get-ChangelogSection (Get-Content (Join-Path $root 'CHANGELOG.md') -Raw) $version
    if ($section) {
        $slug = $repository -replace '^https://github.com/', ''
        $footer = "`n`n---`n`nOne file, nothing to install: download ``quick-video-transcoder.exe`` and run it. It needs " +
                  "FFmpeg, and offers to install it if it is missing. Built from $commit; documentation of this " +
                  "version: $docLink. Check the download with ``gh attestation verify quick-video-transcoder.exe --repo $slug``."
        Set-Content -Path (Join-Path $dist 'notes.md') -Encoding utf8 -NoNewline -Value ($section + $footer)
    }
    Write-Host "    $published"
    Write-Host "    sha256 $hash"
    Write-Host "    $(if ($section) { 'notes.md from the changelog' } else { "no notes: CHANGELOG.md has no dated section for $version" })"
}

if ($Task -eq 'release') { Assert-CleanTree }
Invoke-Tests
if ($Fast -and $Task -ne 'release') { Fail "-Fast only applies to the release task" }
if ($Task -eq 'build') { Invoke-Build 'release-fast' }
if ($Task -eq 'release') {
    $cargoProfile = if ($Fast) { 'release-fast' } else { 'release' }
    Invoke-Build $cargoProfile -WithNotices
    Invoke-Package $cargoProfile
    if ($Fast) { Write-Host "    built with release-fast: a check, not an executable to publish" -ForegroundColor Yellow }
}
Write-Host ""
Write-Host "$Task OK" -ForegroundColor Green
