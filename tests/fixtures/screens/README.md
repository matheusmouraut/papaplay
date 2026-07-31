# Fixtures de OCR

Screenshots reais de jogos usados pelo `/screen-test` e pela spike 02.

- `<nome>.png|jpg` — screenshot do jogo
- `<nome>.expected.json` — gabarito: `{ "kind": "...", "notes": "...", "words": [...] }`
- `<nome>.boxes.png` — saída visual do `ocr-spike --desenhar`; **fora do git**, regerável

## O gabarito

`words` lista os termos que o OCR **deveria** ler. Só palavras de conteúdo: números de HUD,
contadores e glifos de ícone ficam de fora, porque não é isso que vai virar card.

A comparação passa os dois lados pelo mesmo tokenizador (minúsculas, quebra em não-alfanumérico,
apóstrofo conta como letra). Por isso `Keyboard/Mouse` lido pelo OCR casa com `Keyboard` e `Mouse`
no gabarito, e `didn't` conta como uma palavra só.

`kind` classifica a dificuldade: `dialogo`, `menu`, `fundo-complexo`, `fonte-pequena`.

## Conjunto atual

10 telas de *The Seven Deadly Sins: Origin* a 2560×1440, capturadas em 2026-07-30:

| Arquivo  | kind           | O que testa                                                    |
| -------- | -------------- | -------------------------------------------------------------- |
| `215520` | dialogo        | fala de uma linha, fonte serifada sobre faixa translúcida       |
| `215603` | dialogo        | fala de duas linhas, contrações (`didn't`, `you're`)            |
| `215538` | fundo-complexo | texto solto sobre grama animada, sem caixa atrás                |
| `215622` | menu           | menu principal, rótulos curtos                                  |
| `215633` | menu           | registro de missões, parágrafo corrido                          |
| `215639` | menu           | quase idêntica à `215633` — testa estabilidade entre quadros    |
| `215651` | fonte-pequena  | descrição de habilidade, densa, com palavras coloridas no meio  |
| `215730` | menu           | configurações, alto contraste — é o piso da amostra             |
| `215742` | menu           | frase partida por **ícones de tecla** no meio                   |
| `215751` | menu           | mistura de fontes e nomes próprios fora de dicionário           |

Resultado da spike 02: recall agregado **96,5%**. Detalhes e por-imagem em
[`docs/spikes/spike-ocr-resultado.md`](../../../docs/spikes/spike-ocr-resultado.md).

## Ao adicionar uma fixture

Variedade vale mais que quantidade. O que ainda falta no conjunto: outro jogo (tudo aqui é do mesmo
título e da mesma família de fontes), legenda de cutscene em movimento, e uma tela a 1920×1080 —
todas as atuais são 2.5K.
