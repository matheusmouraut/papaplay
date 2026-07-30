# 03 — Especificação das Funcionalidades (MVP)

## F1 — Overlay transparente

**Descrição:** janela sem borda, transparente, sempre-no-topo, cobrindo o monitor do jogo. Dois estados:

- **Passivo (padrão):** invisível e click-through — todo input vai para o jogo. Consumo de CPU ~0.
- **Lookup (via hotkey):** captura input de mouse, desenha destaques sobre palavras OCR-izadas. O jogo continua renderizando atrás (não pausamos nada).

**Requisitos:**

- Hotkey global configurável (padrão `Alt+X`), registrada mesmo com o jogo em foco.
- `Esc` sempre sai do modo lookup.
- Suporte a multi-monitor: overlay no monitor onde o jogo está (detecção da janela em foco no momento da hotkey).
- Compatível com janela/borderless. Fullscreen exclusivo: detectar e exibir aviso com instrução.
- Indicador visual discreto (ex.: borda sutil ou ícone no canto) mostrando que o modo lookup está ativo.

**Critérios de aceite:** ativar/desativar em <150ms; nenhum frame drop perceptível no jogo em estado passivo; nenhuma injeção no processo do jogo.

## F2 — Captura e OCR

**Descrição:** ao entrar em modo lookup, capturar o frame do monitor e extrair palavras com bounding boxes.

**Pipeline:**

1. Screenshot do monitor ativo (Windows Graphics Capture API — rápido e compatível com jogos).
2. Pré-processamento leve (grayscale; upscale 2× se a fonte for pequena).
3. OCR com saída palavra-a-palavra: `[{texto, bbox, confiança}]` + agrupamento em linhas/frases pela geometria.
4. Filtragem: descartar confiança <60%, números puros, strings de 1 caractere.
5. Renderizar destaques nas bboxes (sublinhado sutil; palavras já salvas no deck ganham cor diferente).

**Engine:** RapidOCR (modelos PaddleOCR em ONNX Runtime — rápido em CPU, ótimo em texto latino, embarcável). Fallback/experimento: Windows OCR nativo (zero peso extra). Decisão final em `04-arquitetura.md`.

**Critérios de aceite:** OCR completo de um frame 1080p em <500ms em CPU média; ≥90% de acerto em fontes regulares de UI de jogos; re-OCR manual disponível (hotkey de refresh) se a tela mudar.

## F3 — Lookup de palavra

**Descrição:** interação núcleo do produto.

**Comportamento:**

- **Hover (300ms):** tooltip mínimo — `palavra → tradução mais frequente (classe)`. Ex.: `dread → pavor, temer (subst./verbo)`.
- **Clique:** popup expandido ancorado à palavra:
  - Palavra + IPA (se disponível no dicionário) + lema (ex.: clicou em "ran" → mostra "run").
  - Até 4 acepções ordenadas por relevância, com classe gramatical.
  - **Frase de contexto** (linha OCR-izada onde a palavra está) + tradução da frase via motor NMT offline.
  - Badge de frequência da palavra (comum/média/rara — via lista de frequência) para o usuário calibrar se vale salvar.
  - Botão ⭐ **Salvar no deck** (ou "salvo ✓" se já existe, com atalho para o card).
- Lematização obrigatória: "running", "ran" → card de "run" (guardando a forma encontrada no contexto).
- Multi-palavra: seleção por arrastar sobre 2+ palavras → traduz a expressão inteira (pega phrasal verbs: "give up").

**Fontes de dados:**

- Dicionário EN→PT offline construído do Wiktionary (via wikt­extract/kaikki.org) — palavra, classe, acepções, IPA.
- Lista de frequência (ex.: wordfreq) para ordenar acepções e mostrar badge.
- NMT offline (Bergamot/Marian EN→PT) para frases completas.

**Critérios de aceite:** tooltip em <300ms após hover; popup completo (com tradução da frase) em <1s; funciona sem internet.

## F4 — Deck e cards

**Descrição:** cada palavra salva vira um card rico.

**Card contém:** lema; forma encontrada; tradução(ões) escolhida(s); frase de contexto original + tradução; screenshot recortado (região da frase, ~texto + margem); fonte (nome do jogo — via título da janela/processo detectado, editável); data de captura; estado FSRS.

**Gestão:**

- Lista de cards com busca, filtro por jogo/data/estado, ordenação.
- Editar tradução e contexto; excluir; marcar como "já sei" (suspende).
- Deck único no MVP com filtro por jogo (múltiplos decks só se ficar trivial).
- Detecção de duplicata: salvar palavra já existente adiciona novo contexto ao card em vez de duplicar.
- Exportar CSV (obrigatório) e Anki .apkg (desejável).

## F5 — Revisão (SRS)

**Descrição:** repetição espaçada com **FSRS** (biblioteca `ts-fsrs`), o algoritmo estado-da-arte usado pelo Anki moderno.

**Sessão de revisão:**

- Fila do dia = cards novos (limite configurável, padrão 15/dia) + revisões vencidas.
- Formato do card na revisão: frente = frase de contexto com a palavra **destacada** + screenshot; usuário tenta lembrar o significado; verso = traduções. (Variante inversa PT→EN no backlog.)
- Botões Again/Hard/Good/Easy → `ts-fsrs` calcula próximo agendamento; parâmetros FSRS otimizáveis no futuro com o histórico.
- Atalhos de teclado (espaço = revelar; 1–4 = avaliar).
- Resumo pós-sessão: cards revisados, taxa de acerto, streak atual.

**Critérios de aceite:** agendamento persistido corretamente entre sessões; log completo de revisões (para re-otimizar FSRS depois); revisão funciona 100% offline.

## F6 — App principal e configurações

- Janela principal (não-overlay): abas **Revisar**, **Deck**, **Estatísticas**, **Configurações**.
- Estatísticas MVP: cards criados/revisados por dia (gráfico simples), taxa de acerto, streak, palavras por jogo.
- Configurações: hotkeys; aparência do overlay (cor/opacidade dos destaques); limite de cards novos/dia; notificação diária on/off; idioma da UI (PT-BR no MVP); pasta do banco de dados; exportar/importar backup.
- Roda no system tray; iniciar com o Windows (opcional).

## F7 — Onboarding

- Wizard de 3 passos na primeira execução: (1) configurar hotkey, (2) playground embutido — uma imagem de tela de jogo dentro do app para treinar o lookup sem abrir jogo, (3) explicação do loop de revisão.
- Meta: primeiro lookup bem-sucedido em <2 minutos após instalar.
