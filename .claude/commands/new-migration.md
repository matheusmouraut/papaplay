---
description: Cria uma migration SQLite numerada + teste
---

Argumento: descrição curta da mudança. $ARGUMENTS

1. Veja a última migration em `src-tauri/migrations/` (formato `NNNN_descricao.sql`).
2. Crie a próxima com número sequencial. Schema de referência: docs/04-arquitetura.md.
3. Migrations são só-ida (sem down) e idempotentes quando possível.
4. Atualize o runner de migrations no core Rust se necessário.
5. Adicione teste que aplica todas as migrations num banco em memória e valida o schema final.
6. Se a mudança tocar campos fsrs_*, confirme que o wrapper em `src/shared/srs/` continua o único ponto de escrita.
