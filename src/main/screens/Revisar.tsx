import { useCallback, useEffect, useState } from "react";

import {
  acepcoesPrincipais,
  classeEmPtBr,
} from "../../shared/dict/apresentacao";
import { Screenshot } from "../../shared/components/Screenshot";
import { Botao, Cartao, Vazio } from "../../shared/components/ui";
import { useApplyReview, useReviewQueue } from "../../shared/hooks/useReview";
import { useDictEntry } from "../../shared/hooks/useDictEntry";
import { usePreferences } from "../../shared/hooks/usePreferences";
import {
  gradeCard,
  Rating,
  withFsrsFields,
  type Grade,
} from "../../shared/srs";
import type { ReviewCard, ReviewQueue } from "../../shared/types";
import { FraseDeContexto } from "../components/FraseDeContexto";
import { NotasDeRevisao } from "../components/NotasDeRevisao";
import { useMainStore } from "../store";

/**
 * Sessão de revisão (F5).
 *
 * A fila é carregada uma vez e avança na memória: recarregá-la a cada nota
 * reordenaria os cards no meio da sessão, e um card que muda de lugar enquanto
 * se responde é a forma mais rápida de perder a confiança no agendamento.
 *
 * Errar devolve o card ao fim da fila da sessão, já com o estado novo. O FSRS
 * agenda a repetição para poucos minutos adiante de qualquer forma; ver a
 * palavra de novo antes de fechar o app é o que fecha o ciclo no mesmo dia.
 */

interface Progresso {
  feitas: number;
  acertos: number;
}

export function Revisar() {
  const preferencias = usePreferences();
  const fila = useReviewQueue(preferencias.data?.newPerDay);

  if (preferencias.isPending || fila.isPending) {
    return <Aviso texto="Montando a fila…" />;
  }
  if (fila.isError || !fila.data) {
    return <Aviso texto={`A fila não carregou: ${String(fila.error)}`} erro />;
  }

  // A `key` é o que reinicia a sessão quando uma fila nova chega: o estado da
  // sessão nasce da fila, e ressemear esse estado por efeito seria uma renderização
  // em cascata a cada recarga.
  return <Sessao key={fila.dataUpdatedAt} fila={fila.data} />;
}

function Sessao({ fila }: { fila: ReviewQueue }) {
  const aplicar = useApplyReview();
  const [restantes, setRestantes] = useState<ReviewCard[]>(fila.cards);
  const [revelado, setRevelado] = useState(false);
  const [progresso, setProgresso] = useState<Progresso>({
    feitas: 0,
    acertos: 0,
  });

  const atual = restantes[0] ?? null;

  const responder = useCallback(
    (nota: Grade) => {
      if (!atual) return;
      const entrada = gradeCard(atual.card, nota);
      const errou = nota === Rating.Again;

      // A tela avança sem esperar o disco: a gravação é um insert local de
      // microssegundos, e travar a revisão por ela seria pagar latência à toa.
      // Se falhar, a mensagem aparece e o card volta para o fim da fila.
      aplicar.mutate(entrada, {
        onError: () => setRestantes((atuais) => [...atuais, atual]),
      });

      setProgresso((p) => ({
        feitas: p.feitas + 1,
        acertos: p.acertos + (errou ? 0 : 1),
      }));
      setRevelado(false);
      setRestantes((atuais) => {
        const resto = atuais.slice(1);
        if (!errou) return resto;
        const repetido: ReviewCard = {
          ...atual,
          card: withFsrsFields(atual.card, entrada.fsrs),
        };
        return [...resto, repetido];
      });
    },
    [atual, aplicar],
  );

  // Revisar é uma tela de teclado: espaço revela, 1–4 avaliam. O mouse
  // continua funcionando, mas quem revisa 30 cards por dia não usa.
  useEffect(() => {
    if (!atual) return;
    const aoTeclar = (evento: KeyboardEvent) => {
      if (evento.repeat || evento.altKey || evento.ctrlKey || evento.metaKey) {
        return;
      }
      if (evento.key === " " || evento.key === "Enter") {
        evento.preventDefault();
        setRevelado(true);
        return;
      }
      if (!revelado) return;
      const posicao = Number(evento.key);
      if (posicao >= 1 && posicao <= 4) {
        evento.preventDefault();
        responder(posicao as Grade);
      }
    };
    window.addEventListener("keydown", aoTeclar);
    return () => window.removeEventListener("keydown", aoTeclar);
  }, [atual, revelado, responder]);

  if (!atual) {
    return progresso.feitas > 0 ? (
      <ResumoDaSessao progresso={progresso} restamNovos={fila.newLeftOver} />
    ) : (
      <NadaParaHoje
        deckVazio={fila.total === 0}
        restamNovos={fila.newLeftOver}
      />
    );
  }

  const total = progresso.feitas + restantes.length;

  return (
    <section className="flex h-full flex-col gap-5">
      <BarraDeProgresso feitas={progresso.feitas} total={total} />

      <CartaoDeRevisao card={atual} revelado={revelado} />

      <div className="mt-auto">
        {revelado ? (
          <NotasDeRevisao
            card={atual.card}
            onNota={responder}
            desabilitado={false}
          />
        ) : (
          <Botao
            tamanho="lg"
            onClick={() => setRevelado(true)}
            className="w-full py-3"
          >
            Mostrar resposta{" "}
            <kbd className="ml-1 font-mono text-[11px] text-papa-faint">
              espaço
            </kbd>
          </Botao>
        )}
        {aplicar.isError && (
          <p className="mt-2 text-xs text-papa-erro">
            A última nota não foi salva; o card voltou para o fim da fila.
          </p>
        )}
      </div>
    </section>
  );
}

