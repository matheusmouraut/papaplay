import { useMemo } from "react";

import type { LookupResult, LookupWord } from "../../shared/types";

/**
 * Camada de destaques: uma caixa por palavra lida da tela.
 *
 * Os retângulos já chegam do core em pixels lógicos relativos à overlay
 * (`lookup::para_overlay`), então aqui não há nenhuma conta de DPI — só
 * posicionamento. Se um destaque sair torto, o bug está no Rust, não aqui.
 *
 * O tooltip mostra a palavra e a frase em que ela apareceu. O significado
 * entra quando o dicionário chegar (F3); por ora isto prova que o pipeline
 * captura → OCR → posição está de pé.
 */

/** Palavras com confiança abaixo disto viram ruído visual — não desenhamos. */
const CONF_MINIMA = 0.5;

export function WordHighlights({
  resultado,
  cursor,
}: {
  resultado: LookupResult;
  /** Cursor em pixels lógicos da overlay, ou `null` antes do primeiro movimento. */
  cursor: { x: number; y: number } | null;
}) {
  const visiveis = useMemo(
    () => resultado.words.filter((palavra) => palavra.conf >= CONF_MINIMA),
    [resultado.words],
  );

  // A última palavra da lista vence quando duas caixas se sobrepõem: as caixas
  // de palavra saem de uma faixa do CTC (spike 02) e podem encostar nas
  // vizinhas, e a de cima é a que o usuário enxerga.
  const sobCursor = useMemo(() => {
    if (!cursor) return null;
    let escolhida: LookupWord | null = null;
    for (const palavra of visiveis) {
      const { x, y, w, h } = palavra.rect;
      if (
        cursor.x >= x &&
        cursor.x < x + w &&
        cursor.y >= y &&
        cursor.y < y + h
      ) {
        escolhida = palavra;
      }
    }
    return escolhida;
  }, [visiveis, cursor]);

  const linha =
    sobCursor !== null ? resultado.lines[sobCursor.lineIndex]?.text : undefined;

  return (
    <>
      {visiveis.map((palavra, i) => {
        const ativa = palavra === sobCursor;
        return (
          <div
            key={`${palavra.text}-${i}`}
            className={
              ativa
                ? "absolute rounded-sm border border-papa-accent bg-papa-accent/25"
                : "absolute rounded-sm border border-papa-accent/25"
            }
            style={{
              left: palavra.rect.x,
              top: palavra.rect.y,
              width: palavra.rect.w,
              height: palavra.rect.h,
            }}
          />
        );
      })}

      {sobCursor && <Tooltip palavra={sobCursor} frase={linha} />}
    </>
  );
}

/**
 * Balão abaixo da palavra. Sobe para cima dela quando não há espaço embaixo,
 * senão o tooltip da última linha de diálogo sairia da tela — e é justamente
 * ali que fica o texto que interessa nos jogos.
 */
function Tooltip({ palavra, frase }: { palavra: LookupWord; frase?: string }) {
  const LARGURA = 320;
  const ALTURA_ESTIMADA = 96;
  const MARGEM = 8;

  const abaixo = palavra.rect.y + palavra.rect.h + MARGEM;
  const cabeEmbaixo = abaixo + ALTURA_ESTIMADA < window.innerHeight;
  const top = cabeEmbaixo ? abaixo : palavra.rect.y - ALTURA_ESTIMADA - MARGEM;
  const left = Math.min(
    Math.max(palavra.rect.x, MARGEM),
    window.innerWidth - LARGURA - MARGEM,
  );

  return (
    <div
      className="absolute rounded-lg border border-papa-border bg-black/90 p-3 shadow-2xl"
      style={{ left, top, width: LARGURA }}
    >
      <p className="text-base font-semibold text-papa-text">{palavra.text}</p>
      {frase && <p className="mt-1 text-xs text-papa-muted">{frase}</p>}
      <p className="mt-2 text-[11px] text-papa-muted/70">
        Dicionário e tradução entram na próxima etapa.
      </p>
    </div>
  );
}
