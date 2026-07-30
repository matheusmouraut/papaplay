# Spike 02 — OCR com posição por palavra em telas de jogos

**Pergunta a responder:** o RapidOCR (modelos PaddleOCR em ONNX via crate `ort`) extrai palavras com bounding boxes de screenshots reais de jogos, com acurácia e latência aceitáveis em CPU?

**Branch:** `spike/ocr` · **Timebox:** 1–2 dias · Independente da spike 01 (pode rodar em paralelo).

## Preparação

1. Capturar 6+ screenshots 1080p reais e salvar em `tests/fixtures/screens/`:
   - 2 de diálogo de RPG (texto em caixa, fonte serif/estilizada)
   - 2 de UI/menu (fonte limpa)
   - 1 com texto sobre cenário animado/fundo complexo
   - 1 com fonte pequena (legenda de jogo)
2. Para cada uma, criar `<nome>.expected.json` com as palavras do gabarito (formato em `.claude/commands/screen-test.md`).

## Roteiro

1. Programa Rust mínimo (`spikes/ocr/` ou binário no src-tauri): carrega imagem → detecção + reconhecimento com modelos RapidOCR (PP-OCRv4/v5 det+rec en, ONNX) → imprime `[{word, bbox, confidence}]` e desenha as bboxes numa cópia da imagem para inspeção visual.
2. Agrupar palavras em linhas pela geometria (baseline Y próxima, gap X limitado) — necessário para a frase de contexto.
3. Medir latência por frame (frio e quente) na sua CPU.
4. Rodar contra o gabarito: recall e precision por imagem.
5. Testar pré-processamento nas imagens ruins: grayscale, upscale 2×, binarização — documentar ganho.
6. Comparar com Windows.Media.Ocr (crate windows) nas mesmas imagens — tabela comparativa (acurácia × latência × peso).

## Critérios GO

- [ ] Recall ≥90% nas imagens de UI/diálogo; ≥75% na de fundo complexo
- [ ] Bboxes visualmente corretas (sobreposição certa palavra a palavra)
- [ ] Latência <500ms por frame 1080p (quente, CPU)
- [ ] Agrupamento em linhas produz frases utilizáveis
- [ ] Modelos embarcáveis (<50MB total)

## Registrar em `spike-ocr-resultado.md`

Tabela recall/precision/latência por imagem e engine, decisão RapidOCR vs Windows OCR, ajustes de pré-processamento adotados, veredito. Plano B se NO-GO: OneOCR, Tesseract 5 com LSTM, ou VLM local pequeno (última opção).
