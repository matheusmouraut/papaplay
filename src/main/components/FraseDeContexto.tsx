import { destacarForma } from "../../shared/text/destaque";

/**
 * A frase onde a palavra foi encontrada, com a palavra estudada marcada.
 *
 * O destaque é peso e cor, não caixa nem fundo: a frase precisa continuar
 * legível como frase — é ela que a revisão pede para o usuário entender (F7).
 */
export function FraseDeContexto({
  frase,
  forma,
  className = "",
}: {
  frase: string;
  forma: string;
  className?: string;
}) {
  return (
    <p className={`font-reading leading-relaxed ${className}`}>
      {destacarForma(frase, forma).map((pedaco, i) =>
        pedaco.destaque ? (
          <strong key={i} className="font-semibold text-papa-accent">
            {pedaco.texto}
          </strong>
        ) : (
          <span key={i}>{pedaco.texto}</span>
        ),
      )}
    </p>
  );
}
