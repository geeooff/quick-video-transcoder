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

# The zip holds four files: the executable, its readme, its licence and the third-party notices. No copy of
# the documentation: the readme's links point at the pages of the exact commit the executable was built
# from, which describe this build however old it gets. Next to the zip: SHA256SUMS.txt, and notes.md
# (the changelog section) when the version has a dated one.
function Invoke-Package {
    $version = Get-Version
    $dist = Join-Path $root 'dist'
    $stage = Join-Path $dist "quick-video-transcoder-$version"
    $exe = Join-Path $root 'target\release\quick-video-transcoder.exe'

    # Asked of the executable rather than of git, so the readme cannot name a commit other than the one
    # compiled in.
    # A windowed program: PowerShell does not wait for it when capturing its output, so it is started and
    # waited for explicitly, its output going to a file of the staging folder.
    Step "The executable names its commit"
    if (Test-Path $stage) { Remove-Item -Recurse -Force $stage }
    New-Item -ItemType Directory -Force $stage | Out-Null
    $stampFile = Join-Path $stage 'version.txt'
    $process = Start-Process -FilePath $exe -ArgumentList '--version' -RedirectStandardOutput $stampFile -NoNewWindow -Wait -PassThru
    $stamp = @(Get-Content $stampFile)
    Remove-Item $stampFile
    if ($process.ExitCode -ne 0 -or $stamp.Count -lt 3) { Fail "quick-video-transcoder --version printed: $stamp" }
    $commit = ($stamp | Select-String -Pattern '^commit:\s+(\S+)').Matches[0].Groups[1].Value
    $docLink = ($stamp | Select-String -Pattern '^documentation:\s+(\S+)').Matches[0].Groups[1].Value
    $head = (& git -C $root rev-parse HEAD).Trim()
    if ($commit -ne $head) { Fail "the executable says commit $commit, the checkout is $head (rebuild, and commit first)" }
    $repository = $docLink -replace '/blob/.*$', ''
    Write-Host "    $commit"

    Step "Package $version"
    Copy-Item $exe $stage
    Copy-Item (Join-Path $root 'LICENSE') (Join-Path $stage 'LICENSE.txt')

    # The readme, its relative links turned into links at the build's commit.
    $base = "$repository/blob/$commit/"
    $readme = Get-Content (Join-Path $root 'README.md') -Raw
    $readme = [regex]::Replace($readme, '\]\((?!https?:|mailto:|#)([^)]+)\)', { param($m) '](' + $base + $m.Groups[1].Value + ')' })
    $header = "> Quick Video Transcoder $version, built from commit $commit. Every link below shows the documentation of this exact build.`n`n"
    Set-Content -Path (Join-Path $stage 'README.md') -Encoding utf8 -NoNewline -Value ($header + $readme)

    Step "Third-party notices"
    if (-not (Get-Command cargo-about -ErrorAction SilentlyContinue)) {
        Fail "cargo-about is needed: cargo install cargo-about --locked --features cli"
    }
    $notices = Join-Path $stage 'THIRD-PARTY-NOTICES.html'
    Run 'cargo' @('about', 'generate', '--locked', '--fail', '--output-file', $notices, (Join-Path $root 'about.hbs'))
    Write-Host "    $([math]::Round((Get-Item $notices).Length / 1KB)) KB"

    Step "Zip, checksum and notes"
    $zip = Join-Path $dist "quick-video-transcoder-$version-windows-x64.zip"
    if (Test-Path $zip) { Remove-Item -Force $zip }
    Compress-Archive -Path (Join-Path $stage '*') -DestinationPath $zip
    Remove-Item -Recurse -Force $stage
    $hash = (Get-FileHash $zip -Algorithm SHA256).Hash.ToLowerInvariant()
    "$hash  $(Split-Path -Leaf $zip)" | Set-Content -NoNewline -Encoding ascii (Join-Path $dist 'SHA256SUMS.txt')
    $notesFile = Join-Path $dist 'notes.md'
    if (Test-Path $notesFile) { Remove-Item -Force $notesFile }
    $section = Get-ChangelogSection (Get-Content (Join-Path $root 'CHANGELOG.md') -Raw) $version
    if ($section) {
        $verify = "`n`n---`n`nBuilt from $commit. Check the download with ``gh attestation verify <zip> --repo $($repository -replace '^https://github.com/', '')``."
        Set-Content -Path $notesFile -Encoding utf8 -NoNewline -Value ($section + $verify)
    }
    Write-Host "    $zip"
    Write-Host "    sha256 $hash"
    Write-Host "    $(if ($section) { 'notes.md from the changelog' } else { "no notes: CHANGELOG.md has no dated section for $version" })"
}

if ($Task -eq 'release') { Assert-CleanTree }
Invoke-Tests
if ($Task -in 'build', 'release') { Invoke-Build }
if ($Task -eq 'release') { Invoke-Package }
Write-Host ""
Write-Host "$Task OK" -ForegroundColor Green
