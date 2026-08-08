import type { DictEntry } from "../types";

/**
 * Como o verbete vira texto na tela.
 *
 * Separado do componente porque é regra de produto, não de layout: a abreviação
 * da classe gramatical e a faixa de frequência aparecem no tooltip, no popup e
 * (depois) no card, e precisam ser as mesmas nos três.
 */

/** Classe gramatical do Wiktionary → abreviação em PT-BR. */
const CLASSES: Record<string, string> = {
  noun: "subst.",
  verb: "verbo",
  adj: "adj.",
  adv: "adv.",
  pron: "pron.",
  prep: "prep.",
  conj: "conj.",
  intj: "interj.",
  num: "num.",
  det: "det.",
  article: "art.",
  phrase: "expr.",
  prep_phrase: "expr.",
  proverb: "provérbio",
  name: "nome próprio",
};

export function classeEmPtBr(pos: string): string {
  return CLASSES[pos] ?? pos;
}

/** As classes gramaticais do verbete, sem repetir, na ordem em que aparecem. */
export function classesDoVerbete(entrada: DictEntry): string[] {
  const vistas: string[] = [];
  for (const sense of entrada.senses) {
    const classe = classeEmPtBr(sense.pos);
    if (!vistas.includes(classe)) vistas.push(classe);
  }
  return vistas;
}

export type Frequencia = "comum" | "média" | "rara";

/**
 * Faixa de frequência da palavra, para o usuário decidir se vale salvar.
 *
 * Os cortes saem do ranking do wordfreq: as ~3 mil primeiras cobrem a conversa
 * do dia a dia, e depois de ~20 mil a palavra já é específica o bastante para
 * valer um card. Sem rank, tratamos como rara — o que não está na lista de
 * frequência é, por definição, incomum.
 */
export function frequenciaDe(freqRank: number | null): Frequencia {
  if (freqRank === null) return "rara";
  if (freqRank <= 3000) return "comum";
  if (freqRank <= 20000) return "média";
  return "rara";
}

/**
 * Linha curta do tooltip: `palavra → tradução (classe)`.
 *
 * Mostra o lema quando a palavra da tela é uma flexão — é assim que o usuário
 * descobre que "ran" vira o card de "run".
 */
export function resumoDoVerbete(entrada: DictEntry): string {
  const primeira = entrada.senses[0];
  if (!primeira) return entrada.lemma;
  const classe = classeEmPtBr(primeira.pos);
  return `${entrada.lemma} → ${primeira.glossPt} (${classe})`;
}

/** Até `limite` acepções, sem repetir a mesma glosa em português. */
export function acepcoesPrincipais(entrada: DictEntry, limite: number) {
  const vistas = new Set<string>();
  const escolhidas = [];
  for (const sense of entrada.senses) {
    const chave = `${sense.pos}|${sense.glossPt}`;
    if (vistas.has(chave)) continue;
    vistas.add(chave);
    escolhidas.push(sense);
    if (escolhidas.length >= limite) break;
  }
  return escolhidas;
}
