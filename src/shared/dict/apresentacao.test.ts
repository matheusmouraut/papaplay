import { describe, expect, it } from "vitest";

import type { DictEntry, DictSense } from "../types";
import {
  acepcoesPrincipais,
  classeEmPtBr,
  classesDoVerbete,
  frequenciaDe,
  resumoDoVerbete,
} from "./apresentacao";

function sense(pos: string, glossPt: string): DictSense {
  return { pos, glossPt, glossEn: null, examples: [] };
}

function verbete(extra: Partial<DictEntry> = {}): DictEntry {
  return {
    lemma: "run",
    ipa: "/ɹʌn/",
    senses: [sense("verb", "correr")],
    freqRank: 300,
    matchedForm: null,
    ...extra,
  };
}

describe("classeEmPtBr", () => {
  it("traduz as classes conhecidas", () => {
    expect(classeEmPtBr("noun")).toBe("subst.");
    expect(classeEmPtBr("verb")).toBe("verbo");
    expect(classeEmPtBr("prep_phrase")).toBe("expr.");
  });

  it("devolve o original quando a classe é desconhecida", () => {
    // O Wiktionary tem classes de nicho; melhor mostrar cru do que esconder.
    expect(classeEmPtBr("particle")).toBe("particle");
  });
});

describe("classesDoVerbete", () => {
  it("lista as classes na ordem e sem repetir", () => {
    const e = verbete({
      senses: [
        sense("verb", "correr"),
        sense("verb", "disparar"),
        sense("noun", "corrida"),
      ],
    });
    expect(classesDoVerbete(e)).toEqual(["verbo", "subst."]);
  });
});

describe("frequenciaDe", () => {
  it("separa as três faixas", () => {
    expect(frequenciaDe(1)).toBe("comum");
    expect(frequenciaDe(3000)).toBe("comum");
    expect(frequenciaDe(3001)).toBe("média");
    expect(frequenciaDe(20000)).toBe("média");
    expect(frequenciaDe(20001)).toBe("rara");
  });

  it("trata palavra fora da lista de frequência como rara", () => {
    expect(frequenciaDe(null)).toBe("rara");
  });
});

describe("resumoDoVerbete", () => {
  it("monta a linha do tooltip", () => {
    expect(resumoDoVerbete(verbete())).toBe("run → correr (verbo)");
  });

  it("mostra o lema, não a forma da tela", () => {
    // O usuário passou o mouse em "ran" e precisa entender que o card é de
    // "run" — é o comportamento que a spec de F3 pede.
    const e = verbete({ matchedForm: "ran" });
    expect(resumoDoVerbete(e)).toBe("run → correr (verbo)");
  });

  it("não quebra em verbete sem acepção", () => {
    expect(resumoDoVerbete(verbete({ senses: [] }))).toBe("run");
  });
});

describe("acepcoesPrincipais", () => {
  it("respeita o limite", () => {
    const e = verbete({
      senses: [
        sense("verb", "correr"),
        sense("verb", "disparar"),
        sense("noun", "corrida"),
        sense("noun", "trajeto"),
        sense("adj", "derretido"),
      ],
    });
    expect(acepcoesPrincipais(e, 4)).toHaveLength(4);
  });

  it("descarta acepção repetida", () => {
    // O merge de duas fontes (Wiktionary EN e PT) pode gerar glosa idêntica.
    const e = verbete({
      senses: [
        sense("verb", "correr"),
        sense("verb", "correr"),
        sense("noun", "corrida"),
      ],
    });
    const escolhidas = acepcoesPrincipais(e, 4);
    expect(escolhidas).toHaveLength(2);
    expect(escolhidas[1].glossPt).toBe("corrida");
  });

  it("mantém glosa igual em classes diferentes", () => {
    // "corrida" como substantivo e adjetivo são acepções distintas de verdade.
    const e = verbete({
      senses: [sense("noun", "corrida"), sense("adj", "corrida")],
    });
    expect(acepcoesPrincipais(e, 4)).toHaveLength(2);
  });
});
