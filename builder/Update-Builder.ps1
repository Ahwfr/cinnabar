[CmdletBinding()]
param([switch]$Accept)

$ErrorActionPreference = 'Stop'
$checkout = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$candidate = "$checkout-update"
$branch = 'builder-features'
$trial = 'builder-update'
$baseRef = 'refs/tags/builder-base'
$trialBaseRef = 'refs/builder/update-base'
$trialFromRef = 'refs/builder/update-from'

function Invoke-BuilderGit([string]$Path, [string[]]$GitArgs) {
    $output = & git -C $Path @GitArgs
    if ($LASTEXITCODE) { throw "Git failed: $($GitArgs -join ' ')" }
    $output
}

if ((Invoke-BuilderGit $checkout @('branch', '--show-current')) -ne $branch -or
    (Invoke-BuilderGit $checkout @('status', '--porcelain'))) {
    throw "Start from a clean $branch checkout."
}
$current = Invoke-BuilderGit $checkout @('rev-parse', 'HEAD')

if ($Accept) {
    $base = Invoke-BuilderGit $checkout @('rev-parse', $baseRef)
    $nextBase = Invoke-BuilderGit $checkout @('rev-parse', $trialBaseRef)
    if ((Invoke-BuilderGit $checkout @('rev-parse', $trialFromRef)) -ne $current) {
        throw 'The main branch changed after staging the update. Review the candidate before proceeding.'
    }
    if ((Invoke-BuilderGit $candidate @('branch', '--show-current')) -ne $trial -or
        (Invoke-BuilderGit $candidate @('status', '--porcelain'))) {
        throw 'Finish the candidate rebase and commit or stash any local changes.'
    }
    $next = Invoke-BuilderGit $candidate @('rev-parse', 'HEAD')
    $null = Invoke-BuilderGit $candidate @('merge-base', '--is-ancestor', $nextBase, $next)
    $stamp = Join-Path $candidate 'target/release/builder-commit.txt'
    if (!(Test-Path -LiteralPath $stamp -PathType Leaf) -or (Get-Content -LiteralPath $stamp -Raw).Trim() -ne $next) {
        throw 'Build and play-test the candidate release before accepting it.'
    }

    $backup = "builder-backup-$(Get-Date -Format yyyyMMdd-HHmmss)"
    $null = Invoke-BuilderGit $checkout @('branch', $backup, $current)
    $null = Invoke-BuilderGit $checkout @('switch', '--detach', $current)
    try {
        $start = New-Object System.Diagnostics.ProcessStartInfo
        $start.FileName = (Get-Command git).Source
        $start.Arguments = 'update-ref --stdin'
        $start.WorkingDirectory = $checkout
        $start.UseShellExecute = $false
        $start.RedirectStandardInput = $true
        $process = [System.Diagnostics.Process]::Start($start)
        try {
            # Git's transaction protocol uses LF even on Windows.
            $process.StandardInput.Write("update refs/heads/$branch $next $current`nupdate $baseRef $nextBase $base`n")
            $process.StandardInput.Close()
            $process.WaitForExit()
            if ($process.ExitCode) { throw 'Could not update the branch and base together.' }
        } finally { $process.Dispose() }
    } finally {
        $null = Invoke-BuilderGit $checkout @('switch', $branch)
    }
    Write-Output "Accepted $next. Previous source is on $backup."
    Write-Output 'Build the accepted release in the main checkout. The tested candidate remains available in its worktree.'
    return
}

if (Test-Path -LiteralPath $candidate) {
    throw "An update worktree already exists at $candidate. Finish or remove that worktree before preparing another."
}
if ('upstream' -notin @(Invoke-BuilderGit $checkout @('remote'))) {
    $null = Invoke-BuilderGit $checkout @('remote', 'add', 'upstream', 'https://github.com/cinnabar-mc/cinnabar.git')
}
$null = Invoke-BuilderGit $checkout @('fetch', 'upstream', '+refs/heads/dev:refs/remotes/upstream/dev')
$nextBase = Invoke-BuilderGit $checkout @('rev-parse', 'upstream/dev')
$base = & git -C $checkout rev-parse --verify --quiet $baseRef
if ($LASTEXITCODE) {
    $base = Invoke-BuilderGit $checkout @('merge-base', $current, $nextBase)
    $null = Invoke-BuilderGit $checkout @('tag', 'builder-base', $base)
}
$null = Invoke-BuilderGit $checkout @('merge-base', '--is-ancestor', $base, $current)
if ($base -eq $nextBase) { Write-Output 'This fork is already on the current Cinnabar base.'; return }

$null = Invoke-BuilderGit $checkout @('worktree', 'add', '-b', $trial, $candidate, $current)
$null = Invoke-BuilderGit $checkout @('update-ref', $trialBaseRef, $nextBase)
$null = Invoke-BuilderGit $checkout @('update-ref', $trialFromRef, $current)
Write-Output "Staging $nextBase in $candidate"
$null = Invoke-BuilderGit $candidate @('rebase', '--onto', $nextBase, $base)
Write-Output "Build and play-test with $candidate\builder\Build-Builder.ps1 and Start-Builder.ps1. Then run Update-Builder.ps1 -Accept from the main checkout."
