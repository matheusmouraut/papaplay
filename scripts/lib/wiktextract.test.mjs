import { describe, expect, it } from "vitest";

import {
  acepcoesDaEntrada,
  acepcoesDoWiktionarioPt,
  distribuirTraducoes,
  ehApenasForma,
  ehEntradaUtil,
  formasDaEntrada,
  ipaDaEntrada,
} from "./wiktextract.mjs";

/** Entrada mínima no formato do kaikki.org. */
function entrada(extra = {}) {
  return { word: "run", lang_code: "en", pos: "verb", senses: [], ...extra };
}

describe("ehEntradaUtil", () => {
  it("aceita um verbete comum do inglês", () => {
    expect(
      ehEntradaUtil(entrada({ senses: [{ glosses: ["to move fast"] }] })),
    ).toBe(true);
  });

  it("recusa outros idiomas", () => {
    // O dump da língua inglesa traz entradas de outros idiomas junto.
    expect(ehEntradaUtil(entrada({ lang_code: "pt" }))).toBe(false);
  });

  it("recusa classes gramaticais que não interessam ao produto", () => {
    expect(ehEntradaUtil(entrada({ pos: "character" }))).toBe(false);
  });

  it("recusa entradas que existem só para apontar para o lema", () => {
    const forma = entrada({
      word: "pies",
      pos: "noun",
      senses: [{ form_of: [{ word: "pie" }], tags: ["form-of", "plural"] }],
    });
    expect(ehEntradaUtil(forma)).toBe(false);
  });

  it("aceita palavra com acepção mista de forma e sentido próprio", () => {
    // "book" é lema (substantivo) e forma dialetal de "bake"; como nem toda
    // acepção é form_of, o verbete continua valendo.
    const mista = entrada({
      word: "book",
      pos: "noun",
      senses: [
        { glosses: ["a written work"] },
        { form_of: [{ word: "bake" }] },
      ],
    });
    expect(ehApenasForma(mista)).toBe(false);
    expect(ehEntradaUtil(mista)).toBe(true);
  });
});

describe("ipaDaEntrada", () => {
  it("prefere a pronúncia sem sotaque marcado", () => {
    const e = entrada({
      sounds: [
        { ipa: "/rʌn/", raw_tags: ["Estados Unidos"] },
        { ipa: "/ɹʌn/" },
        { ipa: "/run/", raw_tags: ["Escócia"] },
      ],
    });
    expect(ipaDaEntrada(e)).toBe("/ɹʌn/");
  });

  it("cai na primeira quando todas têm sotaque", () => {
    const e = entrada({ sounds: [{ ipa: "/rʌn/", raw_tags: ["EUA"] }] });
    expect(ipaDaEntrada(e)).toBe("/rʌn/");
  });

  it("devolve nulo sem pronúncia", () => {
    expect(
      ipaDaEntrada(entrada({ sounds: [{ audio: "run.ogg" }] })),
    ).toBeNull();
  });
});

describe("distribuirTraducoes", () => {
  it("usa o _dis1 para escolher a acepção de cada tradução", () => {
    // Duas acepções; a tradução tem nota alta na segunda.
    const e = entrada({
      senses: [{ glosses: ["to move fast"] }, { glosses: ["to manage"] }],
      translations: [
        { code: "pt", word: "correr", _dis1: "90 10" },
        { code: "pt", word: "gerir", _dis1: "5 95" },
      ],
    });
    const { porAcepcao, geral } = distribuirTraducoes(e);
    expect(porAcepcao[0]).toEqual(["correr"]);
    expect(porAcepcao[1]).toEqual(["gerir"]);
    expect(geral).toEqual([]);
  });

  it("manda para o bolo geral quando o _dis1 não bate com as acepções", () => {
    // Distribuição com tamanho errado é dado inconsistente do dump: melhor o
    // bolo geral do que apontar para a acepção errada.
    const e = entrada({
      senses: [{ glosses: ["a"] }, { glosses: ["b"] }],
      translations: [{ code: "pt", word: "correr", _dis1: "1 2 3" }],
    });
    const { porAcepcao, geral } = distribuirTraducoes(e);
    expect(porAcepcao).toEqual([[], []]);
    expect(geral).toEqual(["correr"]);
  });

  it("ignora _dis1 todo zerado", () => {
    const e = entrada({
      senses: [{ glosses: ["a"] }],
      translations: [{ code: "pt", word: "correr", _dis1: "0" }],
    });
    expect(distribuirTraducoes(e).geral).toEqual(["correr"]);
  });

  it("prefere a tradução pendurada na própria acepção", () => {
    const e = entrada({
      senses: [
        { glosses: ["a"], translations: [{ code: "pt", word: "corrida" }] },
      ],
      translations: [{ code: "pt", word: "outra", _dis1: "100" }],
    });
    // A da acepção fica; a de nível de entrada não sobrescreve.
    expect(distribuirTraducoes(e).porAcepcao[0]).toEqual(["corrida", "outra"]);
  });

  it("descarta idiomas que não são o português", () => {
    const e = entrada({
      senses: [{ glosses: ["a"] }],
      translations: [
        { code: "es", word: "correr" },
        { code: "fr", word: "courir" },
      ],
    });
    const { porAcepcao, geral } = distribuirTraducoes(e);
    expect(porAcepcao[0]).toEqual([]);
    expect(geral).toEqual([]);
  });

  it("não repete a mesma tradução", () => {
    const e = entrada({
      senses: [{ glosses: ["a"] }],
      translations: [
        { code: "pt", word: "correr" },
        { code: "pt", word: "correr" },
      ],
    });
    expect(distribuirTraducoes(e).geral).toEqual(["correr"]);
  });
});

