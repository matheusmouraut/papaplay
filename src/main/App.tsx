import type { ReactElement } from "react";

import { Configuracoes } from "./screens/Configuracoes";
import { Deck } from "./screens/Deck";
import { Estatisticas } from "./screens/Estatisticas";
import { Revisar } from "./screens/Revisar";
import { useMainStore, type Screen } from "./store";

/**
 * Janela principal.
 *
 * A navegação fica numa coluna estreita e silenciosa: o produto é o conteúdo
 * (o deck, a revisão), não o próprio app. Referência de F7 — Notion/Linear.
 */

const NAV: { id: Screen; label: string }[] = [
  { id: "revisar", label: "Revisar" },
  { id: "deck", label: "Deck" },
  { id: "estatisticas", label: "Estatísticas" },
  { id: "configuracoes", label: "Configurações" },
];

const SCREENS: Record<Screen, () => ReactElement> = {
  revisar: Revisar,
  deck: Deck,
  estatisticas: Estatisticas,
  configuracoes: Configuracoes,
};

export function App() {
  const screen = useMainStore((s) => s.screen);
  const setScreen = useMainStore((s) => s.setScreen);
  const Current = SCREENS[screen];

  return (
    <div className="flex h-full bg-papa-bg text-papa-text">
      <nav className="flex w-56 shrink-0 flex-col gap-0.5 border-r border-papa-border px-3 py-5">
        <h1 className="mb-5 px-3 text-[13px] font-medium tracking-wide text-papa-faint uppercase">
          PapaPlay
        </h1>
        {NAV.map((item) => (
          <button
            key={item.id}
            type="button"
            onClick={() => setScreen(item.id)}
            className={`rounded-md px-3 py-1.5 text-left text-sm transition-colors duration-150 ${
              screen === item.id
                ? "bg-white/[0.07] text-papa-text"
                : "text-papa-muted hover:bg-white/[0.04] hover:text-papa-text"
            }`}
          >
            {item.label}
          </button>
        ))}

        <p className="mt-auto px-3 text-[11px] leading-relaxed text-papa-faint">
          Segure <kbd className="font-mono">Alt+X</kbd> durante o jogo para
          espiar uma palavra.
        </p>
      </nav>

      <main className="flex-1 overflow-hidden px-10 py-8">
        <Current />
      </main>
    </div>
  );
}
