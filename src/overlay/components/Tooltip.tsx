import { classeEmPtBr, frequenciaDe } from "../../shared/dict/apresentacao";
import { useDictEntry } from "../../shared/hooks/useDictEntry";
import type { PeekFocus } from "../../shared/types";
import { Ancora } from "./Ancora";
import { Sublinhado } from "./Sublinhado";

/**
 * O que aparece enquanto a tecla está segurada: uma linha de texto.
 *
 * Deliberadamente pequeno. Espiar é um gesto de meio segundo no meio do jogo —
 * o que cabe aqui é `palavra → tradução (classe)` e nada mais. O resto está a
 * um clique de distância, e é aí que o usuário decidiu parar para ler.
 */
export function Tooltip({ foco }: { foco: PeekFocus }) {
  const { data: verbete, isFetching } = useDictEntry(foco.word);
  const primeira = verbete?.senses[0];

  return (
    <>
      <Sublinhado rect={foco.rect} />
      <Ancora rect={foco.rect}>
        <div className="papa-vidro max-w-md rounded-lg px-3 py-2">
          {verbete && primeira ? (
            <p className="text-[15px] leading-snug text-papa-text">
              {/* O lema primeiro: é assim que se descobre que "ran" vira o
                  card de "run", sem precisar de uma legenda explicando. */}
              <span className="font-medium">{verbete.lemma}</span>
              <span className="mx-1.5 text-papa-faint">→</span>
              {primeira.glossPt}
              <span className="ml-1.5 text-[13px] text-papa-muted">
                {classeEmPtBr(primeira.pos)}
              </span>
              {frequenciaDe(verbete.freqRank) === "rara" && (
                <span className="ml-1.5 text-[13px] text-papa-faint">
                  · rara
                </span>
              )}
            </p>
          ) : (
            <p className="text-[15px] leading-snug text-papa-muted">
              <span className="font-medium text-papa-text">{foco.word}</span>
              <span className="ml-1.5">
                {isFetching ? "…" : "não está no dicionário"}
              </span>
            </p>
          )}

          {/* A dica do teclado só aparece quando há verbete: sem ele, abrir o
              card não leva a lugar nenhum. */}
          {verbete && (
            <p className="mt-1 text-[11px] text-papa-faint">
              clique ou <kbd className="font-mono">Alt+C</kbd> para ver mais
            </p>
          )}
        </div>
      </Ancora>
    </>
  );
}
