# 07 — Diferenciais vs Lookupper (e o mercado)

## Leitura do Lookupper

O [Lookupper](https://lookupper.com/) faz muito bem o **lookup**: hover + Alt+Q, dicionário embutido, popup com definição + tradução da frase, 29 idiomas, offline, leve, sem gravação de tela. Publicado na Microsoft Store, 4.6★.

**O que ele é:** um dicionário de tela. **O que ele não é:** um sistema de aprendizado. O slogan dele ("language learning tool for procrastinators") entrega: a palavra é traduzida e esquecida. Não há loop captura → revisão → domínio.

**Nossa tese de diferenciação:** o lookup é a _porta de entrada_, não o produto. O produto é a **memória** — do usuário e do app.

## Diferenciais por área

### A. O loop de aprendizado (gap principal do Lookupper)

1. **Deck automático rico** — cada palavra salva carrega frase de contexto, tradução da frase, screenshot do momento e o jogo de origem. Card nasce pronto, zero digitação.
2. **Revisão FSRS de verdade** — agendamento estado-da-arte (mesmo do Anki moderno), fila diária, estatísticas de retenção. Lookupper não tem revisão.
3. **Quizzes variados na revisão** — flashcard, múltipla escolha, completar a frase do _seu_ jogo, digitar a palavra, ouvir e reconhecer (TTS).
4. **Multi-contexto por palavra** — reencontrou "dread" em outro jogo? O novo contexto anexa ao mesmo card. A palavra ganha uma "biografia" na sua jornada.

### B. Memória do app: ele sabe o que você sabe (ninguém faz isso)

5. **Highlight de conhecimento** — no modo lookup, palavras já dominadas aparecem sem destaque, palavras em aprendizado em amarelo, desconhecidas em azul. A tela do jogo vira um mapa visual do seu progresso.
6. **Reforço passivo** — reencontrar no jogo uma palavra em aprendizado conta como micro-revisão (registra "encontro selvagem" no FSRS-log; encontrá-la naturalmente adia a revisão agendada).
7. **Histórico de lookups não salvos** — olhou a mesma palavra 3+ vezes em dias diferentes? O app sugere: "você já buscou 'ledge' 4 vezes — salvar no deck?". Captura o vocabulário que o usuário não teve disciplina de salvar.
8. **Perfil de vocabulário** — estimativa do tamanho do vocabulário e nível aproximado (A2/B1/B2...) calculada do histórico de lookups, acertos e frequência das palavras. Gráfico de evolução mensal.
9. **Tradução seletiva por nível** — no futuro modo "legendar tela": traduzir só o que o usuário provavelmente NÃO sabe, mantendo o resto em inglês. Imersão calibrada — nem muleta total, nem afogamento.

### C. Inteligência por jogo

10. **Perfil por jogo** — palavras encontradas/salvas/dominadas por jogo; "Hollow Knight: 87 palavras, 62 dominadas".
11. **Warm-up pré-jogo** — ao detectar o jogo abrindo, oferecer revisão de 2 min das palavras daquele jogo que estão vencendo. Prepara o cérebro para a sessão.
12. **Resumo pós-sessão** — fechou o jogo: "sessão de 1h30, 14 palavras novas, 3 reencontros. Revisar agora (5 min)?".
13. **Glossários comunitários por jogo** (fase de plataforma) — "vocabulário essencial de Baldur's Gate 3" criado pela comunidade; baixar antes de jogar. Também é motor de SEO/aquisição.
14. **Dificuldade linguística dos jogos** — ranking crowdsourced: densidade de vocabulário raro por jogo ("Disco Elysium: C1; Stardew Valley: A2"). Vira conteúdo compartilhável e guia de "próximo jogo pro meu nível".

### D. Hábito e motivação (Lookupper não tenta)

15. **Streak + metas semanais** com relatório de progresso.
16. **Boss battle semanal** — quiz com as palavras mais erradas da semana; vencer "fecha" a semana.
17. **Conquistas temáticas** — "100 palavras de RPG", "primeira palavra de terror", "30 dias de streak".
18. **Revisão mobile/complementar** (fase de plataforma) — capturou jogando à noite, revisa na fila do ônibus. Sync do deck.

### E. Além dos jogos — mesmo deck, várias fontes

19. **Séries/YouTube via extensão** (Fase 2) — legendas interativas alimentando o MESMO deck e o MESMO SRS. Lookupper e Language Reactor são mono-cenário; nossa unidade "um vocabulário, todas as fontes" é única.
20. **Modo leitura** — PDFs, ebooks e artigos dentro do app com o mesmo lookup.
21. **Estatística de imersão total** — horas de exposição ao inglês por semana somando jogo + vídeo + leitura.

### F. Qualidade de lookup acima do Lookupper

22. **Lematização + formas irregulares** — clicou "ran", card de "run" (guardando a forma vista).
23. **Phrasal verbs e expressões** — seleção multi-palavra por arrastar; detecção automática de phrasal verbs conhecidos na frase ("gave up" → destaque conjunto).
24. **Acepções ordenadas pela frequência no contexto de jogos** — corpus próprio de legendas/jogos para ordenar significados como aparecem em games, não como no dicionário formal.
25. **Badge de frequência** — "palavra rara" vs "top 1000": ajuda a decidir o que vale salvar.
26. **PT-BR de verdade** — traduções naturais brasileiras, notas de falsos cognatos ("actually ≠ atualmente"), gírias de internet/games.

### G. IA como camada premium (Fase 3 — mantém núcleo offline)

27. **"Por que essa tradução?"** — explicação contextual de ironia, gíria, referência cultural na frase capturada.
28. **Mnemônicos e mini-histórias** gerados com as palavras do seu deck.
29. **Tutor pós-sessão** — conversar (texto/voz) sobre o que aconteceu no jogo usando o vocabulário capturado. Produção ativa, não só reconhecimento.
30. **Card enrichment** — exemplos extras, sinônimos, collocations gerados 1× e salvos no card (custo único, uso offline depois).

## Priorização dos diferenciais no MVP

Já no MVP (baratos e de alto impacto): **1, 2, 4, 7, 22, 23, 25, 26** + streak simples (15).
Logo após (v1.x): **5, 6, 10, 12** — são o fosso competitivo real, pois dependem do histórico que só nós acumulamos.
Fases 2–4: o restante, conforme roadmap.

> Atualização da tabela de concorrentes do doc 01: adicionar Lookupper como referência de UX de lookup; gap = sem SRS/deck rico/memória de usuário.
