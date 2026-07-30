# 06 — Plano de desenvolvimento com Claude Code

Como estruturar o repositório para desenvolver com Claude Code de forma eficiente.

## CLAUDE.md (raiz do repo)

Conteúdo essencial:

```markdown
# PapaPlay

Overlay Windows para aprender inglês jogando: OCR local + dicionário offline

- tradução offline + deck com FSRS. Tauri 2 (Rust) + React/TS. 100% offline.

## Comandos

- `pnpm tauri dev` — app em dev
- `pnpm tauri build` — instalador
- `pnpm test` / `cargo test` — testes UI / core
- `pnpm lint && cargo clippy` — lint
- `pnpm run build:dict` — reconstrói dict.db a partir dos dumps

## Arquitetura (resumo)

- `src/overlay/` janela overlay; `src/main/` janela principal; `src/shared/` comum
- `src-tauri/src/` core Rust: capture, ocr, dict, translate, deck, hotkeys
- Dados: SQLite em %APPDATA%/papaplay/papaplay.db; screenshots em media/
- Docs completas em /docs — LER antes de mudanças de escopo

## Regras

- NUNCA injetar código/DLL no processo de jogos (anti-cheat)
- Núcleo 100% offline: nenhuma chamada de rede em runtime do MVP
- Toda mudança de schema SQLite via migration numerada
- Estado FSRS só muda através do wrapper ts-fsrs em src/shared/srs/
- Performance: overlay passivo ~0% CPU; lookup <1s (há testes disso)
- UI em PT-BR
```

## .claude/ do projeto

```
.claude/
├── settings.json            # permissões (permitir cargo/pnpm, negar rede em runtime de teste)
├── commands/                # slash commands (skills de projeto)
│   ├── spike-report.md      # /spike-report — roda benchmarks de OCR/latência e resume
│   ├── new-migration.md     # /new-migration — cria migration SQLite numerada + teste
│   ├── build-dict.md        # /build-dict — pipeline wiktextract → dict.db com validações
│   ├── release.md           # /release — checklist: versão, changelog, build, smoke test
│   └── screen-test.md       # /screen-test — roda OCR nos screenshots de fixtures e compara com gabarito
└── agents/
    ├── rust-core.md         # subagente focado no core Rust (capture/ocr/translate)
    ├── ui-review.md         # subagente de revisão de UX/acessibilidade das telas
    └── qa.md                # subagente que roda a suíte + testes manuais roteirizados
```

**Skills (commands) valem ouro aqui porque o projeto tem pipelines repetitivos:** regenerar dicionário, testar OCR contra fixtures, migrations. Cada skill documenta o procedimento uma vez e o Claude Code executa sempre igual.

## Fixtures de teste (fazer cedo)

- `tests/fixtures/screens/` — screenshots reais de jogos (RPG com diálogo, menu, texto sobre cenário) + `expected.json` com palavras/gabarito. O `/screen-test` compara OCR vs gabarito e reporta acurácia. Isso transforma "o OCR piorou?" em um número.
- Banco SQLite de exemplo com 50 cards em estados FSRS variados para testar a fila de revisão.

## Fluxo de trabalho sugerido

1. **Sessão 1:** scaffold (repo, Tauri 2, React, CI simples) + CLAUDE.md + .claude/.
2. **Spike (Fase 0):** uma sessão por risco; salvar conclusões em `docs/spikes/`.
3. **Feature por sessão:** cada item da ordem da Fase 1 numa sessão/branch, sempre com testes; Claude Code atualiza docs quando o comportamento muda.
4. **Plan mode para features grandes** (overlay, pipeline OCR): planejar antes, aprovar, depois codar.
5. **`/screen-test` e suíte completa antes de cada merge.**

## Decisões já tomadas (não rediscutir em sessões futuras)

| Decisão    | Valor                                  |
| ---------- | -------------------------------------- |
| Escopo MVP | Jogos no PC, Windows, EN→PT-BR         |
| IA         | Fora do MVP (Fase 3, opcional/premium) |
| Stack      | Tauri 2 + Rust core + React/TS         |
| OCR        | RapidOCR/ONNX (fallback Windows OCR)   |
| Tradução   | Marian/Bergamot offline                |
| SRS        | FSRS via ts-fsrs                       |
| Banco      | SQLite único + media/                  |
| Anti-cheat | Captura externa apenas, zero injeção   |
