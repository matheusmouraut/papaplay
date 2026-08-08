import { useState } from "react";

/**
 * Série diária como barras — uma medida por gráfico.
 *
 * Duas medidas viram dois gráficos empilhados (small multiples) em vez de duas
 * cores num gráfico só: a linguagem visual tem um acento só, e ele significa
 * "seu trabalho" (F7). Colorir "capturadas" e "revisadas" com hues diferentes
 * gastaria a única cor com significado do produto em decoração.
 *
 * Sem biblioteca de gráfico: são 30 divs. Arrastar uma dependência de ~50 kB
 * para desenhar retângulos contraria o "aplicação leve".
 */

export interface BarraDoDia {
  /** `YYYY-MM-DD` no fuso local. */
  day: string;
  valor: number;
}

/** "2026-08-08" → "8 ago". Sem `Date` para não reinterpretar o fuso. */
function rotuloCurto(dia: string): string {
  const MESES = [
    "jan",
    "fev",
    "mar",
    "abr",
    "mai",
    "jun",
    "jul",
    "ago",
    "set",
    "out",
    "nov",
    "dez",
  ];
  const [, mes, data] = dia.split("-");
  return `${Number(data)} ${MESES[Number(mes) - 1] ?? ""}`;
}

export function BarrasPorDia({
  titulo,
  dados,
  unidade,
}: {
  titulo: string;
  dados: BarraDoDia[];
  /** Singular; o plural sai do próprio número ("1 card"/"3 cards"). */
  unidade: string;
}) {
  const [ativo, setAtivo] = useState<number | null>(null);
  // Escala própria por gráfico: as duas medidas têm ordens de grandeza
  // diferentes (revisa-se muito mais do que se captura), e forçar uma escala
  // comum achataria a menor até virar uma linha.
  const maximo = Math.max(1, ...dados.map((d) => d.valor));
  const emFoco = ativo !== null ? dados[ativo] : null;
  const total = dados.reduce((soma, d) => soma + d.valor, 0);

  return (
    <figure className="rounded-xl border border-papa-border bg-papa-surface px-5 py-4">
      <figcaption className="flex items-baseline justify-between">
        <span className="text-sm text-papa-text">{titulo}</span>
        {/* O valor sob o cursor substitui o total: um número de cada vez. */}
        <span className="text-xs text-papa-muted">
          {emFoco
            ? `${rotuloCurto(emFoco.day)} · ${emFoco.valor} ${unidade}${emFoco.valor === 1 ? "" : "s"}`
            : `${total} no período`}
        </span>
      </figcaption>

      <div
        className="mt-4 flex h-24 items-end gap-[2px]"
        onMouseLeave={() => setAtivo(null)}
      >
        {dados.map((ponto, i) => (
          <div
            key={ponto.day}
            onMouseEnter={() => setAtivo(i)}
            // A área sensível é a coluna inteira, e não a barra: num dia sem
            // nada a barra tem altura zero e não haveria o que apontar.
            className="flex h-full flex-1 cursor-default items-end"
          >
            <div
              className={`w-full rounded-t-[4px] transition-colors duration-150 ${
                ativo === i ? "bg-papa-accent" : "bg-papa-accent/60"
              }`}
              style={{
                height: `${(ponto.valor / maximo) * 100}%`,
                // Um dia com pouquíssimo trabalho ainda precisa aparecer;
                // zero continua zero.
                minHeight: ponto.valor > 0 ? "2px" : "0",
              }}
            />
          </div>
        ))}
      </div>

      <div className="mt-2 flex justify-between text-[11px] text-papa-faint">
        <span>{dados.length > 0 ? rotuloCurto(dados[0].day) : ""}</span>
        <span>
          {dados.length > 0 ? rotuloCurto(dados[dados.length - 1].day) : ""}
        </span>
      </div>
    </figure>
  );
}
