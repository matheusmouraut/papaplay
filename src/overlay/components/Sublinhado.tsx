import type { OverlayRect } from "../../shared/types";

/**
 * A marca sob a palavra espiada.
 *
 * Sublinhado, e não caixa: a F1 proíbe desenhar molduras em volta das palavras
 * — foi o que fez a primeira versão parecer um painel de debug colado no jogo.
 * Uma linha de 2px sob a palavra diz "é esta" sem tampar nada da cena.
 *
 * É a **única** palavra marcada na tela. As outras que o OCR leu ficam
 * invisíveis até o cursor passar por elas.
 */
export function Sublinhado({ rect }: { rect: OverlayRect }) {
  return (
    <div
      className="papa-surge pointer-events-none absolute rounded-full bg-papa-accent"
      style={{
        left: rect.x,
        // Encostado na base da caixa da palavra, um pixel abaixo: dentro da
        // caixa a linha cortaria as descidas de "g", "p", "y".
        top: rect.y + rect.h + 1,
        width: rect.w,
        height: 2,
        boxShadow:
          "0 0 8px color-mix(in srgb, var(--color-papa-accent) 50%, transparent)",
      }}
    />
  );
}
