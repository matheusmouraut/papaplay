# 04 — Arquitetura e Stack

## Stack recomendado

| Camada             | Escolha                                                                                                                       | Por quê                                                                                                                                                                                                |
| ------------------ | ----------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Shell desktop      | **Tauri 2** (Rust + WebView)                                                                                                  | Binário pequeno, baixo consumo (crítico: roda junto com jogos), acesso nativo ao Windows via Rust, suporte a janelas transparentes/click-through/always-on-top, multi-janela (overlay + app principal) |
| UI                 | **React + TypeScript + Vite + Tailwind CSS**                                                                                  | Produtividade, ecossistema, ótimo suporte do Claude Code                                                                                                                                               |
| Estado UI          | **Zustand** + TanStack Query (para chamadas ao core)                                                                          | Simples, sem boilerplate                                                                                                                                                                               |
| Core nativo        | **Rust** (comandos Tauri)                                                                                                     | Captura de tela, OCR, hotkeys, tudo no mesmo processo — sem sidecar Python                                                                                                                             |
| Captura de tela    | **Windows Graphics Capture API** (crate `windows-capture` ou `windows-rs`)                                                    | Rápida, moderna, funciona com a maioria dos jogos borderless                                                                                                                                           |
| OCR                | **RapidOCR / modelos PaddleOCR em ONNX** via crate `ort` (ONNX Runtime)                                                       | Rápido em CPU, preciso em texto latino, 100% local, embarcável (~20MB de modelos). Plano B: Windows.Media.Ocr (nativo, zero peso, precisão menor)                                                      |
| Tradução de frases | **Bergamot / Marian NMT** (modelos OPUS-MT en→pt via `bergamot-translator` C++ com binding, ou ONNX)                          | Offline, qualidade boa para frases curtas, ~40MB por par. Plano B: chamar binário do Argos Translate                                                                                                   |
| Dicionário         | **SQLite embutido**, dados extraídos do **Wiktionary EN→PT** (dumps do kaikki.org/wiktextract) + **wordfreq** para frequência | Offline, gratuito, licença compatível (CC BY-SA — atribuir)                                                                                                                                            |
| Lematização        | Tabela de lemas embutida (ex.: derivada do LemmInflect/UDLexicons) no SQLite                                                  | Suficiente para inglês no MVP; sem runtime de NLP pesado                                                                                                                                               |
| Banco de dados     | **SQLite** (crate `sqlx` ou `rusqlite`) — um arquivo `papaplay.db`                                                            | Local, robusto, fácil backup/export                                                                                                                                                                    |
| SRS                | **ts-fsrs** (TypeScript, roda na UI; estado persistido no SQLite)                                                             | Implementação oficial da comunidade open-spaced-repetition                                                                                                                                             |
| Hotkeys globais    | Plugin `tauri-plugin-global-shortcut`                                                                                         | Oficial do Tauri                                                                                                                                                                                       |
| Updates            | `tauri-plugin-updater` + GitHub Releases                                                                                      | Distribuição simples no início                                                                                                                                                                         |

> **Validação antes de codar tudo:** os 2 riscos técnicos (overlay click-through alternando estados + OCR com bboxes por palavra) devem ser provados numa spike de 1–2 dias antes de investir no resto. Ver Fase 0 do roadmap.

## Componentes

```
┌────────────────────────────── Tauri App (1 processo) ──────────────────────────────┐
│                                                                                    │
│  Janela OVERLAY (WebView)          Janela PRINCIPAL (WebView)      Tray            │
│  - transparente/click-through      - Revisar (ts-fsrs)            - badge fila     │
│  - destaques sobre palavras        - Deck / Estatísticas          - abrir/sair     │
│  - tooltip + popup de lookup       - Configurações                                 │
│           │  eventos/commands              │ commands                              │
│  ┌────────▼────────────────────────────────▼─────────────────────┐                 │
│  │                       CORE (Rust)                             │                 │
│  │  capture: Windows Graphics Capture → frame                    │                 │
│  │  ocr: ONNX Runtime (RapidOCR) → [{word, bbox, conf}] + linhas │                 │
│  │  dict: SQLite (wiktionary + freq + lemas) → acepções          │                 │
│  │  translate: Marian/Bergamot en→pt → tradução de frase         │                 │
│  │  deck: CRUD cards + screenshots (arquivos .webp)              │                 │
│  │  settings/hotkeys                                             │                 │
│  └───────────────────────┬───────────────────────────────────────┘                 │
│                          │                                                         │
│                  SQLite (papaplay.db) + pasta media/                                 │
└────────────────────────────────────────────────────────────────────────────────────┘
```

