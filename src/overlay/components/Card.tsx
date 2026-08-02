import { useEffect } from "react";

import { peekClose } from "../../shared/api/core";
import {
  acepcoesPrincipais,
  classeEmPtBr,
  frequenciaDe,
} from "../../shared/dict/apresentacao";
import { useCardStatus, useSaveCard } from "../../shared/hooks/useDeckCard";
import { useDictEntry } from "../../shared/hooks/useDictEntry";
import { useSentenceTranslation } from "../../shared/hooks/useSentenceTranslation";
import type { DictEntry, PeekFocus } from "../../shared/types";
import { Ancora } from "./Ancora";
import { Sublinhado } from "./Sublinhado";

/**
 * O card: o que aparece depois do clique.
 *
 * Aqui o usuário parou para ler, então o desenho muda de tooltip para página:
 * texto de leitura em serifada, acepções numa lista respirada, a frase original
 * acima da tradução. É o momento "editor de texto", não "HUD" (F7).
 *
 * A overlay recebe cliques neste estado — sem tirar o jogo do primeiro plano
 * (`WS_EX_NOACTIVATE`). Clique fora do card fecha.
 */

/** Acepções no card. Mais do que isto vira lista para consultar, não para ler. */
const MAX_ACEPCOES = 4;

export function Card({ foco }: { foco: PeekFocus }) {
  const { data: verbete, isFetching } = useDictEntry(foco.word);
  const frase = foco.sentence.trim();
  const {
    data: traducao,
    isFetching: traduzindo,
    error: erroDaTraducao,
  } = useSentenceTranslation(frase.length > 0 ? frase : null);

  // Salvar com `S` enquanto o card está aberto: a mão já está no teclado por
  // causa do Alt+X, e tirar ela dali para clicar é o que faz desistir de salvar.
  const salvar = useSaveCard();
  const { data: salvo } = useCardStatus(verbete?.lemma ?? null);
  useEffect(() => {
    function onKeyDown(evento: KeyboardEvent) {
      if (evento.key.toLowerCase() !== "s" || !verbete) return;
      evento.preventDefault();
      salvar.mutate({
        lemma: verbete.lemma,
        form: foco.word,
        sentenceEn: frase,
        sentencePt: traducao ?? null,
        gameName: foco.gameName,
        lookupId: foco.lookupId,
        lineIndex: foco.lineIndex,
      });
    }
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [verbete, foco, frase, traducao, salvar]);

  return (
    <>
      <Sublinhado rect={foco.rect} />

      {/* Clique fora fecha. Cobre a tela inteira, mas só existe com o card
          aberto — em `peek` a overlay ainda é click-through. */}
      <div className="absolute inset-0" onClick={() => void peekClose()} />

      <Ancora rect={foco.rect}>
        <div
          className="papa-vidro w-[26rem] rounded-xl px-5 py-4"
          onClick={(evento) => evento.stopPropagation()}
        >
          {verbete ? (
            <Verbete
              verbete={verbete}
              foco={foco}
              frase={frase}
              traducao={{
                texto: traducao,
                carregando: traduzindo,
                erro: erroDaTraducao ? String(erroDaTraducao) : undefined,
              }}
              salvo={salvo?.contexts ?? null}
              salvando={salvar.isPending}
              erroAoSalvar={salvar.isError ? String(salvar.error) : undefined}
              onSalvar={() =>
                salvar.mutate({
                  lemma: verbete.lemma,
                  form: foco.word,
                  sentenceEn: frase,
                  sentencePt: traducao ?? null,
                  gameName: foco.gameName,
                  lookupId: foco.lookupId,
                  lineIndex: foco.lineIndex,
                })
              }
            />
          ) : (
            <>
              <p className="font-reading text-xl text-papa-text">{foco.word}</p>
              <p className="mt-1 text-sm text-papa-muted">
                {isFetching
                  ? "Consultando…"
                  : "Esta palavra não está no dicionário."}
              </p>
            </>
          )}

          <Rodape />
        </div>
      </Ancora>
    </>
  );
}

type Traducao = { texto?: string; carregando: boolean; erro?: string };

function Verbete({
  verbete,
  foco,
  frase,
  traducao,
  salvo,
  salvando,
  erroAoSalvar,
  onSalvar,
}: {
  verbete: DictEntry;
  foco: PeekFocus;
  frase: string;
  traducao: Traducao;
  /** Quantos contextos o card já tem, ou `null` se a palavra não está no deck. */
  salvo: number | null;
  salvando: boolean;
  erroAoSalvar?: string;
  onSalvar: () => void;
}) {
  const acepcoes = acepcoesPrincipais(verbete, MAX_ACEPCOES);
  const frequencia = frequenciaDe(verbete.freqRank);

  return (
    <>
      <header className="flex items-baseline gap-2">
        <h1 className="font-reading text-2xl leading-none text-papa-text">
          {verbete.lemma}
        </h1>
        {verbete.matchedForm && verbete.matchedForm !== verbete.lemma && (
          <span className="text-sm text-papa-faint">
            de “{verbete.matchedForm}”
          </span>
        )}
        {verbete.ipa && (
          <span className="font-mono text-xs text-papa-muted">
            {verbete.ipa}
          </span>
        )}
        <span className="ml-auto text-xs text-papa-faint">{frequencia}</span>
      </header>

      <ol className="mt-4 space-y-2.5">
        {acepcoes.map((sense, i) => (
          <li key={`${sense.pos}-${i}`} className="flex gap-2.5">
            {/* A classe gramatical numa coluna própria: alinhada, ela vira
                índice visual em vez de ruído no meio da frase. */}
            <span className="w-14 shrink-0 pt-0.5 text-right text-xs text-papa-faint">
              {classeEmPtBr(sense.pos)}
            </span>
            <span className="font-reading text-[15px] leading-relaxed text-papa-text">
              {sense.glossPt}
              {sense.glossEn && (
                <span className="mt-0.5 block text-xs leading-normal text-papa-faint">
                  {sense.glossEn}
                </span>
              )}
            </span>
          </li>
        ))}
      </ol>

      {frase && (
        <section className="mt-4 border-t border-papa-border pt-3">
          {/* O inglês primeiro, e em destaque: quem aprende lê o original e
              usa o português como conferência, não o contrário. */}
          <p className="font-reading text-[15px] leading-relaxed text-papa-text">
            {frase}
          </p>
          {traducao.erro ? (
            <p className="mt-1.5 text-xs text-red-300/90">{traducao.erro}</p>
          ) : (
            <p className="mt-1.5 font-reading text-sm leading-relaxed text-papa-muted">
              {traducao.texto ?? (traducao.carregando ? "traduzindo…" : null)}
            </p>
          )}
        </section>
      )}

      <button
        type="button"
        disabled={salvando}
        onClick={onSalvar}
        title={
          salvo !== null
            ? "Já está no deck. Clique para anexar esta frase ao card."
            : undefined
        }
        className={`mt-4 w-full rounded-lg border px-3 py-2 text-sm transition-colors duration-150 disabled:opacity-50 ${
          salvo !== null
            ? "border-papa-accent/40 bg-papa-accent/10 text-papa-accent"
            : "border-papa-border-strong text-papa-text hover:bg-white/[0.06]"
        }`}
      >
        {salvando
          ? "salvando…"
          : salvo !== null
            ? `no deck · ${salvo} ${salvo === 1 ? "contexto" : "contextos"}`
            : "salvar no deck"}
        <span className="ml-2 text-xs text-papa-faint">S</span>
      </button>

      {erroAoSalvar && (
        <p className="mt-1.5 text-xs text-red-300/90">{erroAoSalvar}</p>
      )}

      {/* Deliberadamente fora do fluxo do card: o usuário salvou a palavra
          "ran", e ver que o card é de "run" evita a impressão de bug. */}
      {foco.word.toLowerCase() !== verbete.lemma.toLowerCase() && (
        <p className="mt-2 text-center text-[11px] text-papa-faint">
          o card guarda “{foco.word}” como contexto de {verbete.lemma}
        </p>
      )}
    </>
  );
}

function Rodape() {
  return (
    <footer className="mt-3 flex justify-center gap-3 text-[11px] text-papa-faint">
      <span>Esc fecha</span>
    </footer>
  );
}
