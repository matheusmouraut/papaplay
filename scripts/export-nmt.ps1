# Monta o ambiente Python e roda scripts/export-nmt.py, que gera o tradutor
# EN->PT em ONNX int8 dentro de src-tauri/resources/nmt/.
#
# Rodar UMA VEZ, no setup. O nucleo do app e 100% offline em runtime (regra 2
# do CLAUDE.md): nada aqui e chamado durante o uso normal.
#
# Uso:
#   powershell -File scripts/export-nmt.ps1
#   powershell -File scripts/export-nmt.ps1 -Force   # refaz o export fp32
#
# POR QUE O uv E POR QUE 3.12: no Python 3.14 o functools.partial virou
# descritor, entao um partial guardado como atributo de classe passa a ligar o
# self como se fosse metodo. O optimum usa esse padrao em NORMALIZED_CONFIG_CLASS
# e quebra com "got multiple values for argument 'allow_new'". Em vez de torcer
# para a maquina ter um Python compativel no PATH, o uv baixa um CPython 3.12
# standalone para o cache dele. Nada e instalado no sistema nem no PATH.
#
# ATENCAO: manter este arquivo em ASCII puro. Travessao em .ps1 vira aspa de
# fechamento no PowerShell 5.1 e quebra o parser longe da linha do erro.

[CmdletBinding()]
param(
    [switch]$Force,
    [switch]$SkipValidation
)

$ErrorActionPreference = 'Stop'

$root = Split-Path -Parent $PSScriptRoot
$cache = Join-Path $root 'scripts\.cache'
$boot = Join-Path $cache 'venv-boot'
$venv = Join-Path $cache 'venv-nmt'
$python = Join-Path $venv 'Scripts\python.exe'
$uv = Join-Path $boot 'Scripts\uv.exe'

$PYTHON_ALVO = '3.12'

if (-not (Test-Path $cache)) {
    New-Item -ItemType Directory -Path $cache -Force | Out-Null
}

# 1. uv, num venv descartavel feito com o Python que existir na maquina.
if (-not (Test-Path $uv)) {
    Write-Host '[1/3] instalando o uv...'
    $host_python = (Get-Command python -ErrorAction SilentlyContinue).Source
    if (-not $host_python) {
        throw 'python nao encontrado no PATH. Instale qualquer Python 3 e rode de novo; a versao dele nao importa, o uv baixa a certa.'
    }
    & $host_python -m venv $boot
    & (Join-Path $boot 'Scripts\python.exe') -m pip install --quiet --upgrade pip uv
    if ($LASTEXITCODE -ne 0) { throw 'falha ao instalar o uv' }
} else {
    Write-Host '[1/3] uv ja instalado'
}

# 2. Ambiente do export, no Python que o optimum suporta.
if (-not (Test-Path $python)) {
    Write-Host "[2/3] criando o ambiente em Python $PYTHON_ALVO..."
    & $uv venv --python $PYTHON_ALVO $venv
    if ($LASTEXITCODE -ne 0) { throw 'falha ao criar o venv' }
    & $uv pip install --python $python torch transformers 'optimum[onnxruntime]' onnx onnxruntime sentencepiece
    if ($LASTEXITCODE -ne 0) { throw 'falha ao instalar as dependencias do export' }
} else {
    Write-Host '[2/3] ambiente do export ja existe'
}

# 3. Export propriamente dito.
Write-Host '[3/3] exportando...'
Write-Host ''

$argumentos = @((Join-Path $PSScriptRoot 'export-nmt.py'))
if ($Force) { $argumentos += '--force' }
if ($SkipValidation) { $argumentos += '--skip-validation' }

$env:HF_HUB_DISABLE_SYMLINKS_WARNING = '1'
& $python @argumentos
if ($LASTEXITCODE -ne 0) { throw "export falhou (codigo $LASTEXITCODE)" }
