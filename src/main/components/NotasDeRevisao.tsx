import type { DeckCard } from "../../shared/types";
import {
  intervalLabels,
  RATING_LABELS,
  RATINGS,
  type Grade,
} from "../../shared/srs";

/**
 * Os quatro botões de nota, com o intervalo que cada um promete.
 *
 * A ordem é a do FSRS (Errei → Fácil) e o número do atalho é a posição: quem
 * revisa todo dia acaba respondendo só pelo teclado, e a correspondência
 * tecla ↔ posição é o que torna isso automático.
 */
export function NotasDeRevisao({
  card,
  onNota,
  desabilitado,
}: {
  card: DeckCard;
  onNota: (nota: Grade) => void;
  desabilitado: boolean;
}) {
  // Recalculado a cada card: o intervalo depende do estado atual dele.
  const intervalos = intervalLabels(card);

  return (
    <div className="grid grid-cols-4 gap-2">
      {RATINGS.map((nota, i) => (
        <button
          key={nota}
          type="button"
          disabled={desabilitado}
          onClick={() => onNota(nota)}
          className="group flex flex-col items-center gap-0.5 rounded-lg border border-papa-border bg-papa-surface px-3 py-2.5 transition-colors duration-150 hover:border-papa-border-strong hover:bg-papa-raised disabled:opacity-40"
        >
          <span className="flex items-baseline gap-1.5 text-sm text-papa-text">
            <span className="font-mono text-[11px] text-papa-faint">
              {i + 1}
            </span>
            {RATING_LABELS[nota]}
          </span>
          <span className="text-[11px] text-papa-muted">
            {intervalos[nota]}
          </span>
        </button>
      ))}
    </div>
  );
}
