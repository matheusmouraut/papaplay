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

import type { DeckCard, FsrsFields, FsrsState } from "../types";

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

/**
 * Estado inicial de uma palavra salva no overlay, pronto para o `deck_save_card`.
 *
 * Existe para que o core nunca precise inventar um estado zerado por conta
 * propria: "card novo" e uma definicao do FSRS, e ela mora aqui.
 */
export function newCardFields(now: Date = new Date()): FsrsFields {
  return toFsrsFields(createEmptyCard(now));
}

/** Converte um card do ts-fsrs para o formato que o core persiste. */
export function toFsrsFields(card: FsrsCard): FsrsFields {
  return {
    due: card.due.toISOString(),
    stability: card.stability,
    difficulty: card.difficulty,
    state: STATE_TO_STRING[card.state],
    reps: card.reps,
    lapses: card.lapses,
    scheduledDays: card.scheduled_days,
    learningSteps: card.learning_steps,
    lastReview: card.last_review ? card.last_review.toISOString() : null,
  };
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
 * `elapsed_days` nao e persistido de proposito: e a distancia entre a ultima
 * revisao e agora, ou seja, muda sozinho com o tempo. O proprio ts-fsrs o
 * recalcula a cada `repeat`/`next`, entao guardar seria guardar uma copia que
 * envelhece.
 */
export function toFsrsCard(card: DeckCard): FsrsCard {
  return {
    due: new Date(card.fsrsDue),
    stability: card.fsrsStability,
    difficulty: card.fsrsDifficulty,
    elapsed_days: 0,
    scheduled_days: card.fsrsScheduledDays,
    learning_steps: card.fsrsLearningSteps,
    reps: card.fsrsReps,
    lapses: card.fsrsLapses,
    state: STRING_TO_STATE[card.fsrsState],
    last_review: card.fsrsLastReview
      ? new Date(card.fsrsLastReview)
      : undefined,
  };
}

/** Aplica um card do ts-fsrs de volta sobre o registro persistido. */
export function fromFsrsCard(card: DeckCard, next: FsrsCard): DeckCard {
  const campos = toFsrsFields(next);
  return {
    ...card,
    fsrsDue: campos.due,
    fsrsStability: campos.stability,
    fsrsDifficulty: campos.difficulty,
    fsrsState: campos.state,
    fsrsReps: campos.reps,
    fsrsLapses: campos.lapses,
    fsrsScheduledDays: campos.scheduledDays,
    fsrsLearningSteps: campos.learningSteps,
    fsrsLastReview: campos.lastReview,
  };
}
