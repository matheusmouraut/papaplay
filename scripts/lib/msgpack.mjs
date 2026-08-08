/**
 * Decodificador msgpack mínimo — só o suficiente para ler os dados do wordfreq.
 *
 * Por que não uma biblioteca: o wordfreq publica a lista de frequência em
 * msgpack dentro do wheel do PyPI, e é o único lugar do projeto que precisa do
 * formato. Uma dependência a mais no build para ~120 linhas de decodificação
 * não se paga, ainda mais sendo um formato congelado.
 *
 * Cobre o que aparece no arquivo (arrays, mapas, strings, inteiros) e falha
 * alto no resto, em vez de devolver dado errado em silêncio.
 */

/** Lê um valor msgpack a partir de `pos`. Devolve `[valor, próximaPosição]`. */
function ler(buf, pos) {
  const byte = buf[pos];

  // Inteiro positivo curto: 0xxxxxxx
  if (byte <= 0x7f) return [byte, pos + 1];
  // Inteiro negativo curto: 111xxxxx
  if (byte >= 0xe0) return [byte - 0x100, pos + 1];
  // Mapa curto: 1000xxxx
  if (byte >= 0x80 && byte <= 0x8f) return lerMapa(buf, pos + 1, byte & 0x0f);
  // Array curto: 1001xxxx
  if (byte >= 0x90 && byte <= 0x9f) return lerArray(buf, pos + 1, byte & 0x0f);
  // String curta: 101xxxxx
  if (byte >= 0xa0 && byte <= 0xbf) return lerString(buf, pos + 1, byte & 0x1f);

  switch (byte) {
    case 0xc0:
      return [null, pos + 1];
    case 0xc2:
      return [false, pos + 1];
    case 0xc3:
      return [true, pos + 1];
    case 0xca:
      return [buf.readFloatBE(pos + 1), pos + 5];
    case 0xcb:
      return [buf.readDoubleBE(pos + 1), pos + 9];
    case 0xcc:
      return [buf.readUInt8(pos + 1), pos + 2];
    case 0xcd:
      return [buf.readUInt16BE(pos + 1), pos + 3];
    case 0xce:
      return [buf.readUInt32BE(pos + 1), pos + 5];
    case 0xd0:
      return [buf.readInt8(pos + 1), pos + 2];
    case 0xd1:
      return [buf.readInt16BE(pos + 1), pos + 3];
    case 0xd2:
      return [buf.readInt32BE(pos + 1), pos + 5];
    case 0xd9:
      return lerString(buf, pos + 2, buf.readUInt8(pos + 1));
    case 0xda:
      return lerString(buf, pos + 3, buf.readUInt16BE(pos + 1));
    case 0xdb:
      return lerString(buf, pos + 5, buf.readUInt32BE(pos + 1));
    case 0xdc:
      return lerArray(buf, pos + 3, buf.readUInt16BE(pos + 1));
    case 0xdd:
      return lerArray(buf, pos + 5, buf.readUInt32BE(pos + 1));
    case 0xde:
      return lerMapa(buf, pos + 3, buf.readUInt16BE(pos + 1));
    case 0xdf:
      return lerMapa(buf, pos + 5, buf.readUInt32BE(pos + 1));
    default:
      throw new Error(
        `msgpack: byte 0x${byte.toString(16)} não suportado (posição ${pos})`,
      );
  }
}

function lerString(buf, pos, tamanho) {
  return [buf.toString("utf8", pos, pos + tamanho), pos + tamanho];
}

function lerArray(buf, pos, tamanho) {
  const itens = new Array(tamanho);
  for (let i = 0; i < tamanho; i++) {
    [itens[i], pos] = ler(buf, pos);
  }
  return [itens, pos];
}

function lerMapa(buf, pos, pares) {
  const mapa = {};
  for (let i = 0; i < pares; i++) {
    let chave, valor;
    [chave, pos] = ler(buf, pos);
    [valor, pos] = ler(buf, pos);
    mapa[chave] = valor;
  }
  return [mapa, pos];
}

/** Decodifica o primeiro valor msgpack do buffer. */
export function decodeMsgpack(buf) {
  const [valor] = ler(buf, 0);
  return valor;
}

/**
 * Converte os dados do wordfreq em `palavra -> posição no ranking`.
 *
 * O formato é `[cabeçalho, faixa0, faixa1, ...]`, onde cada faixa é uma lista
 * de palavras de frequência parecida, da mais comum para a mais rara. O rank
 * sai da ordem de leitura: 1 = a palavra mais comum do idioma.
 */
export function ranksDoWordfreq(dados) {
  if (!Array.isArray(dados) || dados.length < 2) {
    throw new Error("wordfreq: esperava [cabeçalho, ...faixas]");
  }
  const ranks = new Map();
  let proximo = 1;
  for (const faixa of dados.slice(1)) {
    if (!Array.isArray(faixa)) continue;
    for (const palavra of faixa) {
      // A primeira ocorrência vence: faixas anteriores são mais frequentes.
      if (typeof palavra === "string" && !ranks.has(palavra)) {
        ranks.set(palavra, proximo++);
      }
    }
  }
  return ranks;
}
