import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";

import { reviewApply, reviewQueue } from "../api/core";
import type { ReviewInput, ReviewQueue } from "../types";

/**
 * A fila do dia (F5).
 *
 * O `now` e o `dayStart` são calculados aqui, e não no core, porque só a UI
 * sabe o fuso do usuário: "15 novos por dia" é uma pergunta sobre o dia local,
 * e o banco guarda tudo em UTC.
 */

const RAIZ = ["review"] as const;

/** Meia-noite local de hoje, em ISO-8601 UTC. */
function inicioDoDiaLocal(agora: Date): string {
  const meiaNoite = new Date(agora);
  meiaNoite.setHours(0, 0, 0, 0);
  return meiaNoite.toISOString();
}

export function useReviewQueue(newLimit: number | undefined) {
  return useQuery<ReviewQueue>({
    queryKey: [...RAIZ, "queue", newLimit],
    queryFn: () => {
      const agora = new Date();
      return reviewQueue({
        now: agora.toISOString(),
        dayStart: inicioDoDiaLocal(agora),
        newLimit: newLimit ?? 0,
      });
    },
    // Sem a cota não dá para montar a fila: pedir com 0 traria uma fila sem
    // nenhum card novo e a tela mostraria "nada para hoje" por um instante.
    enabled: newLimit !== undefined,
    // A fila é montada uma vez por sessão e depois avança na memória. Refazer a
    // consulta a cada nota reordenaria os cards no meio da revisão.
    staleTime: Infinity,
    gcTime: 0,
  });
}

/**
 * Grava a nota do usuário.
 *
 * Não invalida a fila de propósito (ver acima): o que invalida é o deck e as
 * estatísticas, que passaram a mostrar números velhos.
 */
export function useApplyReview() {
  const cache = useQueryClient();
  return useMutation<void, unknown, ReviewInput>({
    mutationFn: reviewApply,
    onSuccess: () => {
      cache.invalidateQueries({ queryKey: ["deck"] });
      cache.invalidateQueries({ queryKey: ["stats"] });
    },
  });
}

/** Descarta a fila em cache — usado ao começar uma sessão nova. */
export function useResetQueue() {
  const cache = useQueryClient();
  return () => cache.invalidateQueries({ queryKey: RAIZ });
}
