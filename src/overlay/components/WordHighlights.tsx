import { useEffect, useMemo, useState } from "react";

import {
  acepcoesPrincipais,
  classeEmPtBr,
  frequenciaDe,
} from "../../shared/dict/apresentacao";
import { useCardStatus, useSaveCard } from "../../shared/hooks/useDeckCard";
import { useDictEntry } from "../../shared/hooks/useDictEntry";
import { useSentenceTranslation } from "../../shared/hooks/useSentenceTranslation";
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
 * (`palavra → tradução (classe)`); clicar abre as acepções, o IPA, a frase de
 * contexto e a tradução dela.
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

  // Só a palavra fixada por clique traduz a frase: no hover o custo estoura o
  // orçamento do tooltip (ver `useSentenceTranslation`).
  const {
    data: traducao,
    isFetching: traduzindo,
    error: erroDaTraducao,
  } = useSentenceTranslation(fixada && frase ? frase : null);

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
          traducao={{
            texto: traducao,
            carregando: traduzindo,
            erro: erroDaTraducao ? String(erroDaTraducao) : undefined,
          }}
          gameName={resultado.gameName}
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
  traducao,
  gameName,
}: {
  palavra: LookupWord;
  expandido: boolean;
  verbete: DictEntry | null;
  carregando: boolean;
  frase?: string;
  traducao: Traducao;
  gameName: string | null;
}) {
  const LARGURA = expandido ? 380 : 300;
  const ALTURA_ESTIMADA = expandido ? 300 : 72;
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
        <Verbete
          verbete={verbete}
          expandido={expandido}
          frase={frase}
          traducao={traducao}
          form={palavra.text}
          gameName={gameName}
        />
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

/** Estado da tradução da frase de contexto, do ponto de vista do balão. */
type Traducao = {
  texto?: string;
  carregando: boolean;
  erro?: string;
};

function Verbete({
  verbete,
  expandido,
  frase,
  traducao,
  form,
  gameName,
}: {
  verbete: DictEntry;
  expandido: boolean;
  frase?: string;
  traducao: Traducao;
  /** A palavra como estava na tela — é ela que vai para o contexto do card. */
  form: string;
  gameName: string | null;
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
          {/* A frase original fica acima da tradução de propósito: quem está
              aprendendo lê o inglês primeiro e usa o português como conferência. */}
          {traducao.erro ? (
            <p className="mt-1 text-[11px] text-red-300">{traducao.erro}</p>
          ) : (
            <p className="mt-1 text-sm text-papa-text">
              {traducao.texto ??
                (traducao.carregando ? (
                  <span className="text-papa-muted">Traduzindo…</span>
                ) : null)}
            </p>
          )}
        </div>
      )}

      {expandido && frase && (
        <BotaoSalvar
          lemma={verbete.lemma}
          form={form}
          sentenceEn={frase}
          sentencePt={traducao.texto ?? null}
          gameName={gameName}
        />
      )}

      {!expandido && (
        <p className="mt-1 text-[11px] text-papa-muted/70">
          Clique para ver as acepções.
        </p>
      )}
    </>
  );
}

/**
 * Salvar no deck. Um card por lema: reencontrar a mesma palavra em outra frase
 * anexa contexto ao card que já existe, e é por isso que o botão continua
 * clicável depois de salvo.
 *
 * A tradução da frase entra no card se já tiver chegado — não vale segurar o
 * salvamento esperando por ela. O card guarda a frase em inglês de todo jeito,
 * e a tradução pode ser reposta depois, na janela principal.
 */
function BotaoSalvar({
  lemma,
  form,
  sentenceEn,
  sentencePt,
  gameName,
}: {
  lemma: string;
  form: string;
  sentenceEn: string;
  sentencePt: string | null;
  gameName: string | null;
}) {
  const { data: card } = useCardStatus(lemma);
  const salvar = useSaveCard();
  const salvo = card !== null && card !== undefined;

  return (
    <div className="mt-3 border-t border-papa-border pt-2">
      <button
        type="button"
        disabled={salvar.isPending}
        title={
          salvo
            ? "Já está no deck. Clique para anexar esta frase ao card."
            : `Salva o card de "${lemma}" com esta frase.`
        }
        onClick={() =>
          salvar.mutate({ lemma, form, sentenceEn, sentencePt, gameName })
        }
        className={`w-full rounded px-2 py-1.5 text-sm transition-colors disabled:opacity-50 ${
          salvo
            ? "border border-papa-accent/50 bg-papa-accent/10 text-papa-accent"
            : "border border-papa-border text-papa-text hover:bg-white/5"
        }`}
      >
        {salvar.isPending
          ? "Salvando…"
          : salvo
            ? `★ No deck · ${card.contexts} ${card.contexts === 1 ? "contexto" : "contextos"}`
            : "☆ Salvar no deck"}
      </button>

      {salvar.isError && (
        <p className="mt-1 text-[11px] text-red-300">{String(salvar.error)}</p>
      )}
    </div>
  );
}
