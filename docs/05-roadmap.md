# 05 — Roadmap

## Fase 0 — Spike técnica (1–2 semanas)

Provar os 2 riscos antes de construir o produto:

1. **Overlay:** Tauri 2 com janela transparente always-on-top sobre um jogo real (borderless), alternando click-through ↔ interativo via hotkey global, sem frame drops.
2. **OCR:** RapidOCR/ONNX em Rust extraindo palavras + bboxes de screenshots reais de 3 jogos diferentes (UI limpa, fonte estilizada, texto sobre fundo animado), medindo latência e acurácia.

**Gate:** se ambos funcionam, seguir. Se OCR falhar em fontes de jogo, avaliar OneOCR/Windows OCR ou revisar abordagem.

## Fase 1 — MVP (6–10 semanas de trabalho)

Tudo do doc 02/03. Ordem sugerida de construção:

1. ✅ Esqueleto Tauri: 2 janelas + hotkeys + settings. _(o **tray** nunca foi feito: sem ele, fechar a janela principal deixa o processo vivo por causa da overlay, e não há como sair nem reabrir a janela — ver item 11)_
2. ✅ Pipeline captura → OCR → posicionamento no overlay.
3. ✅ Dicionário: script de build (wiktextract → SQLite) + lookup com lematização + tooltip/popup.
4. ✅ Tradução de frases offline (Marian/Bergamot).
5. ✅ Deck: salvar card com contexto + screenshot; telas de gestão. _(falta export CSV)_
6. ✅ **Interação "espiar" (F1 reescrita)** — segurar `Alt+X` para espiar, soltar para sumir; clique abre o card; overlay nunca rouba o foco; sem caixas em volta das palavras. Substitui o modo lookup persistente. _(validado no app em 2026-08-01)_
7. ✅ **Qualidade da leitura** — frase de contexto atravessando várias linhas; leitura acompanhando o cursor; modelos quentes (OCR pré-carregado, NMT com descarte por ociosidade em vez de a cada uso).
8. **Linguagem visual (F7)** — sistema de design minimalista aplicado às duas janelas.
9. **Atalhos configuráveis (F6)** — uma combinação por ação, editável e persistida; hoje `Alt+X`/`Alt+C` estão fixos no código. Tabela de ações e requisitos na F6 do doc 03.
10. Revisão FSRS + estatísticas + streak.
11. Onboarding + instalador + polish — inclui o **tray** que ficou faltando no item 1 (ícone com "Abrir"/"Sair", fechar a janela principal esconde em vez de matar) e o `bundle.resources` do `tauri.conf.json`, hoje vazio: o app só acha dict.db/modelos/nmt (390 MB) porque cai no caminho absoluto do repo compilado junto — em outra máquina não acharia nada.

**Por que 6–8 vêm antes da revisão:** o teste com o app rodando mostrou que o loop de captura funciona mas atrapalha o jogo — e uma ferramenta que quebra a imersão não é usada diariamente, que é justamente o critério de release abaixo. Corrigir a interação vale mais do que empilhar features em cima dela.

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

## Ideias que nasceram do primeiro teste com o app rodando

Ordenadas por quanto protegem a imersão — o critério que o teste mostrou ser o que decide se o app é usado:

- **Salvar sem abrir nada:** espiando, com a palavra sob o cursor, uma tecla (ex.: `S`) salva direto no deck e mostra só um toast de 1s. Salvar sem parar de jogar é o caminho mais curto entre "vi a palavra" e "vou revisar depois".
- **Fila de revisita:** as palavras espiadas na sessão ficam numa lista na janela principal ("você olhou 12 palavras hoje, 3 viraram card") — decidir o que salvar depois, fora do jogo, sem gastar atenção durante.
- **Cache da tela lida:** espiar duas vezes a mesma caixa de diálogo não deveria pagar OCR de novo. Cache por região + hash do recorte, invalidado quando o conteúdo muda.
- **Marca discreta em palavra já salva:** enquanto espia, um ponto sutil sob palavras que já estão no deck — sem pintar a tela.
- **Espiar com o teclado:** navegar entre as palavras da frase com as setas enquanto segura o atalho, para quem joga com as duas mãos no teclado.
- **Perfis por jogo:** lembrar o atalho, a região e a opacidade por processo — o que funciona num RPG de texto não é o que funciona num FPS.
- **Modo leitura sem jogo:** a mesma espiada por cima do navegador/PDF já funciona hoje sem código novo; documentar como recurso em vez de deixar como efeito colateral.

## Backlog / ideias registradas

- Text hooking (Textractor/Agent) para visual novels — texto perfeito sem OCR.
- Integração AnkiConnect para quem já usa Anki.
- Modo leitura para PDFs/ebooks/artigos no app.
- Estatísticas de imersão: tempo jogado em inglês por semana.
- Palavras "fantasma": marcar palavra como conhecida sem criar card (treina o filtro do que destacar).
- Suporte a fullscreen exclusivo (captura via duplicação de display).
