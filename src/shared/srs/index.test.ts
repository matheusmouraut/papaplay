import { describe, expect, it } from "vitest";

import type { DeckCard } from "../types";
import {
  RATINGS,
  Rating,
  State,
  fromFsrsCard,
  newCard,
  newCardFields,
  preview,
  review,
  toFsrsCard,
} from "./index";

describe("wrapper do FSRS", () => {
  it("cria card novo com due igual a agora", () => {
    const now = new Date("2026-01-01T12:00:00Z");
    const card = newCard(now);

    expect(card.state).toBe(State.New);
    expect(card.reps).toBe(0);
    expect(card.due.getTime()).toBe(now.getTime());
  });

  it("agenda mais longe para Facil do que para Errei", () => {
    const now = new Date("2026-01-01T12:00:00Z");
    const card = newCard(now);

    const again = review(card, Rating.Again, now).card;
    const easy = review(card, Rating.Easy, now).card;

    expect(easy.due.getTime()).toBeGreaterThan(again.due.getTime());
  });

  it("expoe previa das quatro notas", () => {
    const now = new Date("2026-01-01T12:00:00Z");
    const log = preview(newCard(now), now);

    for (const rating of RATINGS) {
      expect(log[rating].card.due).toBeInstanceOf(Date);
    }
  });
});

describe("ponte com o banco", () => {
  const now = new Date("2026-01-01T12:00:00Z");

  function cardPersistido(): DeckCard {
    const campos = newCardFields(now);
    return {
      id: 1,
      lemma: "run",
      createdAt: now.toISOString(),
      suspended: false,
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

  it("monta o estado de um card novo no formato que o core grava", () => {
    const campos = newCardFields(now);

    expect(campos.state).toBe("new");
    expect(campos.reps).toBe(0);
    expect(campos.due).toBe(now.toISOString());
    // O core so grava; se algum campo virar undefined aqui, ele chega ao SQLite
    // como NULL numa coluna NOT NULL.
    for (const valor of Object.values(campos)) {
      expect(valor).toBeDefined();
    }
  });

  it("preserva o agendamento na ida e volta pelo banco", () => {
    // O caminho real de uma revisao: le do banco, aplica a nota, grava de volta.
    const revisado = review(
      toFsrsCard(cardPersistido()),
      Rating.Good,
      now,
    ).card;
    const gravado = fromFsrsCard(cardPersistido(), revisado);
    const relido = toFsrsCard(gravado);

    expect(relido.due.getTime()).toBe(revisado.due.getTime());
    expect(relido.stability).toBe(revisado.stability);
    expect(relido.reps).toBe(revisado.reps);
    expect(relido.state).toBe(revisado.state);
    // Os tres campos que so passaram a existir com a migration 0001: sem eles
    // o agendamento de curto prazo era reconstruido zerado a cada leitura.
    expect(relido.scheduled_days).toBe(revisado.scheduled_days);
    expect(relido.learning_steps).toBe(revisado.learning_steps);
    expect(relido.last_review?.getTime()).toBe(revisado.last_review?.getTime());
  });
});
