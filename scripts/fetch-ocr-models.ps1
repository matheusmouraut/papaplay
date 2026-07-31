# Baixa os modelos de OCR (RapidOCR / PP-OCR en, ONNX) usados pela spike 02.
#
# Rodar UMA VEZ, no setup. O nucleo do app e 100% offline em runtime (regra 2
# do CLAUDE.md): nada aqui e chamado durante o uso normal.
#
# Uso:
#   pwsh -File scripts/fetch-ocr-models.ps1
#   pwsh -File scripts/fetch-ocr-models.ps1 -Force   # rebaixa mesmo se existir
#
# ATENCAO: manter este arquivo em ASCII puro. Travessao em .ps1 vira aspa de
# fechamento no PowerShell 5.1 e quebra o parser longe da linha do erro.

[CmdletBinding()]
param(
    [switch]$Force
)

$ErrorActionPreference = 'Stop'

$root = Split-Path -Parent $PSScriptRoot
$dest = Join-Path $root 'src-tauri\resources\models'

# sha256 conferido no primeiro download; serve para detectar troca de arquivo
# no servidor entre maquinas.
$assets = @(
    @{
        Name = 'en_PP-OCRv3_det_infer.onnx'
        Url  = 'https://huggingface.co/SWHL/RapidOCR/resolve/main/PP-OCRv4/en_PP-OCRv3_det_infer.onnx'
        Size = 2423224
        Role = 'deteccao (DBNet): acha as caixas de texto'
    },
    @{
        Name = 'en_PP-OCRv3_rec_infer.onnx'
        Url  = 'https://huggingface.co/SWHL/RapidOCR/resolve/main/PP-OCRv3/en_PP-OCRv3_rec_infer.onnx'
        Size = 8967018
        Role = 'reconhecimento (CRNN+CTC): le o texto de cada caixa'
    },
    @{
        Name = 'en_dict.txt'
        Url  = 'https://raw.githubusercontent.com/PaddlePaddle/PaddleOCR/release/2.7/ppocr/utils/en_dict.txt'
        Size = 190
        Role = 'charset do reconhecedor (95 caracteres)'
    }
)

if (-not (Test-Path $dest)) {
    New-Item -ItemType Directory -Path $dest -Force | Out-Null
}

Write-Host "Destino: $dest"
Write-Host ''

$baixados = 0
foreach ($asset in $assets) {
    $path = Join-Path $dest $asset.Name

    if ((Test-Path $path) -and (-not $Force)) {
        $atual = (Get-Item $path).Length
        Write-Host ("  [ja existe] {0} ({1:N0} bytes)" -f $asset.Name, $atual)
        continue
    }

    Write-Host ("  [baixando ] {0} - {1}" -f $asset.Name, $asset.Role)
    $tmp = "$path.download"
    try {
        Invoke-WebRequest -Uri $asset.Url -OutFile $tmp -UseBasicParsing -TimeoutSec 300
    } catch {
        if (Test-Path $tmp) { Remove-Item $tmp -Force }
        throw "falha ao baixar $($asset.Name): $($_.Exception.Message)"
    }

    $real = (Get-Item $tmp).Length
    if ($real -ne $asset.Size) {
        Remove-Item $tmp -Force
        throw "tamanho inesperado em $($asset.Name): esperado $($asset.Size), veio $real"
    }

    Move-Item $tmp $path -Force
    $baixados++
    Write-Host ("              OK, {0:N0} bytes" -f $real)
}

Write-Host ''
$total = (Get-ChildItem $dest -File | Measure-Object -Property Length -Sum).Sum
Write-Host ("Pronto. {0} arquivo(s) baixado(s); {1:N1} MB no total." -f $baixados, ($total / 1MB))
Write-Host 'Criterio GO da spike 02: modelos embarcaveis em menos de 50 MB.'

foreach ($asset in $assets) {
    $path = Join-Path $dest $asset.Name
    $hash = (Get-FileHash $path -Algorithm SHA256).Hash.ToLower()
    Write-Host ("  {0}  {1}" -f $hash.Substring(0, 16), $asset.Name)
}