## Fluxo do lookup (sequência)

1. Hotkey → core captura frame do monitor da janela em foco (guarda também o título da janela = nome do jogo).
2. Core roda OCR → emite `ocr_result { words[], lines[] }` para a janela overlay.
3. Overlay sai de click-through, desenha destaques nas bboxes (escaladas por DPI).
4. Hover → UI consulta `dict_lookup(word)` (lematiza → busca acepções → ordena por frequência). Tooltip.
5. Clique → UI chama `translate_sentence(line_text)` em paralelo com `dict_lookup` completo. Popup.
6. ⭐ → `deck_save { lemma, form, senses[], sentence, sentence_pt, bbox, frame_crop, game, timestamp }` → core recorta o screenshot, grava card.
7. `Esc` → overlay limpa e volta a click-through.

## Modelo de dados (SQLite)

```sql
-- Dicionário (read-only, embarcado)
dict_entries(id, lemma, pos, ipa, senses_json, freq_rank)
lemma_forms(form, lemma)                     -- "ran" → "run"

-- Dados do usuário
cards(id, lemma, created_at, suspended,
      fsrs_due, fsrs_stability, fsrs_difficulty, fsrs_state, fsrs_reps, fsrs_lapses)
card_senses(card_id, sense_text, chosen)     -- traduções escolhidas/editadas
contexts(id, card_id, form, sentence_en, sentence_pt,
         game_name, screenshot_path, captured_at)
review_log(id, card_id, reviewed_at, rating, elapsed_days, state_before, state_after)
settings(key, value)
```

Decisões: card por **lema** (contextos múltiplos anexados); `review_log` completo permite re-otimizar parâmetros FSRS depois; screenshots como arquivos `.webp` em `media/` com path no banco (não blob).

## Estrutura do repositório

```
papaplay/
├── .claude/                  # ver doc 06
├── docs/                     # esta documentação
├── src/                      # React (UI das duas janelas)
│   ├── overlay/              # janela overlay
│   ├── main/                 # janela principal (review, deck, stats, settings)
│   ├── shared/               # componentes, hooks, tipos, ts-fsrs wrapper
│   └── lib/
├── src-tauri/
│   ├── src/
│   │   ├── capture.rs  ocr.rs  dict.rs  translate.rs  deck.rs  hotkeys.rs
│   │   └── main.rs
│   ├── resources/            # modelos ONNX, dict.db, modelos NMT
│   └── tauri.conf.json
├── scripts/                  # build do dicionário (wiktextract → SQLite), download de modelos
└── tests/
```

## Requisitos não-funcionais

- **Performance:** estado passivo ≈0% CPU; lookup completo <1s; RAM total alvo <400MB com modelos carregados (lazy-load do NMT).
- **Privacidade:** nenhum dado sai da máquina; sem telemetria no MVP (se adicionar depois, opt-in).
- **Segurança/anti-cheat:** captura externa de tela apenas; nunca injetar DLL/hook no processo do jogo.
- **Instalador:** NSIS/MSI via Tauri bundler; tamanho alvo <300MB com todos os modelos embarcados.
- **Licenças:** Wiktionary (CC BY-SA — exibir atribuição na tela "Sobre"), modelos OPUS-MT (CC/Apache conforme modelo), RapidOCR (Apache-2.0), ts-fsrs (MIT). Verificar cada uma antes do release.
