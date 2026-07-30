# 02 — Escopo do MVP

## Objetivo do MVP

Provar o loop central com um único cenário e um único par de idiomas:

**Jogar (EN) → ver palavra desconhecida → lookup instantâneo (PT-BR) → salvar no deck → revisar depois (FSRS) → voltar a jogar sabendo mais.**

Sucesso = você (usuário zero) usar por 2+ semanas em jogos reais, com lookup confiável em <1s e revisões diárias funcionando.

## Dentro do MVP ✅

| Área          | Entrega                                                                                                                     |
| ------------- | --------------------------------------------------------------------------------------------------------------------------- |
| Overlay       | Janela transparente click-through sobre qualquer jogo em janela/borderless; hotkey global para ativar/desativar modo lookup |
| Captura       | OCR local da região da tela sob demanda (ao ativar lookup), com detecção de palavras e suas posições                        |
| Lookup        | Hover/clique em palavra → popup com: tradução PT-BR, classe gramatical, frase de contexto capturada, tradução da frase      |
| Deck          | Botão "salvar" no popup → card criado com palavra, contexto, tradução, screenshot recortado e nome do jogo                  |
| Revisão       | Tela de revisão no app principal com FSRS (botões Again/Hard/Good/Easy), fila diária                                        |
| Gestão        | Listar/editar/excluir cards, busca, estatísticas básicas (cards por dia, taxa de acerto, streak)                            |
| App principal | Janela de configurações: hotkeys, opacidade, idioma UI, pasta de dados                                                      |
| Dados         | Tudo em SQLite local; exportar deck para CSV/Anki (.apkg é desejável, CSV é obrigatório)                                    |

## Fora do MVP ❌ (e onde entra)

| Item                                                                | Fase                                                             |
| ------------------------------------------------------------------- | ---------------------------------------------------------------- |
| Séries/filmes/navegador (legendas interativas)                      | Fase 2                                                           |
| Gamificação (XP, conquistas, ligas) — só streak simples no MVP      | Fase 3                                                           |
| IA/LLM (explicação de contexto, gírias, tutor)                      | Fase 3                                                           |
| Outros pares de idioma além de EN→PT-BR                             | Fase 4                                                           |
| Tradução automática contínua da tela inteira (modo "legendar tudo") | Fase 2                                                           |
| Text hooking (Textractor/Agent) para visual novels                  | Backlog                                                          |
| Áudio/pronúncia (TTS)                                               | Fase 2                                                           |
| Conta online, sync entre máquinas                                   | Fase 4                                                           |
| macOS/Linux                                                         | Backlog (arquitetura não deve impedir)                           |
| Jogos fullscreen exclusivo                                          | Backlog (documentar limitação; borderless resolve 90% dos casos) |

## User stories do MVP

1. Como jogador, quero apertar uma hotkey durante o jogo e ver as palavras da tela ficarem interativas, para traduzir sem alt-tab.
2. Como jogador, quero passar o mouse/clicar numa palavra e ver a tradução em <1s, para não perder a imersão.
3. Como jogador, quero ver a frase inteira onde a palavra apareceu traduzida, para entender o contexto (ex.: phrasal verbs).
4. Como jogador, quero salvar a palavra com um clique, para revisar depois sem interromper o jogo.
5. Como estudante, quero abrir o app e revisar meus cards pendentes do dia, para fixar o vocabulário.
6. Como estudante, quero ver de qual jogo cada palavra veio e o print do momento, para reativar a memória do contexto.
7. Como usuário, quero configurar hotkeys e aparência do overlay, para não conflitar com os controles do jogo.
8. Como usuário, quero exportar meu deck, para não ficar preso à ferramenta.

## Fluxos principais

### Fluxo 1 — Lookup durante o jogo

1. Usuário joga em modo janela/borderless. App roda no tray.
2. Aperta `Alt+X` (configurável) → overlay entra em modo lookup: captura a tela, roda OCR (~200–500ms), desenha destaques discretos sobre as palavras reconhecidas. O jogo continua visível; input de mouse vai para o overlay.
3. Hover numa palavra → tooltip compacto: **palavra • tradução principal • classe**. Clique → popup expandido: traduções, frase de contexto + tradução da frase, botão ⭐ salvar.
4. `Esc` ou `Alt+X` de novo → overlay volta a ser invisível/click-through; input volta ao jogo.

### Fluxo 2 — Revisão diária

1. Ícone do tray mostra badge com cards pendentes; notificação opcional 1×/dia.
2. Usuário abre o app → "Revisar (23)".
3. Card mostra: palavra + frase de contexto (com a palavra oculta ou destacada) → usuário responde mentalmente → revela → avalia Again/Hard/Good/Easy → FSRS agenda a próxima revisão.
4. Fim da fila → resumo da sessão (acertos, tempo, streak).

### Fluxo 3 — Primeira execução

1. Instala → wizard curto: escolhe hotkeys, testa o overlay numa tela de exemplo embutida, entende o loop (3 passos ilustrados).
2. Modelos de OCR/tradução já embarcados no instalador (sem download pós-install, se o tamanho permitir; senão, download único com barra de progresso).

## Riscos e mitigações

| Risco                                                | Impacto                       | Mitigação                                                                                                                                        |
| ---------------------------------------------------- | ----------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------ |
| OCR falha em fontes estilizadas de jogos             | Lookup não encontra a palavra | Pré-processamento (upscale, binarização); permitir selecionar região manualmente; documentar jogos problemáticos; VLM local como fallback futuro |
| Overlay não aparece sobre jogos fullscreen exclusivo | Cenário principal quebra      | Detectar e orientar usuário a usar borderless; suporte a fullscreen fica no backlog                                                              |
| Anti-cheat de jogos online sinalizar o overlay       | Banimento do usuário          | Não injetar nada no processo do jogo (captura externa de tela apenas); avisar sobre jogos competitivos com anti-cheat agressivo                  |
| Latência do OCR irritar                              | Abandono                      | OCR sob demanda (não contínuo) na região visível; cache do último frame; meta <1s                                                                |
| Tradução palavra solta ser ambígua                   | Confiança baixa               | Sempre mostrar a frase de contexto traduzida junto; dicionário com múltiplas acepções ordenadas por frequência                                   |
