# PapaPlay — Documentação do Projeto

> Nome provisório. Alternativas sugeridas em `01-visao-produto.md`.

Aprenda inglês jogando: um overlay para PC que traduz palavras na tela com hover/clique, salva vocabulário em decks e agenda revisões com repetição espaçada.

## Índice

| Doc                                                | Conteúdo                                                                |
| -------------------------------------------------- | ----------------------------------------------------------------------- |
| [01-visao-produto.md](01-visao-produto.md)         | Visão, problema, personas, concorrentes, diferenciais                   |
| [02-escopo-mvp.md](02-escopo-mvp.md)               | O que entra e o que fica de fora do MVP, user stories, fluxos           |
| [03-funcionalidades.md](03-funcionalidades.md)     | Especificação detalhada de cada funcionalidade do MVP                   |
| [04-arquitetura.md](04-arquitetura.md)             | Stack, componentes, pipeline OCR, modelo de dados                       |
| [05-roadmap.md](05-roadmap.md)                     | Fases pós-MVP: séries, gamificação, IA, multi-idioma                    |
| [06-plano-claude-code.md](06-plano-claude-code.md) | Estrutura `.claude/`, CLAUDE.md, skills e agents para o desenvolvimento |
| [07-diferenciais.md](07-diferenciais.md)           | 30 features de diferenciação vs Lookupper e o mercado, com priorização  |

## Resumo em 30 segundos

- **O quê:** app desktop (Windows primeiro) que fica por cima de qualquer jogo como overlay transparente. O jogador ativa o "modo lookup", passa o mouse ou clica numa palavra na tela, vê a tradução EN→PT-BR instantânea com a frase de contexto, e salva num deck com um clique.
- **Depois:** as palavras salvas viram cards revisados com o algoritmo FSRS (o mesmo estado da arte do Anki moderno) dentro do próprio app.
- **Como:** OCR local + dicionário offline + tradução neural offline. Zero custo por uso, funciona sem internet, privacidade total.
- **MVP:** jogos no PC, inglês→português. Séries/filmes, gamificação e IA vêm no roadmap.
