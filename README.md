# PapaPlay

Aprenda inglês jogando. Overlay para Windows: passe o mouse numa palavra no jogo, veja a tradução PT-BR com contexto, salve num deck e revise com repetição espaçada (FSRS). 100% offline.

- **Documentação completa:** [`docs/`](docs/README.md)
- **Instruções para o Claude Code:** [`CLAUDE.md`](CLAUDE.md)
- **Estado:** Fase 0 fechada — as duas spikes deram GO (ver [`docs/spikes/`](docs/spikes/)). Fase 1 em andamento.

## Setup

Três artefatos ficam fora do git por serem grandes e regeráveis. Sem eles o app sobe, mas a consulta falha com uma mensagem dizendo qual comando rodar. O deck (`%APPDATA%/papaplay/papaplay.db`) não está nessa lista: ele é criado e migrado sozinho no primeiro salvamento.

```powershell
pnpm install
powershell -File scripts/fetch-ocr-models.ps1   # modelos de OCR, ~11 MB
pnpm run build:dict                             # dict.db, ~42 MB (baixa ~500 MB de dumps, só na 1a vez)
powershell -File scripts/export-nmt.ps1         # tradutor EN->PT em ONNX int8, ~350 MB (precisa de Python)
pnpm tauri dev
```

## Como usar

- **Alt+X** — entra em modo consulta: lê o entorno do cursor e destaca as palavras.
- **Passar o mouse** numa palavra — tradução curta; **clicar** — acepções, IPA, a frase de contexto, a tradução dela e o botão de salvar no deck.
- **Esc** — volta ao modo passivo (a overlay continua visível, mas para de interceptar o mouse).
- **F9** — painel de diagnóstico com as latências de captura e OCR. Sai quando houver tela de configurações.

## Ordem de desenvolvimento

Fase 1 (MVP) na ordem de [`docs/05-roadmap.md`](docs/05-roadmap.md).
