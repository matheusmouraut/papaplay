import { useState } from "react";

import { Cartao, TituloDaTela, Vazio } from "../../shared/components/ui";
import { useStats } from "../../shared/hooks/useStats";
import type { StatsSummary } from "../../shared/types";
import { BarrasPorDia } from "../components/BarrasPorDia";

/**
 * Estatísticas (F6).
 *
 * Quatro números no topo e dois gráficos abaixo. A ordem é a das perguntas que
 * o usuário faz: "mantive o hábito?" antes de "quanto acertei?" — a sequência
 * de dias é o que sustenta a revisão espaçada, e a taxa de acerto sem hábito
 * não significa nada.
 */

const JANELAS = [
  { dias: 30, rotulo: "30 dias" },
  { dias: 90, rotulo: "90 dias" },
] as const;

export function Estatisticas() {
  const [dias, setDias] = useState<number>(JANELAS[0].dias);
  const stats = useStats(dias);

  return (
    <section className="flex h-full flex-col gap-6 overflow-y-auto pr-1">
      <TituloDaTela
        acao={
          <div className="flex gap-1">
            {JANELAS.map((janela) => (
              <button
                key={janela.dias}
                type="button"
                onClick={() => setDias(janela.dias)}
                className={`rounded-md px-2.5 py-1 text-xs transition-colors duration-150 ${
                  dias === janela.dias
                    ? "bg-papa-accent-soft text-papa-accent"
                    : "text-papa-muted hover:text-papa-text"
                }`}
              >
                {janela.rotulo}
              </button>
            ))}
          </div>
        }
      >
        Estatísticas
      </TituloDaTela>

      {stats.isError && (
        <p className="text-sm text-papa-erro">
          Os números não carregaram: {String(stats.error)}
        </p>
      )}
      {stats.isPending && <p className="text-sm text-papa-muted">Somando…</p>}
      {stats.data && <Painel dados={stats.data} dias={dias} />}
    </section>
  );
}

function Painel({ dados, dias }: { dados: StatsSummary; dias: number }) {
  if (dados.total === 0) {
    return (
      <Vazio titulo="Sem números ainda">
        Salve a primeira palavra durante o jogo e o progresso aparece aqui.
      </Vazio>
    );
  }

  return (
    <>
      <div className="grid grid-cols-4 gap-3">
        <Numero
          valor={dados.streak}
          rotulo={dados.streak === 1 ? "dia seguido" : "dias seguidos"}
          destaque={dados.streak > 0}
        />
        <Numero valor={dados.reviewedToday} rotulo="revisões hoje" />
        <Numero
          valor={
            dados.accuracy === null
              ? "—"
              : `${Math.round(dados.accuracy * 100)}%`
          }
          rotulo={`acerto em ${dias} dias`}
        />
        <Numero valor={dados.total} rotulo="palavras no deck" />
      </div>

      <div className="grid grid-cols-2 gap-3">
        <BarrasPorDia
          titulo="Palavras capturadas"
          unidade="palavra"
          dados={dados.daily.map((d) => ({ day: d.day, valor: d.created }))}
        />
        <BarrasPorDia
          titulo="Revisões"
          unidade="revisão"
          dados={dados.daily.map((d) => ({ day: d.day, valor: d.reviewed }))}
        />
      </div>

      <div className="grid grid-cols-2 gap-3">
        <EstadoDoDeck dados={dados} />
        <PorJogo dados={dados} />
      </div>
    </>
  );
}

function Numero({
  valor,
  rotulo,
  destaque = false,
}: {
  valor: number | string;
  rotulo: string;
  destaque?: boolean;
}) {
  return (
    <Cartao padding="sm">
      <p
        className={`text-2xl font-semibold tracking-tight tabular-nums ${
          destaque ? "text-papa-accent" : "text-papa-text"
        }`}
      >
        {valor}
      </p>
      <p className="mt-0.5 text-xs text-papa-muted">{rotulo}</p>
    </Cartao>
  );
}

/** Distribuição pelos estados do FSRS, em uma barra empilhada. */
function EstadoDoDeck({ dados }: { dados: StatsSummary }) {
  const faixas = [
    {
      rotulo: "Novas",
      valor: dados.states.new,
      opacidade: "bg-papa-accent/30",
    },
    {
      rotulo: "Aprendendo",
      valor: dados.states.learning + dados.states.relearning,
      opacidade: "bg-papa-accent/60",
    },
    {
      rotulo: "Em revisão",
      valor: dados.states.review,
      opacidade: "bg-papa-accent",
    },
  ];
  const total = Math.max(
    1,
    faixas.reduce((s, f) => s + f.valor, 0),
  );

  return (
    <figure className="rounded-xl border border-papa-border bg-papa-surface px-5 py-4">
      <figcaption className="flex items-baseline justify-between">
        <span className="text-sm text-papa-text">Estado do deck</span>
        {dados.dueNow > 0 && (
          <span className="text-xs text-papa-muted">
            {dados.dueNow} vencida{dados.dueNow === 1 ? "" : "s"}
          </span>
        )}
      </figcaption>

      {/* Uma medida, três degraus do mesmo acento: a ordem novas → em revisão
          é uma progressão, e progressão pede um hue só, não três. */}
      <div className="mt-4 flex h-2 gap-[2px] overflow-hidden rounded-full">
        {faixas.map((faixa) => (
          <div
            key={faixa.rotulo}
            className={faixa.opacidade}
            style={{ width: `${(faixa.valor / total) * 100}%` }}
          />
        ))}
      </div>

      <ul className="mt-4 space-y-1.5">
        {faixas.map((faixa) => (
          <li
            key={faixa.rotulo}
            className="flex items-center gap-2 text-xs text-papa-muted"
          >
            <span
              className={`h-2 w-2 shrink-0 rounded-sm ${faixa.opacidade}`}
            />
            {faixa.rotulo}
            <span className="ml-auto tabular-nums text-papa-text">
              {faixa.valor}
            </span>
          </li>
        ))}
        {dados.suspended > 0 && (
          <li className="flex items-center gap-2 border-t border-papa-border pt-1.5 text-xs text-papa-faint">
            Já sei
            <span className="ml-auto tabular-nums">{dados.suspended}</span>
          </li>
        )}
      </ul>
    </figure>
  );
}

/** Quanto vocabulário cada jogo rendeu. */
function PorJogo({ dados }: { dados: StatsSummary }) {
  const jogos = dados.byGame.slice(0, 6);
  const maximo = Math.max(1, ...jogos.map((j) => j.cards));

  return (
    <figure className="rounded-xl border border-papa-border bg-papa-surface px-5 py-4">
      <figcaption className="text-sm text-papa-text">Por jogo</figcaption>
      {jogos.length === 0 ? (
        <p className="mt-4 text-xs text-papa-muted">
          Nenhum contexto trouxe o nome do jogo.
        </p>
      ) : (
        <ul className="mt-4 space-y-2.5">
          {jogos.map((jogo) => (
            <li key={jogo.game}>
              <div className="flex items-baseline justify-between text-xs">
                <span className="truncate text-papa-text">{jogo.game}</span>
                <span className="ml-3 shrink-0 tabular-nums text-papa-muted">
                  {jogo.cards}
                </span>
              </div>
              <div className="mt-1 h-1 rounded-full bg-papa-border">
                <div
                  className="h-full rounded-full bg-papa-accent/70"
                  style={{ width: `${(jogo.cards / maximo) * 100}%` }}
                />
              </div>
            </li>
          ))}
        </ul>
      )}
    </figure>
  );
}
