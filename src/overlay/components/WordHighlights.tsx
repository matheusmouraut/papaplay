import { useEffect, useMemo, useState } from "react";

import {
  acepcoesPrincipais,
  classeEmPtBr,
  frequenciaDe,
} from "../../shared/dict/apresentacao";
import { useDictEntry } from "../../shared/hooks/useDictEntry";
import type { DictEntry, LookupResult, LookupWord } from "../../shared/types";

/**
 * Camada de destaques: uma caixa por palavra lida da tela, com o verbete da
 * palavra sob o cursor.
 *
 * Os retângulos já chegam do core em pixels lógicos relativos à overlay
 * (`lookup::para_overlay`), então aqui não há nenhuma conta de DPI — só
 * posicionamento. Se um destaque sair torto, o bug está no Rust, não aqui.
 *
 * Dois níveis, como a F3 pede: passar o mouse mostra a linha curta
 * (`palavra → tradução (classe)`); clicar abre as acepções, o IPA e a frase de
 * contexto. A tradução da frase entra quando o motor NMT existir.
 */

/** Palavras com confiança abaixo disto viram ruído visual — não desenhamos. */
const CONF_MINIMA = 0.5;

/** Espera antes de consultar o dicionário, em ms (critério de aceite da F3). */
const ATRASO_DO_HOVER = 300;

/** Acepções no popup — o resto vira ruído numa tela de jogo. */
const MAX_ACEPCOES_VISIVEIS = 4;

export function WordHighlights({
  resultado,
  cursor,
}: {
  resultado: LookupResult;
  /** Cursor em pixels lógicos da overlay, ou `null` antes do primeiro movimento. */
  cursor: { x: number; y: number } | null;
}) {
  const [fixada, setFixada] = useState<LookupWord | null>(null);

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

  // A palavra fixada por clique manda: sem isso, mover o mouse para dentro do
  // próprio popup trocaria o conteúdo dele embaixo do cursor.
  const ativa = fixada ?? sobCursor;

  // O atraso existe para não disparar uma consulta por pixel percorrido — o
  // mouse cruza dezenas de palavras a caminho da que interessa. Clique não
  // espera: o usuário já escolheu.
  const [amadurecida, setAmadurecida] = useState<string | null>(null);
  useEffect(() => {
    if (!ativa) return;
    const id = setTimeout(
      () => setAmadurecida(ativa.text),
      fixada ? 0 : ATRASO_DO_HOVER,
    );
    return () => clearTimeout(id);
  }, [ativa, fixada]);

  // Derivado em vez de guardado: a palavra só está "consultada" enquanto ainda
  // é a ativa. Sem isso, sair de uma palavra deixaria o verbete dela na tela.
  const consultada = ativa && amadurecida === ativa.text ? amadurecida : null;
  const { data: verbete, isFetching } = useDictEntry(consultada);

  const frase = ativa ? resultado.lines[ativa.lineIndex]?.text : undefined;

  return (
    <>
      {visiveis.map((palavra, i) => (
        <div
          key={`${palavra.text}-${i}`}
          onClick={() => setFixada(palavra === fixada ? null : palavra)}
          className={
            palavra === ativa
              ? "absolute cursor-pointer rounded-sm border border-papa-accent bg-papa-accent/25"
              : "absolute cursor-pointer rounded-sm border border-papa-accent/25"
          }
          style={{
            left: palavra.rect.x,
            top: palavra.rect.y,
            width: palavra.rect.w,
            height: palavra.rect.h,
          }}
        />
      ))}

      {ativa && (
        <Balao
          palavra={ativa}
          expandido={fixada !== null}
          verbete={verbete ?? null}
          carregando={consultada === null || isFetching}
          frase={frase}
        />
      )}
    </>
  );
}

/**
 * Balão abaixo da palavra. Sobe para cima dela quando não há espaço embaixo,
 * senão o balão da última linha de diálogo sairia da tela — e é justamente ali
 * que fica o texto que interessa nos jogos.
 */
function Balao({
  palavra,
  expandido,
  verbete,
  carregando,
  frase,
}: {
  palavra: LookupWord;
  expandido: boolean;
  verbete: DictEntry | null;
  carregando: boolean;
  frase?: string;
}) {
  const LARGURA = expandido ? 380 : 300;
  const ALTURA_ESTIMADA = expandido ? 260 : 72;
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
      {verbete ? (
        <Verbete verbete={verbete} expandido={expandido} frase={frase} />
      ) : (
        <>
          <p className="text-base font-semibold text-papa-text">
            {palavra.text}
          </p>
          <p className="mt-1 text-xs text-papa-muted">
            {carregando ? "Consultando…" : "Não está no dicionário."}
          </p>
        </>
      )}
    </div>
  );
}

function Verbete({
  verbete,
  expandido,
  frase,
}: {
  verbete: DictEntry;
  expandido: boolean;
  frase?: string;
}) {
  const acepcoes = acepcoesPrincipais(verbete, MAX_ACEPCOES_VISIVEIS);
  const primeira = acepcoes[0];

  return (
    <>
      <div className="flex items-baseline gap-2">
        <span className="text-base font-semibold text-papa-text">
          {verbete.lemma}
        </span>
        {/* A forma da tela vira legenda: é assim que o usuário entende que
            passou o mouse em "ran" e o card vai ser de "run". */}
        {verbete.matchedForm && (
          <span className="text-xs text-papa-muted">
            ← {verbete.matchedForm}
          </span>
        )}
        {expandido && verbete.ipa && (
          <span className="text-xs text-papa-muted">{verbete.ipa}</span>
        )}
        <span className="ml-auto shrink-0 rounded bg-white/10 px-1.5 py-0.5 text-[10px] text-papa-muted">
          {frequenciaDe(verbete.freqRank)}
        </span>
      </div>

      {!expandido && primeira && (
        <p className="mt-1 text-sm text-papa-text">
          {primeira.glossPt}{" "}
          <span className="text-papa-muted">
            ({classeEmPtBr(primeira.pos)})
          </span>
        </p>
      )}

      {expandido && (
        <ol className="mt-2 space-y-1.5">
          {acepcoes.map((sense, i) => (
            <li key={`${sense.pos}-${i}`} className="text-sm text-papa-text">
              <span className="mr-1 text-xs text-papa-muted">
                {classeEmPtBr(sense.pos)}
              </span>
              {sense.glossPt}
              {sense.glossEn && (
                <span className="block text-xs text-papa-muted">
                  {sense.glossEn}
                </span>
              )}
            </li>
          ))}
        </ol>
      )}

      {expandido && frase && (
        <div className="mt-3 border-t border-papa-border pt-2">
          <p className="text-xs text-papa-muted">{frase}</p>
          <p className="mt-1 text-[11px] text-papa-muted/70">
            Tradução da frase e salvar no deck entram nas próximas etapas.
          </p>
        </div>
      )}

      {!expandido && (
        <p className="mt-1 text-[11px] text-papa-muted/70">
          Clique para ver as acepções.
        </p>
      )}
    </>
  );
}
