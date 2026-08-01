-- Schema inicial dos dados do usuario (docs/04-arquitetura.md).
--
-- O dicionario NAO mora aqui: e um banco separado, read-only, embarcado em
-- resources/dict.db. Este e o banco que o usuario acumula e que precisa
-- sobreviver a atualizacoes do app.
--
-- Datas sao TEXT em ISO-8601 UTC, nao inteiros: o agendamento vem do ts-fsrs na
-- UI, que fala em `Date`, e comparacao lexicografica de ISO-8601 ja da a ordem
-- cronologica que as consultas de "vencidos" precisam.

-- Um card por lema (decisao fechada no CLAUDE.md): "ran" e "running" caem no
-- card de "run", cada ocorrencia virando um contexto.
CREATE TABLE IF NOT EXISTS cards (
    id            INTEGER PRIMARY KEY,
    lemma         TEXT    NOT NULL UNIQUE,
    created_at    TEXT    NOT NULL,
    -- "ja sei": sai da fila sem perder o historico.
    suspended     INTEGER NOT NULL DEFAULT 0,

    -- Campos do ts-fsrs. Regra inviolavel #4: so o wrapper em src/shared/srs/
    -- produz estes valores; o core Rust os persiste sem calcular nada.
    fsrs_due             TEXT    NOT NULL,
    fsrs_stability       REAL    NOT NULL,
    fsrs_difficulty      REAL    NOT NULL,
    fsrs_state           TEXT    NOT NULL,
    fsrs_reps            INTEGER NOT NULL,
    fsrs_lapses          INTEGER NOT NULL,
    fsrs_scheduled_days  INTEGER NOT NULL DEFAULT 0,
    fsrs_learning_steps  INTEGER NOT NULL DEFAULT 0,
    fsrs_last_review     TEXT
);

-- A fila do dia le por `fsrs_due` filtrando os suspensos — e a consulta mais
-- frequente do app depois do lookup.
CREATE INDEX IF NOT EXISTS idx_cards_due ON cards (suspended, fsrs_due);

-- Traducoes escolhidas/editadas para o verso do card. Vazio significa "usa o
-- que o dicionario disser", que e o caso comum: so quem edita gera linha aqui.
CREATE TABLE IF NOT EXISTS card_senses (
    id         INTEGER PRIMARY KEY,
    card_id    INTEGER NOT NULL REFERENCES cards (id) ON DELETE CASCADE,
    sense_text TEXT    NOT NULL,
    chosen     INTEGER NOT NULL DEFAULT 1
);

CREATE INDEX IF NOT EXISTS idx_card_senses_card ON card_senses (card_id);

-- Cada vez que a palavra apareceu num jogo. E o que distingue este app de uma
-- lista de vocabulario: a frase e a tela onde ela foi encontrada.
CREATE TABLE IF NOT EXISTS contexts (
    id              INTEGER PRIMARY KEY,
    card_id         INTEGER NOT NULL REFERENCES cards (id) ON DELETE CASCADE,
    -- A forma como estava na tela ("ran"), nao o lema.
    form            TEXT    NOT NULL,
    sentence_en     TEXT    NOT NULL,
    sentence_pt     TEXT,
    game_name       TEXT,
    -- Caminho de um .webp em media/, nunca o blob: banco pequeno faz backup e
    -- migracao caberem em segundos (docs/04).
    screenshot_path TEXT,
    captured_at     TEXT    NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_contexts_card ON contexts (card_id);

-- Salvar a mesma frase duas vezes e engano de clique, nao contexto novo.
CREATE UNIQUE INDEX IF NOT EXISTS idx_contexts_unicos
    ON contexts (card_id, sentence_en, form);

-- Historico completo das revisoes. Existe para poder re-otimizar os parametros
-- do FSRS depois (docs/04) — por isso guarda o estado antes e depois, nao so a
-- nota.
CREATE TABLE IF NOT EXISTS review_log (
    id           INTEGER PRIMARY KEY,
    card_id      INTEGER NOT NULL REFERENCES cards (id) ON DELETE CASCADE,
    reviewed_at  TEXT    NOT NULL,
    rating       INTEGER NOT NULL,
    elapsed_days REAL    NOT NULL,
    state_before TEXT    NOT NULL,
    state_after  TEXT    NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_review_log_card ON review_log (card_id, reviewed_at);

CREATE TABLE IF NOT EXISTS settings (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
);
