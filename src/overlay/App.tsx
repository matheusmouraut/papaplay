import { useQuery } from "@tanstack/react-query";

import { ping } from "../shared/api/core";

/**
 * Placeholder da janela overlay. Prova que o bundle da janela `overlay`
 * carrega separado do `main` e que o fundo transparente funciona.
 *
 * A implementação real (destaques por bbox, tooltip, popup de lookup) entra
 * depois da spike de click-through — ver docs/spikes/.
 */
export function App() {
  const core = useQuery({ queryKey: ["ping"], queryFn: ping });

  return (
    <div className="flex h-full items-start justify-center p-6">
      <div className="pointer-events-auto rounded-lg border border-papa-accent/40 bg-black/70 px-4 py-3 text-sm text-papa-text shadow-lg backdrop-blur-sm">
        <p className="font-medium">PapaPlay — overlay</p>
        <p className="mt-1 text-xs text-papa-muted">
          Core: {core.data ?? (core.isError ? "sem resposta" : "…")}
        </p>
      </div>
    </div>
  );
}
