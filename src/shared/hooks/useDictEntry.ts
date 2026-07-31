import { useQuery } from "@tanstack/react-query";

import { dictLookup } from "../api/core";
import type { DictEntry } from "../types";

/**
 * Verbete de uma palavra, com cache por palavra.
 *
 * O dicionário é um SQLite read-only embarcado: o conteúdo nunca muda enquanto
 * o app roda, então a resposta pode ficar em cache para sempre. Isso importa
 * porque o mouse passa pela mesma palavra várias vezes numa consulta só, e o
 * orçamento do tooltip é de 300 ms (doc 03).
 */
export function useDictEntry(palavra: string | null) {
  return useQuery<DictEntry | null>({
    queryKey: ["dict", palavra],
    queryFn: () => dictLookup(palavra ?? ""),
    enabled: palavra !== null && palavra.length > 0,
    staleTime: Infinity,
    gcTime: Infinity,
  });
}
