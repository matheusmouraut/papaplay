/**
 * Marcação da palavra estudada dentro da frase de contexto.
 *
 * Existe como função pura, e não como um `dangerouslySetInnerHTML` no
 * componente, porque a frase vem de OCR de tela de jogo: é texto de terceiros,
 * e montar HTML com ele seria injeção esperando acontecer.
 */

/** Um pedaço da frase. `destaque` marca a palavra que o card ensina. */
export interface Pedaco {
  texto: string;
  destaque: boolean;
}

/** Neutraliza os metacaracteres — "don't" e "(un)do" aparecem em jogo. */
function escaparRegex(bruto: string): string {
  return bruto.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}

/**
 * Quebra a frase nos pedaços que o componente vai renderizar.
 *
 * Casa palavra inteira e ignora caixa: a forma salva é "ran" e a frase pode
 * trazer "Ran"; por outro lado, destacar "ran" dentro de "brand" seria ruído.
 * Sem forma, ou sem ocorrência dela, devolve a frase inteira sem destaque — a
 * revisão continua possível, só sem a marca.
 */
export function destacarForma(frase: string, forma: string): Pedaco[] {
  const alvo = forma.trim();
  if (!alvo) return [{ texto: frase, destaque: false }];

  // `\b` não fecha em volta de apóstrofo nem de hífen, que é justamente o que
  // separa palavras em inglês; as bordas explícitas evitam casar "ran" no meio
  // de "brand" sem quebrar "don't".
  const padrao = new RegExp(
    `(?<![\\p{L}\\p{N}])${escaparRegex(alvo)}(?![\\p{L}\\p{N}])`,
    "giu",
  );

  const pedacos: Pedaco[] = [];
  let cursor = 0;
  for (const achado of frase.matchAll(padrao)) {
    const inicio = achado.index;
    if (inicio > cursor) {
      pedacos.push({ texto: frase.slice(cursor, inicio), destaque: false });
    }
    pedacos.push({ texto: achado[0], destaque: true });
    cursor = inicio + achado[0].length;
  }
  if (cursor < frase.length) {
    pedacos.push({ texto: frase.slice(cursor), destaque: false });
  }
  return pedacos.length > 0 ? pedacos : [{ texto: frase, destaque: false }];
}
