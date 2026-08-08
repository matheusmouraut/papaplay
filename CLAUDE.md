# PapaPlay

Overlay para Windows que transforma jogos em aprendizado de inglês: OCR local + dicionário offline + tradução offline + deck com FSRS. 100% offline no núcleo, zero custo por uso.

**Stack:** Tauri 2 (core Rust) + React + TypeScript + Vite + Tailwind. SQLite. ts-fsrs na UI.

## Estado atual do projeto

**Fase 1 (MVP) completa.** Overlay + espiar, captura, OCR, dicionário, tradução de frases, deck, revisão FSRS, estatísticas, export CSV, atalhos configuráveis, tray, onboarding e instalador — os 11 itens de `docs/05-roadmap.md`. O que falta é a Fase 2.

**Distribuição:** o instalador NSIS leva dicionário + OCR (~55 MB). Os dois `.onnx` do tradutor de frases (332 MB) ficam de fora e são baixados no primeiro uso, para `%APPDATA%/papaplay/nmt/` — ver `src-tauri/src/setup.rs`. Sem esse download o app funciona; só a tradução da frase de contexto fica indisponível.

## Documentação — LER antes de mudanças de escopo

Toda a especificação está em `/docs`:

- `docs/README.md` — índice e resumo
- `docs/02-escopo-mvp.md` — o que entra/não entra no MVP (não adicionar escopo sem discutir)
- `docs/03-funcionalidades.md` — spec detalhada de cada feature (F1–F8) com critérios de aceite
- `docs/04-arquitetura.md` — stack, componentes, modelo de dados SQLite, estrutura do repo
- `docs/05-roadmap.md` — fases; `docs/07-diferenciais.md` — features de diferenciação priorizadas
- `docs/spikes/` — roteiros das spikes da Fase 0

## Comandos

- `pnpm tauri dev` — app em dev (OCR ~3,4x mais lento; medir latência só em release)
- `pnpm tauri build` — instalador + exe standalone em `src-tauri/target/release/`
- `pnpm app` / `pnpm app:stop` / `pnpm app:status` — abre/fecha o app já compilado, sem servidor do Vite
- `pnpm test` — testes da UI · `cargo test` (em `src-tauri/`) — testes do core
- `pnpm lint && cargo clippy` — lint
- `pnpm run build:dict` — reconstrói dict.db a partir dos dumps (script em `scripts/`)

## Arquitetura (resumo)

- `src/overlay/` — janela overlay (transparente, click-through alternável)
- `src/main/` — janela principal: Revisar, Deck, Estatísticas, Configurações + o wizard de primeira execução
- `src/shared/` — componentes (`components/ui.tsx` = primitivos da F7), hooks, tipos; wrapper do ts-fsrs em `src/shared/srs/`; paleta em `styles/theme.css`
- `site/` — landing page, HTML e CSS à mão, sem build (publicada por `.github/workflows/site.yml`)
- `src-tauri/src/` — core Rust: `capture/`, `ocr/`, `dict.rs`, `translate.rs`, `deck.rs`, `review.rs`, `stats.rs`, `hotkeys.rs`, `settings.rs`, `setup.rs`, `tray.rs`
- Dados do usuário: SQLite em `%APPDATA%/papaplay/papaplay.db`; screenshots em `media/` (arquivos .webp, path no banco)
- Dicionário: SQLite read-only embarcado em `src-tauri/resources/`

## Regras invioláveis

1. **NUNCA injetar código/DLL/hook no processo de jogos** — risco de anti-cheat. Captura externa de tela apenas (Windows Graphics Capture).
2. **Núcleo 100% offline** — nenhuma chamada de rede em runtime. A única exceção é o download do tradutor no setup (`setup.rs`), disparado pelo usuário. Espiar, traduzir, salvar e revisar funcionam com o cabo desconectado.
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
| Interação          | "Espiar": segurar `Alt+X` mostra, soltar esconde. Clique ou `Alt+C` abre o card. Sem modo persistente, sem roubo de foco do jogo, sem caixas em volta das palavras (F1) |
| Visual             | Minimalista tipográfico em tons pastéis, referência lookupper.com (F7). Janela principal **clara** (creme/branco, verde-papagaio como acento); overlay **escura** (vidro sobre o jogo). Os dois lados usam os mesmos tokens — o `overlay.css` redefine os valores |
| Marca              | Papagaio de perfil cujo bico é o triângulo de play (`src/assets/logo.svg`). Coral e amarelo só na marca, nunca em dado |
| Ciclo de vida      | Fechar a janela principal encerra o processo. A bandeja existe enquanto o app roda ("Abrir"/"Sair"); para espiar durante o jogo, o app fica aberto — minimizado basta |
| Instalador         | Enxuto (~55 MB): o tradutor de frases é baixado no setup   |

## Convenções

- Commits: conventional commits em inglês (`feat:`, `fix:`, `chore:`...)
- Branches: `feat/<nome>`, `spike/<nome>`
- Rust: `cargo fmt` + clippy sem warnings; TS: eslint + prettier
- Testes junto da feature, não depois; fixtures de OCR em `tests/fixtures/screens/`
- Conclusões de spikes documentadas em `docs/spikes/<spike>-resultado.md`