describe("acepcoesDaEntrada", () => {
  it("junta as traduções numa glosa e guarda a inglesa junto", () => {
    const e = entrada({
      senses: [
        { glosses: ["to move fast"], examples: [{ text: "He ran home." }] },
      ],
      translations: [
        { code: "pt", word: "correr" },
        { code: "pt", word: "disparar" },
      ],
    });
    expect(acepcoesDaEntrada(e)).toEqual([
      {
        glossPt: "correr, disparar",
        glossEn: "to move fast",
        examples: ["He ran home."],
      },
    ]);
  });

  it("descarta acepção sem português", () => {
    // Sem tradução a acepção não serve a um dicionário EN→PT.
    const e = entrada({
      senses: [{ glosses: ["sem tradução"] }, { glosses: ["com tradução"] }],
      translations: [{ code: "pt", word: "algo", _dis1: "0 100" }],
    });
    const acepcoes = acepcoesDaEntrada(e);
    expect(acepcoes).toHaveLength(1);
    expect(acepcoes[0].glossEn).toBe("com tradução");
  });

  it("o bolo geral vai só para a primeira acepção", () => {
    const e = entrada({
      senses: [{ glosses: ["primeira"] }, { glosses: ["segunda"] }],
      translations: [{ code: "pt", word: "algo" }],
    });
    const acepcoes = acepcoesDaEntrada(e);
    expect(acepcoes).toHaveLength(1);
    expect(acepcoes[0].glossEn).toBe("primeira");
  });

  it("pula acepções que são só remissão a outro lema", () => {
    const e = entrada({
      senses: [{ form_of: [{ word: "outro" }], glosses: ["plural of outro"] }],
      translations: [{ code: "pt", word: "algo" }],
    });
    expect(acepcoesDaEntrada(e)).toEqual([]);
  });

  it("corta exemplos longos demais para um popup", () => {
    const e = entrada({
      senses: [
        {
          glosses: ["a"],
          examples: [{ text: "x".repeat(300) }, { text: "curto" }],
        },
      ],
      translations: [{ code: "pt", word: "algo" }],
    });
    expect(acepcoesDaEntrada(e)[0].examples).toEqual(["curto"]);
  });
});

describe("acepcoesDoWiktionarioPt", () => {
  it("usa a definição em português como glosa, sem inglês", () => {
    const e = {
      word: "software",
      senses: [
        { glosses: ["as instruções do computador"] },
        { glosses: ["programas que seguem uma lógica"] },
      ],
    };
    expect(acepcoesDoWiktionarioPt(e)).toEqual([
      { glossPt: "as instruções do computador", glossEn: null, examples: [] },
      {
        glossPt: "programas que seguem uma lógica",
        glossEn: null,
        examples: [],
      },
    ]);
  });
});

describe("formasDaEntrada", () => {
  it("tira a flexão da lista de formas do lema", () => {
    const e = entrada({
      word: "dictionary",
      forms: [
        { form: "dictionaries", tags: ["plural"] },
        { form: "dictionnary", tags: ["alternative"] },
      ],
    });
    expect(formasDaEntrada(e)).toEqual([
      { form: "dictionaries", lemma: "dictionary" },
      { form: "dictionnary", lemma: "dictionary" },
    ]);
  });

  it("ignora formas sem tag de flexão", () => {
    // Transliteração e afins não são flexão e poluiriam a tabela.
    const e = entrada({
      word: "run",
      forms: [
        { form: "ran-romaji", tags: ["romanization"] },
        { form: "runs", tags: ["plural"] },
      ],
    });
    expect(formasDaEntrada(e)).toEqual([{ form: "runs", lemma: "run" }]);
  });

  it("tira o par do verbete de forma flexionada", () => {
    const e = entrada({
      word: "Pies",
      pos: "noun",
      senses: [{ form_of: [{ word: "pie" }], tags: ["form-of", "plural"] }],
    });
    // A forma é normalizada para busca; o lema mantém a grafia do dicionário.
    expect(formasDaEntrada(e)).toEqual([{ form: "pies", lemma: "pie" }]);
  });

  it("aceita form_of em texto puro", () => {
    const e = entrada({ word: "ran", senses: [{ form_of: ["run"] }] });
    expect(formasDaEntrada(e)).toEqual([{ form: "ran", lemma: "run" }]);
  });

  it("descarta forma igual ao próprio lema", () => {
    const e = entrada({
      word: "run",
      forms: [{ form: "run", tags: ["present"] }],
    });
    expect(formasDaEntrada(e)).toEqual([]);
  });

  it("descarta os marcadores de ausência de forma", () => {
    const e = entrada({
      word: "run",
      forms: [
        { form: "-", tags: ["plural"] },
        { form: "no-plural", tags: ["plural"] },
      ],
    });
    expect(formasDaEntrada(e)).toEqual([]);
  });
});
