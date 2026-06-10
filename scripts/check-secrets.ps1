$ErrorActionPreference = "Stop"

$root = Split-Path -Parent $PSScriptRoot
$excludedDirectories = @(
    ".git",
    ".install-test-final",
    "dist",
    "node_modules",
    "target",
    "记忆"
)
$textExtensions = @(
    ".css", ".html", ".js", ".json", ".md", ".ps1", ".rs", ".sql",
    ".toml", ".ts", ".tsx", ".txt", ".yaml", ".yml"
)
$patterns = @(
    @{
        Name = "JWT-like access token"
        Regex = 'eyJ[A-Za-z0-9_-]{20,}\.[A-Za-z0-9_-]{20,}\.[A-Za-z0-9_-]{20,}'
    },
    @{
        Name = "TMDB v3 API key"
        Regex = '(?i)(api[_-]?key|tmdb[_-]?api[_-]?key)\s*[:=]\s*["'']?[a-f0-9]{32}'
    },
    @{
        Name = "Hard-coded bearer token"
        Regex = '(?i)bearer\s+[A-Za-z0-9._~-]{32,}'
    },
    @{
        Name = "API key in URL"
        Regex = '(?i)[?&]api_key=[A-Za-z0-9_-]{20,}'
    }
)

$findings = @()
Get-ChildItem -LiteralPath $root -Recurse -File | Where-Object {
    $relativePath = $_.FullName.Substring($root.Length).TrimStart([char[]]"\/")
    $segments = $relativePath -split '[\\/]'
    $isExcluded = $false
    foreach ($directory in $excludedDirectories) {
        if ($segments -contains $directory) {
            $isExcluded = $true
            break
        }
    }
    -not $isExcluded -and $textExtensions -contains $_.Extension.ToLowerInvariant()
} | ForEach-Object {
    $relativePath = $_.FullName.Substring($root.Length).TrimStart([char[]]"\/")
    $lineNumber = 0
    Get-Content -LiteralPath $_.FullName | ForEach-Object {
        $lineNumber += 1
        $line = $_
        foreach ($pattern in $patterns) {
            if ($line -match $pattern.Regex) {
                $findings += [PSCustomObject]@{
                    File = $relativePath
                    Line = $lineNumber
                    Type = $pattern.Name
                }
            }
        }
    }
}

if ($findings.Count -gt 0) {
    Write-Error ("Potential secrets detected:`n" + ($findings | Format-Table -AutoSize | Out-String))
}

Write-Host "Secret check passed: no token-like values were found."
