---
description: Pipeline do dicionário — wiktextract/kaikki → dict.db com validações
---

1. Rode/implemente `scripts/build-dict`: baixa o dump EN do kaikki.org (permitido apenas em build-time), extrai entradas com traduções PT, classe gramatical e IPA.
2. Gere tabela de lemas (`lemma_forms`) e junte ranks do wordfreq.
3. Escreva `src-tauri/resources/dict.db` (SQLite read-only).
4. Validações obrigatórias (falhe se violar):
   - > 40k lemas com pelo menos 1 tradução PT
   - Palavras-teste corretas: run, dread, ledge, quest, give up (phrasal), told→tell (lema)
   - Tamanho do .db < 200MB
5. Reporte: nº de lemas, nº de formas, cobertura do top-5000 do wordfreq (meta >95%).
6. Registre a versão do dump em `scripts/dict-version.txt` e a atribuição CC BY-SA na tela Sobre.
