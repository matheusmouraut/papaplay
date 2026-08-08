/**
 * Extração das entradas do wiktextract (kaikki.org) para o formato do dict.db.
 *
 * As funções aqui são puras de propósito: o pipeline em `build-dict.mjs` é
 * lento demais para servir de teste, então a lógica que decide o que vira
 * acepção, tradução e lema fica isolada e coberta por testes.
 *
 * Duas fontes, com papéis diferentes:
 *
 * - **Wiktionary EN** (`lang_code: "en"`): a base. Traz IPA, acepções em
 *   inglês, tabelas de tradução (é de onde sai o português) e os dados de
 *   flexão que viram `lemma_forms`.
 * - **Wiktionary PT**: definições em português de verdade, escritas por
 *   humanos. Cobre pouca coisa (~15k palavras), mas onde cobre é melhor que
 *   uma lista de traduções — entra como complemento.
 */

/** Classes gramaticais que interessam. O resto é ruído para o produto. */
const POS_UTEIS = new Set([
  "noun",
  "verb",
  "adj",
  "adv",
  "pron",
  "prep",
  "conj",
  "intj",
  "num",
  "det",
  "phrase",
  "prep_phrase",
  "proverb",
]);

/**
 * Tags que marcam uma forma flexionada aproveitável.
 *
 * A lista `forms` do wiktextract mistura flexão com transliteração, variantes
 * dialetais e lixo de template; sem filtro, "lemma_forms" viraria um mapa de
 * qualquer coisa para qualquer coisa.
 */
const TAGS_DE_FLEXAO = new Set([
  "plural",
  "singular",
  "past",
  "participle",
  "present",
  "gerund",
  "comparative",
  "superlative",
  "third-person",
  "singular-third-person",
  "alternative",
  "archaic",
  "obsolete",
]);

/** Formas assim não ajudam ninguém e só incham a tabela. */
function formaDescartavel(forma, palavra) {
  if (!forma || typeof forma !== "string") return true;
  const limpa = forma.trim();
  if (limpa.length === 0 || limpa.length > 60) return true;
  if (limpa === palavra) return true;
  // Marcadores que o wiktextract usa quando não há forma de verdade.
  if (limpa === "-" || limpa === "—" || limpa.startsWith("no-")) return true;
  return false;
}

export function normalizar(palavra) {
  return palavra.trim().toLowerCase();
}

/** `true` se a entrada é um lema aproveitável do inglês. */
export function ehEntradaUtil(entrada) {
  if (!entrada || entrada.lang_code !== "en") return false;
  if (!entrada.word || !POS_UTEIS.has(entrada.pos)) return false;
  // Entradas de forma flexionada ("pies") não viram verbete próprio: elas
  // alimentam `lemma_forms` e o usuário é levado ao lema.
  return !ehApenasForma(entrada);
}

/** `true` se a entrada existe só para apontar para outro lema. */
export function ehApenasForma(entrada) {
  const senses = entrada.senses || [];
  if (senses.length === 0) return false;
  return senses.every((s) => Array.isArray(s.form_of) && s.form_of.length > 0);
}

/** Primeiro IPA da entrada, preferindo o que não tem sotaque marcado. */
export function ipaDaEntrada(entrada) {
  const sons = (entrada.sounds || []).filter((s) => s.ipa);
  if (sons.length === 0) return null;
  const neutro = sons.find((s) => !s.raw_tags || s.raw_tags.length === 0);
  return (neutro || sons[0]).ipa || null;
}

/**
 * Distribui as traduções em português pelas acepções.
 *
 * O campo `sense` de uma tradução é o rótulo da tabela de traduções do
 * Wiktionary, **não** o texto da acepção — comparar os dois como string quase
 * nunca casa. Quem faz a ligação é o `_dis1`, a distribuição que o wiktextract
 * calcula: uma nota por acepção, na mesma ordem de `senses`. Cada tradução vai
 * para a acepção de maior nota.
 *
 * Sem `_dis1` a tradução vai para o bolo geral da entrada, que a primeira
 * acepção herda — é a acepção mais comum, então é o palpite menos errado.
 */
