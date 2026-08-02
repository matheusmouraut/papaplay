# Abre, fecha e inspeciona o PapaPlay ja compilado, sem precisar do Claude Code
# nem do servidor do Vite.
#
# Uso:
#   pwsh -File scripts/papaplay.ps1            # abre (start e o padrao)
#   pwsh -File scripts/papaplay.ps1 stop
#   pwsh -File scripts/papaplay.ps1 restart
#   pwsh -File scripts/papaplay.ps1 status
#
# O binario sai de `pnpm tauri build`. O `pnpm tauri dev` produz um exe no mesmo
# caminho, mas aquele espera o servidor do Vite em localhost:1420 e abre uma
# janela em branco se rodar sozinho -- por isso este script checa a data do
# arquivo contra a do `dist/`.
#
# ATENCAO: manter este arquivo em ASCII puro. Travessao em .ps1 vira aspa de
# fechamento no PowerShell 5.1 e quebra o parser longe da linha do erro.

[CmdletBinding()]
param(
    [ValidateSet('start', 'stop', 'restart', 'status')]
    [string]$Acao = 'start'
)

$ErrorActionPreference = 'Stop'

$root = Split-Path -Parent $PSScriptRoot
$exe = Join-Path $root 'src-tauri\target\release\papaplay.exe'
$dist = Join-Path $root 'dist\index.html'

function Get-App {
    # -ErrorAction SilentlyContinue nao basta: sem processo o cmdlet ainda faz o
    # script sair com 1. Ver a nota de exit code no CLAUDE.md do harness.
    try {
        Get-Process -Name 'papaplay' -ErrorAction Stop
    } catch {
        $null
    }
}

function Stop-App {
    $app = Get-App
    if (-not $app) {
        Write-Host 'PapaPlay nao esta rodando.'
        return
    }
    # Sem tray e sem quit-on-close, fechar a janela principal deixa o processo
    # vivo por causa da overlay: matar e o unico caminho por enquanto.
    $app | Stop-Process -Force
    Write-Host ('PapaPlay fechado (pid ' + ($app.Id -join ', ') + ').')
}

function Start-App {
    if (-not (Test-Path $exe)) {
        throw "Nao achei $exe. Rode: pnpm tauri build"
    }

    $app = Get-App
    if ($app) {
        # O plugin single-instance ja traz a janela principal de volta, entao
        # abrir de novo e inofensivo -- mas avisar evita confusao.
        Write-Host ('PapaPlay ja esta rodando (pid ' + ($app.Id -join ', ') + '). Abrindo de novo traz a janela para a frente.')
    }

    if ((Test-Path $dist) -and ((Get-Item $exe).LastWriteTime -lt (Get-Item $dist).LastWriteTime)) {
        Write-Warning 'O exe e mais antigo que o dist/. Se a UI parecer velha ou a janela abrir em branco, rode: pnpm tauri build'
    }

    Start-Process -FilePath $exe
    Write-Host "PapaPlay aberto: $exe"
    Write-Host 'Espiar: segure Alt+X. Abrir o card: clique ou Alt+C. Fechar o card: Esc.'
}

function Show-Status {
    $app = Get-App
    if ($app) {
        $app | Format-Table Id, ProcessName, StartTime, @{ Name = 'RAM(MB)'; Expression = { [math]::Round($_.WorkingSet64 / 1MB) } } -AutoSize
    } else {
        Write-Host 'PapaPlay nao esta rodando.'
    }

    if (Test-Path $exe) {
        Write-Host ('exe: ' + $exe + ' (' + (Get-Item $exe).LastWriteTime + ')')
    } else {
        Write-Host "exe: nao compilado ainda. Rode: pnpm tauri build"
    }
}

switch ($Acao) {
    'start' { Start-App }
    'stop' { Stop-App }
    'restart' {
        Stop-App
        Start-App
    }
    'status' { Show-Status }
}
