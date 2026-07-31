# PapaPlay

Aprenda inglês jogando. Overlay para Windows: passe o mouse numa palavra no jogo, veja a tradução PT-BR com contexto, salve num deck e revise com repetição espaçada (FSRS). 100% offline.

- **Documentação completa:** [`docs/`](docs/README.md)
- **Instruções para o Claude Code:** [`CLAUDE.md`](CLAUDE.md)
- **Estado:** Fase 0 fechada — as duas spikes deram GO (ver [`docs/spikes/`](docs/spikes/)). Fase 1 em andamento.

## Setup

Dois artefatos ficam fora do git por serem grandes e regeráveis. Sem eles o app sobe, mas a consulta falha com uma mensagem dizendo qual comando rodar.

```powershell
pnpm install
powershell -File scripts/fetch-ocr-models.ps1   # modelos de OCR, ~11 MB
pnpm run build:dict                             # dict.db, ~42 MB (baixa ~500 MB de dumps, só na 1a vez)
pnpm tauri dev
```

## Como usar

- **Alt+X** — entra em modo consulta: lê o entorno do cursor e destaca as palavras.
- **Passar o mouse** numa palavra — tradução curta; **clicar** — acepções, IPA e a frase de contexto.
- **Esc** — volta ao modo passivo (a overlay continua visível, mas para de interceptar o mouse).
- **F9** — painel de diagnóstico com as latências de captura e OCR. Sai quando houver tela de configurações.

## Ordem de desenvolvimento

Fase 1 (MVP) na ordem de [`docs/05-roadmap.md`](docs/05-roadmap.md).
