import { describe, expect, it } from "vitest";

import { RATINGS, Rating, State, newCard, preview, review } from "./index";

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
