# 05 — Roadmap

## Fase 0 — Spike técnica (1–2 semanas)

Provar os 2 riscos antes de construir o produto:

1. **Overlay:** Tauri 2 com janela transparente always-on-top sobre um jogo real (borderless), alternando click-through ↔ interativo via hotkey global, sem frame drops.
2. **OCR:** RapidOCR/ONNX em Rust extraindo palavras + bboxes de screenshots reais de 3 jogos diferentes (UI limpa, fonte estilizada, texto sobre fundo animado), medindo latência e acurácia.

**Gate:** se ambos funcionam, seguir. Se OCR falhar em fontes de jogo, avaliar OneOCR/Windows OCR ou revisar abordagem.

## Fase 1 — MVP (6–10 semanas de trabalho)

Tudo do doc 02/03. Ordem sugerida de construção:

1. Esqueleto Tauri: 2 janelas + tray + hotkeys + settings.
2. Pipeline captura → OCR → destaques no overlay.
3. Dicionário: script de build (wiktextract → SQLite) + lookup com lematização + tooltip/popup.
4. Tradução de frases offline (Marian/Bergamot).
5. Deck: salvar card com contexto + screenshot; telas de gestão.
6. Revisão FSRS + estatísticas + streak.
7. Onboarding + instalador + polish.

**Release critério:** você usando diariamente por 2 semanas sem abandonar por fricção.

## Fase 2 — Séries, vídeos e navegador

- **Extensão de navegador** (ou integração no app) estilo Language Reactor: legendas interativas em Netflix/YouTube com o **mesmo deck e mesmo SRS** — o diferencial contra concorrentes mono-cenário.
- Modo overlay sobre players de vídeo locais/desktop (mesmo pipeline OCR do MVP já cobre players sem API).
- **Modo "legendar tudo":** tradução contínua de diálogos do jogo (OCR em intervalo + overlay de tradução), além do lookup pontual.
- TTS para pronúncia nos cards (ex.: Piper TTS, local).
- Export .apkg completo se ainda não feito.

## Fase 3 — Gamificação e IA

**Gamificação (motivação):**

- XP por captura/revisão; níveis; conquistas ("100 palavras de RPGs", "streak 30 dias").
- Metas semanais e relatório de progresso.
- Quizzes variados na revisão (múltipla escolha, digitar a palavra, completar a frase) — inspiração: os 8 modos do Playto.
- "Boss battle" semanal: quiz com as palavras mais difíceis (mais lapses) da semana.

**IA (camada premium opcional — mantém núcleo offline/grátis):**

- Explicação contextual: por que "run" significa outra coisa nessa frase; gírias, ironia, referências culturais.
- Cards inteligentes: exemplos adicionais gerados, mnemônicos, mini-histórias com as palavras do deck.
- Tutor conversacional: conversar (texto/voz) sobre o jogo/episódio usando o vocabulário capturado.
- Detecção de nível: sugerir o que salvar com base no que o usuário já domina.

## Fase 4 — Plataforma

- Multi-idioma (ES, JP — JP exige OCR/segmentação específicos, é um projeto próprio).
- Conta + sync entre máquinas (e app mobile de revisão — revisar na fila do ônibus o que capturou jogando).
- Decks compartilhados por jogo ("vocabulário essencial de Hollow Knight").
- macOS/Linux.
- Monetização: núcleo grátis; premium = recursos de IA + sync + mobile. Sem anúncios.

## Backlog / ideias registradas

- Text hooking (Textractor/Agent) para visual novels — texto perfeito sem OCR.
- Integração AnkiConnect para quem já usa Anki.
- Modo leitura para PDFs/ebooks/artigos no app.
- Estatísticas de imersão: tempo jogado em inglês por semana.
- Palavras "fantasma": marcar palavra como conhecida sem criar card (treina o filtro do que destacar).
- Suporte a fullscreen exclusivo (captura via duplicação de display).