/** Frente e verso do card. */
function CartaoDeRevisao({
  card,
  revelado,
}: {
  card: ReviewCard;
  revelado: boolean;
}) {
  const verbete = useDictEntry(card.card.lemma);
  const contexto = card.context;

  return (
    // Centralizado na vertical, e num bloco estreito: a frente do card é uma
    // frase só, e ancorá-la no topo de um cartão alto deixava o olho procurando
    // o conteúdo num vazio de meia tela. Com o verso aberto o conteúdo cresce e
    // o `justify-center` deixa de ter efeito sozinho — daí o `my-auto`.
    <Cartao padding="lg" className="flex flex-1 flex-col overflow-y-auto">
      <div className="my-auto w-full max-w-xl">
        {contexto?.gameName && (
          <p className="mb-4 text-[11px] tracking-wide text-papa-faint uppercase">
            {contexto.gameName}
          </p>
        )}

        {contexto ? (
          <FraseDeContexto
            frase={contexto.sentenceEn}
            forma={contexto.form}
            className="text-2xl text-papa-text"
          />
        ) : (
          <p className="font-reading text-2xl text-papa-text">
            {card.card.lemma}
          </p>
        )}

        {contexto?.screenshotPath && (
          <div className="mt-5">
            <Screenshot
              path={contexto.screenshotPath}
              alt={`Tela onde "${contexto.form}" apareceu`}
            />
          </div>
        )}

        {/* O verso não desmonta a frente: reler a frase depois de saber o
            significado é metade do aprendizado. */}
        {revelado && (
          <div className="mt-7 animate-[papa-surgir_160ms_var(--ease-papa)] border-t border-papa-border pt-6">
            <div className="flex items-baseline gap-3">
              <h3 className="text-lg font-medium text-papa-text">
                {card.card.lemma}
              </h3>
              {verbete.data?.ipa && (
                <span className="font-mono text-sm text-papa-muted">
                  /{verbete.data.ipa}/
                </span>
              )}
            </div>

            {verbete.isPending && (
              <p className="mt-3 text-sm text-papa-muted">Consultando…</p>
            )}
            {verbete.data ? (
              <ul className="mt-3 space-y-1.5">
                {acepcoesPrincipais(verbete.data, 4).map((sense, i) => (
                  <li
                    key={i}
                    className="text-sm leading-relaxed text-papa-text"
                  >
                    <span className="mr-2 text-papa-faint">
                      {classeEmPtBr(sense.pos)}
                    </span>
                    {sense.glossPt}
                  </li>
                ))}
              </ul>
            ) : (
              !verbete.isPending && (
                <p className="mt-3 text-sm text-papa-muted">
                  O dicionário não conhece esta palavra.
                </p>
              )
            )}

            {contexto?.sentencePt && (
              <p className="mt-5 font-reading text-sm leading-relaxed text-papa-muted">
                {contexto.sentencePt}
              </p>
            )}
          </div>
        )}
      </div>
    </Cartao>
  );
}

function BarraDeProgresso({
  feitas,
  total,
}: {
  feitas: number;
  total: number;
}) {
  const proporcao = total > 0 ? feitas / total : 0;
  return (
    <div>
      <div className="mb-2 flex items-baseline justify-between text-xs text-papa-muted">
        <span>
          {feitas} de {total}
        </span>
        <span className="text-papa-faint">espaço revela · 1–4 avaliam</span>
      </div>
      <div className="h-0.5 w-full rounded-full bg-papa-border">
        <div
          className="h-full rounded-full bg-papa-accent transition-[width] duration-200 ease-[var(--ease-papa)]"
          style={{ width: `${proporcao * 100}%` }}
        />
      </div>
    </div>
  );
}

function ResumoDaSessao({
  progresso,
  restamNovos,
}: {
  progresso: Progresso;
  restamNovos: number;
}) {
  const acerto = Math.round((progresso.acertos / progresso.feitas) * 100);
  const setScreen = useMainStore((s) => s.setScreen);
  return (
    <Vazio
      titulo="Fila zerada"
      acao={
        <Botao onClick={() => setScreen("estatisticas")}>
          Ver estatísticas
        </Botao>
      }
    >
      {progresso.feitas} {progresso.feitas === 1 ? "revisão" : "revisões"},{" "}
      {acerto}% de acerto.
      {restamNovos > 0 && (
        <>
          {" "}
          Ainda há {restamNovos} {restamNovos === 1 ? "palavra" : "palavras"}{" "}
          esperando a cota de amanhã.
        </>
      )}
    </Vazio>
  );
}

function NadaParaHoje({
  deckVazio,
  restamNovos,
}: {
  deckVazio: boolean;
  restamNovos: number;
}) {
  const setScreen = useMainStore((s) => s.setScreen);
  return (
    <Vazio
      titulo={deckVazio ? "Deck vazio" : "Nada para revisar"}
      acao={
        !deckVazio && (
          <Botao onClick={() => setScreen("deck")}>Abrir o deck</Botao>
        )
      }
    >
      {deckVazio
        ? "Segure o atalho de espiar durante o jogo e salve a primeira palavra."
        : restamNovos > 0
          ? "A cota de palavras novas de hoje acabou — o resto volta amanhã."
          : "Tudo em dia. Os próximos cards vencem nos próximos dias."}
    </Vazio>
  );
}

function Aviso({ texto, erro = false }: { texto: string; erro?: boolean }) {
  return (
    <p className={`text-sm ${erro ? "text-papa-erro" : "text-papa-muted"}`}>
      {texto}
    </p>
  );
}
