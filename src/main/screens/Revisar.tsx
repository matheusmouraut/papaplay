import { useQuery } from "@tanstack/react-query";

import { ping } from "../../shared/api/core";
import { Placeholder } from "../../shared/components/Placeholder";

export function Revisar() {
  const core = useQuery({ queryKey: ["ping"], queryFn: ping });

  return (
    <Placeholder
      title="Revisar"
      description="Fila de revisão com FSRS: card com o lema, a frase de contexto e o screenshot do jogo."
      spec="docs/03-funcionalidades.md"
    >
      <div className="rounded-lg border border-papa-border bg-papa-surface px-4 py-3 text-sm">
        <span className="text-papa-muted">Core Rust: </span>
        {core.isPending && <span>verificando…</span>}
        {core.isError && (
          <span className="text-red-400">
            sem resposta ({core.error.message})
          </span>
        )}
        {core.isSuccess && (
          <span className="text-papa-accent">{core.data}</span>
        )}
      </div>
    </Placeholder>
  );
}
