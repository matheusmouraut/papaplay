# PapaPlay

Overlay para Windows que transforma jogos em aprendizado de inglês: OCR local + dicionário offline + tradução offline + deck com FSRS. 100% offline no núcleo, zero custo por uso.

**Stack:** Tauri 2 (core Rust) + React + TypeScript + Vite + Tailwind. SQLite. ts-fsrs na UI.

## Estado atual do projeto

**Pré-scaffold.** O repo contém apenas documentação e configuração do Claude Code. Primeira sessão: rodar `/bootstrap` para criar o app Tauri 2. Depois, executar as spikes em `docs/spikes/` ANTES de construir features (são o gate da Fase 0).

## Documentação — LER antes de mudanças de escopo

Toda a especificação está em `/docs`:

- `docs/README.md` — índice e resumo
- `docs/02-escopo-mvp.md` — o que entra/não entra no MVP (não adicionar escopo sem discutir)
- `docs/03-funcionalidades.md` — spec detalhada de cada feature (F1–F7) com critérios de aceite
- `docs/04-arquitetura.md` — stack, componentes, modelo de dados SQLite, estrutura do repo
- `docs/05-roadmap.md` — fases; `docs/07-diferenciais.md` — features de diferenciação priorizadas
- `docs/spikes/` — roteiros das spikes da Fase 0

## Comandos (após bootstrap)

- `pnpm tauri dev` — app em dev
- `pnpm tauri build` — instalador
- `pnpm test` — testes da UI · `cargo test` (em `src-tauri/`) — testes do core
- `pnpm lint && cargo clippy` — lint
- `pnpm run build:dict` — reconstrói dict.db a partir dos dumps (script em `scripts/`)

## Arquitetura (resumo)

- `src/overlay/` — janela overlay (transparente, click-through alternável)
- `src/main/` — janela principal: Revisar, Deck, Estatísticas, Configurações
- `src/shared/` — componentes, hooks, tipos; wrapper do ts-fsrs em `src/shared/srs/`
- `src-tauri/src/` — core Rust: `capture.rs`, `ocr.rs`, `dict.rs`, `translate.rs`, `deck.rs`, `hotkeys.rs`
- Dados do usuário: SQLite em `%APPDATA%/papaplay/papaplay.db`; screenshots em `media/` (arquivos .webp, path no banco)
- Dicionário: SQLite read-only embarcado em `src-tauri/resources/`

## Regras invioláveis

1. **NUNCA injetar código/DLL/hook no processo de jogos** — risco de anti-cheat. Captura externa de tela apenas (Windows Graphics Capture).
2. **Núcleo 100% offline** — nenhuma chamada de rede em runtime no MVP. Downloads só de modelos/dicionário em install/setup.
3. **Toda mudança de schema SQLite via migration numerada** (usar `/new-migration`).
4. **Estado FSRS só muda através do wrapper** em `src/shared/srs/` — nunca manipular campos fsrs_* diretamente.
5. **Performance é feature:** overlay passivo ~0% CPU; lookup completo <1s; tooltip <300ms. Há critérios de aceite em docs/03.
6. **UI em PT-BR.**
7. Screenshots de usuário nunca saem da máquina; sem telemetria.

## Decisões já tomadas (não rediscutir)

| Decisão            | Valor                                                       |
| ------------------ | ----------------------------------------------------------- |
| Escopo MVP         | Jogos no PC, Windows, EN→PT-BR                              |
| IA                 | Fora do MVP (Fase 3, opcional/premium)                      |
| OCR                | RapidOCR/ONNX via crate `ort` (fallback: Windows.Media.Ocr) |
| Tradução de frases | Marian/Bergamot offline (OPUS-MT en→pt)                     |
| Dicionário         | Wiktionary via kaikki.org/wiktextract → SQLite + wordfreq   |
| SRS                | FSRS via ts-fsrs                                            |
| Banco              | SQLite único + pasta media/                                 |
| Card               | Por lema; contextos múltiplos anexados ao mesmo card        |

## Convenções

- Commits: conventional commits em inglês (`feat:`, `fix:`, `chore:`...)
- Branches: `feat/<nome>`, `spike/<nome>`
- Rust: `cargo fmt` + clippy sem warnings; TS: eslint + prettier
- Testes junto da feature, não depois; fixtures de OCR em `tests/fixtures/screens/`
- Conclusões de spikes documentadas em `docs/spikes/<spike>-resultado.md`
