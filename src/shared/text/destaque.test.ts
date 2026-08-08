import { describe, expect, it } from "vitest";

import { destacarForma } from "./destaque";

/** Só os pedaços destacados, que é o que o teste está afirmando. */
const destacados = (frase: string, forma: string) =>
  destacarForma(frase, forma)
    .filter((p) => p.destaque)
    .map((p) => p.texto);

/** A concatenação tem que devolver a frase original, sempre. */
const recomposta = (frase: string, forma: string) =>
  destacarForma(frase, forma)
    .map((p) => p.texto)
    .join("");

describe("destacarForma", () => {
  it("destaca a palavra no meio da frase", () => {
    expect(destacados("He ran away.", "ran")).toEqual(["ran"]);
  });

  it("ignora a caixa mas preserva o texto original", () => {
    expect(destacados("Ran, he did.", "ran")).toEqual(["Ran"]);
  });

  it("nao destaca a forma dentro de outra palavra", () => {
    expect(destacados("A brand new sword.", "ran")).toEqual([]);
  });

  it("destaca todas as ocorrencias", () => {
    expect(destacados("Run, run, run.", "run")).toEqual(["Run", "run", "run"]);
  });

  it("aceita apostrofo e parenteses na forma", () => {
    expect(destacados("I don't know.", "don't")).toEqual(["don't"]);
    expect(destacados("Time to (un)do it.", "(un)do")).toEqual(["(un)do"]);
  });

  it("devolve a frase inteira quando a forma nao aparece", () => {
    const pedacos = destacarForma("A dread silence.", "run");
    expect(pedacos).toEqual([{ texto: "A dread silence.", destaque: false }]);
  });

  it("devolve a frase inteira quando a forma e vazia", () => {
    const pedacos = destacarForma("A dread silence.", "  ");
    expect(pedacos).toEqual([{ texto: "A dread silence.", destaque: false }]);
  });

  it("nunca perde nem inventa texto", () => {
    const frase = "Run! The dread lord runs after us.";
    expect(recomposta(frase, "run")).toBe(frase);
    expect(recomposta(frase, "dread")).toBe(frase);
    expect(recomposta("", "run")).toBe("");
  });
});
