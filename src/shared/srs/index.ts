/**
 * Wrapper unico do FSRS (regra inviolavel #4 do CLAUDE.md).
 *
 * Nenhum outro modulo deve importar `ts-fsrs` nem escrever nos campos
 * `fsrs*` de um card diretamente. Toda transicao de estado passa por aqui.
 */
import {
  createEmptyCard,
  fsrs,
  generatorParameters,
  Rating,
  State,
  type Card as FsrsCard,
  type FSRSParameters,
  type Grade,
  type RecordLogItem,
} from "ts-fsrs";

import type { DeckCard, FsrsState } from "../types";

export { Rating, State };
export type { FsrsCard, Grade, RecordLogItem };

/** Notas que o usuario da na revisao. Espelha `Rating` sem o `Manual`. */
export const RATINGS = [
  Rating.Again,
  Rating.Hard,
  Rating.Good,
  Rating.Easy,
] as const satisfies readonly Grade[];

/** Rotulos PT-BR dos botoes de revisao. */
export const RATING_LABELS: Record<Grade, string> = {
  [Rating.Again]: "Errei",
  [Rating.Hard]: "Dificil",
  [Rating.Good]: "Bom",
  [Rating.Easy]: "Facil",
};

const DEFAULT_PARAMS: FSRSParameters = generatorParameters({
  enable_fuzz: true,
  enable_short_term: true,
});

const scheduler = fsrs(DEFAULT_PARAMS);

const STATE_TO_STRING: Record<State, FsrsState> = {
  [State.New]: "new",
  [State.Learning]: "learning",
  [State.Review]: "review",
  [State.Relearning]: "relearning",
};

const STRING_TO_STATE: Record<FsrsState, State> = {
  new: State.New,
  learning: State.Learning,
  review: State.Review,
  relearning: State.Relearning,
};

/** Card zerado para uma palavra recem-salva no deck. */
export function newCard(now: Date = new Date()): FsrsCard {
  return createEmptyCard(now);
}

/** Previa dos 4 intervalos possiveis, para mostrar nos botoes de revisao. */
export function preview(card: FsrsCard, now: Date = new Date()) {
  return scheduler.repeat(card, now);
}

/** Aplica a nota e devolve o card novo + a entrada de `review_log`. */
export function review(
  card: FsrsCard,
  grade: Grade,
  now: Date = new Date(),
): RecordLogItem {
  return scheduler.next(card, now, grade);
}

/** Probabilidade de lembrar a palavra agora (0..1). */
export function retrievability(card: FsrsCard, now: Date = new Date()): number {
  return scheduler.get_retrievability(card, now, false);
}

/**
 * Converte o card persistido no SQLite para o formato do ts-fsrs.
 *
 * TODO(schema): `scheduled_days`, `learning_steps` e `last_review` ainda nao
 * existem em `cards` (ver docs/04). Sao reconstruidos com valores neutros ate
 * a migration que os adiciona — o agendamento de curto prazo fica aproximado.
 */
export function toFsrsCard(card: DeckCard): FsrsCard {
  return {
    due: new Date(card.fsrsDue),
    stability: card.fsrsStability,
    difficulty: card.fsrsDifficulty,
    elapsed_days: 0,
    scheduled_days: 0,
    learning_steps: 0,
    reps: card.fsrsReps,
    lapses: card.fsrsLapses,
    state: STRING_TO_STATE[card.fsrsState],
  };
}

/** Aplica um card do ts-fsrs de volta sobre o registro persistido. */
export function fromFsrsCard(card: DeckCard, next: FsrsCard): DeckCard {
  return {
    ...card,
    fsrsDue: next.due.toISOString(),
    fsrsStability: next.stability,
    fsrsDifficulty: next.difficulty,
    fsrsState: STATE_TO_STRING[next.state],
    fsrsReps: next.reps,
    fsrsLapses: next.lapses,
  };
}
