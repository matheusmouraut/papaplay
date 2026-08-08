import type { ReactElement } from "react";

import logo from "../assets/logo.svg";
import { usePreferences } from "../shared/hooks/usePreferences";
import { useReviewQueue } from "../shared/hooks/useReview";
import { useShortcuts } from "../shared/hooks/useShortcuts";
import { Configuracoes } from "./screens/Configuracoes";
import { Deck } from "./screens/Deck";
import { Estatisticas } from "./screens/Estatisticas";
import { Onboarding } from "./screens/Onboarding";
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

  // A mesma consulta que a tela Revisar usa: o react-query serve as duas do
  // mesmo cache, então o número no menu não custa uma segunda ida ao banco.
  const preferencias = usePreferences();
  const fila = useReviewQueue(preferencias.data?.newPerDay);
  const pendentes = fila.data?.cards.length ?? 0;

  const atalhos = useShortcuts();

  // Enquanto as preferências não chegaram não dá para saber se o wizard já foi
  // feito; mostrar a janela e trocá-la pelo wizard um instante depois seria
  // pior do que esperar o disco por 20 ms.
  if (preferencias.isPending) return <div className="h-full bg-papa-bg" />;
  if (!preferencias.data?.onboardingDone) return <Onboarding />;

  return (
    <div className="flex h-full bg-papa-bg text-papa-text">
      <nav className="flex w-56 shrink-0 flex-col gap-0.5 border-r border-papa-border px-3 py-5">
        <h1 className="mb-5 flex items-center gap-2 px-3">
          <img
            src={logo}
            alt=""
            width={22}
            height={22}
            className="rounded-md"
          />
          <span className="font-reading text-[17px] tracking-tight text-papa-text">
            PapaPlay
          </span>
        </h1>
        {NAV.map((item) => (
          <button
            key={item.id}
            type="button"
            onClick={() => setScreen(item.id)}
            className={`flex items-center rounded-md px-3 py-1.5 text-left text-sm transition-colors duration-150 ${
              screen === item.id
                ? "bg-papa-accent-soft font-medium text-papa-accent"
                : "text-papa-muted hover:bg-papa-raised hover:text-papa-text"
            }`}
          >
            {item.label}
            {/* O único número no menu, e ele é uma chamada para a ação: o que
                faz o app valer a pena abrir é ter algo para revisar. */}
            {item.id === "revisar" && pendentes > 0 && (
              <span className="ml-auto text-xs tabular-nums text-papa-accent">
                {pendentes}
              </span>
            )}
          </button>
        ))}

        <p className="mt-auto px-3 text-[11px] leading-relaxed text-papa-faint">
          Segure{" "}
          <kbd className="font-mono">{atalhos.data?.lookup ?? "Alt+X"}</kbd>{" "}
          durante o jogo para espiar uma palavra.
        </p>
      </nav>

      <main className="flex-1 overflow-hidden px-10 py-8">
        <Current />
      </main>
    </div>
  );
}
