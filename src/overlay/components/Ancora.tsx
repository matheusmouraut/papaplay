import { useLayoutEffect, useRef, useState, type ReactNode } from "react";

import type { OverlayRect } from "../../shared/types";

/**
 * Posiciona um bloco ancorado a uma palavra na tela.
 *
 * Mede o conteúdo depois de montado em vez de estimar a altura: o card muda de
 * tamanho conforme o verbete (2 acepções ou 4, com tradução ou sem), e altura
 * chutada é o que faz o balão da última linha de diálogo sair da tela — que é
 * justamente onde o texto dos jogos fica.
 */

/** Distância entre a palavra e o bloco, em px. */
const RESPIRO = 10;

/** Folga mínima até a borda da tela. */
const MARGEM = 12;

export function Ancora({
  rect,
  children,
}: {
  /** Palavra à qual ancorar, em pixels lógicos da overlay. */
  rect: OverlayRect;
  children: ReactNode;
}) {
  const ref = useRef<HTMLDivElement>(null);
  const [medida, setMedida] = useState({ w: 0, h: 0 });

  useLayoutEffect(() => {
    const alvo = ref.current;
    if (!alvo) return;
    const observer = new ResizeObserver(([entrada]) => {
      const { width, height } = entrada.contentRect;
      setMedida({ w: width, h: height });
    });
    observer.observe(alvo);
    return () => observer.disconnect();
  }, []);

  const abaixo = rect.y + rect.h + RESPIRO;
  const cabeEmbaixo = abaixo + medida.h + MARGEM <= window.innerHeight;
  const top = cabeEmbaixo ? abaixo : rect.y - medida.h - RESPIRO;

  // Alinhado pelo começo da palavra, empurrado para dentro da tela — ler
  // começa pela esquerda, então o bloco também.
  const left = Math.min(
    Math.max(rect.x, MARGEM),
    Math.max(MARGEM, window.innerWidth - medida.w - MARGEM),
  );

  return (
    <div
      ref={ref}
      className="papa-surge absolute w-max"
      // Enquanto a medida não chegou o bloco fica invisível: um frame no lugar
      // errado é mais perceptível do que um frame ausente.
      style={{ left, top, opacity: medida.h === 0 ? 0 : 1 }}
    >
      {children}
    </div>
  );
}
