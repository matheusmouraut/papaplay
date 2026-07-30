import type { ReactElement } from "react";

import { Configuracoes } from "./screens/Configuracoes";
import { Deck } from "./screens/Deck";
import { Estatisticas } from "./screens/Estatisticas";
import { Revisar } from "./screens/Revisar";
import { useMainStore, type Screen } from "./store";

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
      <nav className="flex w-52 shrink-0 flex-col gap-1 border-r border-papa-border bg-papa-surface p-3">
        <h1 className="mb-4 px-2 text-lg font-semibold tracking-tight">
          PapaPlay
        </h1>
        {NAV.map((item) => (
          <button
            key={item.id}
            type="button"
            onClick={() => setScreen(item.id)}
            className={`rounded-md px-3 py-2 text-left text-sm transition-colors ${
              screen === item.id
                ? "bg-papa-accent/15 text-papa-accent"
                : "text-papa-muted hover:bg-white/5 hover:text-papa-text"
            }`}
          >
            {item.label}
          </button>
        ))}
      </nav>

      <main className="flex-1 overflow-auto p-8">
        <Current />
      </main>
    </div>
  );
}