export function distribuirTraducoes(entrada) {
  const senses = entrada.senses || [];
  const porAcepcao = senses.map(() => []);
  const geral = [];

  const adicionar = (lista, palavra) => {
    const limpa = (palavra || "").trim();
    if (limpa && !lista.includes(limpa)) lista.push(limpa);
  };

  // Traduções penduradas direto numa acepção são as mais confiáveis.
  senses.forEach((sense, i) => {
    for (const t of sense.translations || []) {
      if (t.code === "pt" || t.lang_code === "pt")
        adicionar(porAcepcao[i], t.word);
    }
  });

  for (const t of entrada.translations || []) {
    if (t.code !== "pt" && t.lang_code !== "pt") continue;
    const notas = pesosDe(t._dis1, senses.length);
    if (notas) {
      adicionar(porAcepcao[indiceDoMaior(notas)], t.word);
    } else {
      adicionar(geral, t.word);
    }
  }

  return { porAcepcao, geral };
}

function pesosDe(dis1, quantasAcepcoes) {
  if (typeof dis1 !== "string" || quantasAcepcoes === 0) return null;
  const notas = dis1
    .trim()
    .split(/\s+/)
    .map((n) => Number.parseFloat(n));
  if (notas.length !== quantasAcepcoes) return null;
  if (notas.some((n) => !Number.isFinite(n))) return null;
  if (notas.every((n) => n === 0)) return null;
  return notas;
}

function indiceDoMaior(notas) {
  let melhor = 0;
  for (let i = 1; i < notas.length; i++) {
    if (notas[i] > notas[melhor]) melhor = i;
  }
  return melhor;
}

/** Quantos exemplos guardar por acepção — o popup mostra poucos. */
const MAX_EXEMPLOS = 2;

/**
 * Acepções da entrada, já com o português resolvido.
 *
 * Só sobrevivem acepções com tradução: uma acepção sem português não serve ao
 * produto, que é EN→PT. A ordem do Wiktionary é preservada porque ela já vem
 * da mais comum para a mais rara.
 */
export function acepcoesDaEntrada(entrada) {
  const { porAcepcao, geral } = distribuirTraducoes(entrada);
  const senses = entrada.senses || [];
  const acepcoes = [];

  senses.forEach((sense, i) => {
    if (Array.isArray(sense.form_of) && sense.form_of.length > 0) return;
    const glosses = sense.glosses || sense.raw_glosses || [];
    // O bolo geral vale só para a primeira acepção (ver `distribuirTraducoes`).
    const traducoes =
      porAcepcao[i].length > 0 ? porAcepcao[i] : i === 0 ? geral : [];
    if (traducoes.length === 0) return;

    acepcoes.push({
      glossPt: traducoes.join(", "),
      glossEn: glosses[0] || null,
      examples: (sense.examples || [])
        .map((e) => e.text)
        .filter((t) => typeof t === "string" && t.length > 0 && t.length < 240)
        .slice(0, MAX_EXEMPLOS),
    });
  });

  return acepcoes;
}

/**
 * Acepções vindas do Wiktionary PT: a definição já está em português.
 *
 * Aqui `glossEn` é nulo de propósito — não existe glosa inglesa nessa fonte, e
 * inventar uma seria pior que não ter.
 */
export function acepcoesDoWiktionarioPt(entrada) {
  return (entrada.senses || [])
    .map((s) => (s.glosses || [])[0])
    .filter((g) => typeof g === "string" && g.trim().length > 0)
    .map((g) => ({ glossPt: g.trim(), glossEn: null, examples: [] }));
}

/**
 * Pares `forma -> lema` da entrada, pelos dois caminhos que o dump oferece.
 *
 * 1. `forms[]` num lema: a flexão listada no próprio verbete ("dictionaries").
 * 2. `senses[].form_of` numa entrada de forma: o verbete "pies" apontando para
 *    "pie".
 *
 * Os dois são incompletos sozinhos, e juntos se cobrem.
 */
export function formasDaEntrada(entrada) {
  const pares = [];
  const palavra = entrada.word;
  if (!palavra) return pares;

  for (const f of entrada.forms || []) {
    const tags = f.tags || [];
    if (!tags.some((t) => TAGS_DE_FLEXAO.has(t))) continue;
    if (formaDescartavel(f.form, palavra)) continue;
    pares.push({ form: normalizar(f.form), lemma: palavra });
  }

  for (const sense of entrada.senses || []) {
    for (const alvo of sense.form_of || []) {
      const lema = typeof alvo === "string" ? alvo : alvo.word;
      if (!lema || formaDescartavel(palavra, lema)) continue;
      pares.push({ form: normalizar(palavra), lemma: lema });
    }
  }

  return pares;
}
