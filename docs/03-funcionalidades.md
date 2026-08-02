# 03 — Especificação das Funcionalidades (MVP)

## F1 — Overlay transparente (modelo "espiar")

**Descrição:** janela sem borda, transparente, sempre-no-topo, cobrindo o monitor do jogo. **Não existe "modo ligado".** O gesto é segurar uma tecla, olhar, soltar — como quem passa o dedo numa palavra enquanto lê.

Três estados:

- **Repouso (padrão):** nada desenhado, click-through, ~0% CPU. O jogo não percebe o app.
- **Espiando (enquanto `Alt+X` está pressionado):** a overlay segue o cursor e mostra a tradução da palavra sob ele. Continua click-through: o jogo mantém o foco e todo o input. Soltar a tecla volta ao repouso.
- **Card aberto (`Alt+X` + clique esquerdo, ou `Alt+C`):** a overlay mostra o card completo ancorado na palavra, **sem** roubar o foco do jogo (`WS_EX_NOACTIVATE`). `Esc` ou clique fora fecha.

**O clique não pode acionar o jogo.** Espiando, a overlay intercepta o mouse, então o clique que abre o card não chega ao que está atrás — clicar numa palavra não atira nem seleciona nada.

Isso funciona para tudo que lê mouse por mensagem do Windows (navegador, PDF, visual novels, jogos 2D e de menu), mas **não** para jogos que leem raw input/DirectInput: esses recebem o botão pelo *foco*, e o foco é do jogo por decisão de projeto. Não há como bloquear sem roubar o foco (proibido acima) ou instalar um hook de baixo nível (risco de anti-cheat, regra 1).

Por isso `Alt+C` existe: um atalho registrado com `RegisterHotKey` é **consumido** pelo Windows antes de o jogo vê-lo — é o caminho garantido para abrir o card em qualquer jogo, e o motivo de o próprio `Alt+X` não digitar "x".

**Requisitos:**

- Hotkey global configurável (padrão `Alt+X`), registrada mesmo com o jogo em foco, reagindo a **pressionar e soltar**. Segunda hotkey (`Alt+C`) abre o card sem usar o mouse.
- O app **nunca** chama `SetForegroundWindow` para si: alt-tab forçado é o que quebra a imersão (e pisca em borderless fullscreen).
- Nada de destaque em massa: não se desenham caixas em volta de todas as palavras lidas. Só a palavra sob o cursor recebe uma marca discreta.
- Em click-through o webview não recebe mouse: a posição do cursor é lida pelo core (`GetCursorPos`) e o clique por leitura de estado (`GetAsyncKeyState`). Nunca por hook.
- Suporte a multi-monitor: overlay no monitor onde o jogo está (detecção no momento em que a espiada começa).
- Compatível com janela/borderless. Fullscreen exclusivo: detectar e exibir aviso com instrução.

**Critérios de aceite:** primeiro tooltip em <400ms depois de segurar a tecla; soltar volta ao repouso em <100ms; nenhum frame drop perceptível no jogo; o jogo nunca perde o foco; nenhuma injeção no processo do jogo.

## F2 — Captura e OCR

**Descrição:** enquanto a espiada está ativa, ler a região da tela em volta do cursor e extrair palavras com bounding boxes.

**Pipeline:**

1. Screenshot da região em volta do cursor (Windows Graphics Capture API — rápido e compatível com jogos).
2. Pré-processamento leve (grayscale; upscale 2× se a fonte for pequena).
3. OCR com saída palavra-a-palavra: `[{texto, bbox, confiança}]` + agrupamento em linhas e em **frases** (uma frase pode ocupar várias linhas na tela — é o caso comum em caixa de diálogo).
4. Filtragem: descartar confiança <60%, números puros, strings de 1 caractere.
5. A leitura **acompanha o cursor**: sair da região já lida dispara uma nova leitura. Ler uma vez só, no instante da hotkey, faria a palavra parar de responder assim que o mouse andasse.

**Engine:** RapidOCR (modelos PaddleOCR em ONNX Runtime — rápido em CPU, ótimo em texto latino, embarcável). Fallback/experimento: Windows OCR nativo (zero peso extra). Decisão final em `04-arquitetura.md`.

**Critérios de aceite:** OCR completo de um frame 1080p em <500ms em CPU média; ≥90% de acerto em fontes regulares de UI de jogos; re-OCR manual disponível (hotkey de refresh) se a tela mudar.

## F3 — Lookup de palavra

**Descrição:** interação núcleo do produto.

**Comportamento:**

- **`Alt+X` segurado + mouse sobre a palavra:** tooltip mínimo — `palavra → tradução mais frequente (classe)`. Ex.: `dread → pavor, temer (subst./verbo)`.
- **`Alt+X` segurado + clique esquerdo:** popup expandido ancorado à palavra:
  - Palavra + IPA (se disponível no dicionário) + lema (ex.: clicou em "ran" → mostra "run").
  - Até 4 acepções ordenadas por relevância, com classe gramatical.
  - **Frase de contexto** (a frase inteira onde a palavra está, mesmo que ela ocupe várias linhas na tela) + tradução da frase via motor NMT offline.
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

## F7 — Linguagem visual

**Referência:** editores de texto modernos e minimalistas (Notion, Linear, Bear) — não "HUD de jogo", não painel de ferramenta.

- **Tipografia primeiro:** o conteúdo é texto (frase, tradução, acepção). Fonte de interface neutra, tamanho generoso, entrelinha 1.5, hierarquia por peso e cor — não por caixinha.
- **Cromatismo contido:** dois tons de fundo, uma borda sutil, um acento só. Cor tem significado (acerto, erro, salvo), nunca decoração.
- **Espaço em branco é elemento de design**, não desperdício: densidade baixa, respiro entre blocos.
- **Sem cromo desnecessário:** nada de sombras pesadas, gradientes, bordas grossas ou emojis como ícone. Cantos suavemente arredondados (4–8px).
- **Movimento discreto:** transições de 120–180ms em opacidade e deslocamento pequeno; nada que chame atenção durante o jogo.
- **Na overlay, o mínimo:** o tooltip é uma linha de texto sobre um fundo escuro translúcido; o card é um cartão de leitura. Nada de moldura de "app" em cima do jogo.

**Critério de aceite:** um print da tela do Deck e um do card do overlay devem parecer o mesmo produto, e nenhum dos dois deve parecer um painel de debug.

## F8 — Onboarding

- Wizard de 3 passos na primeira execução: (1) configurar hotkey, (2) playground embutido — uma imagem de tela de jogo dentro do app para treinar o lookup sem abrir jogo, (3) explicação do loop de revisão.
- Meta: primeiro lookup bem-sucedido em <2 minutos após instalar.
