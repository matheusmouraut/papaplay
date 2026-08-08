import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";

import { deckCardStatus, deckSaveCard } from "../api/core";
import { newCardFields } from "../srs";
import type { CardSummary, SaveCardInput } from "../types";

/** Chave do cache. Por lema, que é a identidade do card no deck. */
function chave(lemma: string | null) {
  return ["card", lemma] as const;
}

/**
 * Diz se a palavra já está no deck.
 *
 * Sem `staleTime` infinito, ao contrário do dicionário: o deck muda enquanto o
 * app roda, e um card excluído na janela principal não pode continuar
 * aparecendo como "salvo" no overlay.
 */
export function useCardStatus(lemma: string | null) {
  return useQuery<CardSummary | null>({
    queryKey: chave(lemma),
    queryFn: () => deckCardStatus(lemma ?? ""),
    enabled: lemma !== null && lemma.length > 0,
  });
}

/**
 * Salva a palavra sob o cursor no deck.
 *
 * O estado inicial do FSRS é montado aqui, pelo wrapper — o core não inventa
 * agendamento (regra inviolável #4). Só o card **novo** usa esse estado:
 * reencontrar a palavra num jogo apenas anexa contexto.
 */
export function useSaveCard() {
  const cache = useQueryClient();
  return useMutation<CardSummary, unknown, Omit<SaveCardInput, "fsrs">>({
    mutationFn: (entrada) =>
      deckSaveCard({ ...entrada, fsrs: newCardFields() }),
    // A resposta já é o estado novo do card: escrever direto no cache evita um
    // ida-e-volta e o piscar do botão entre "salvando" e "salvo".
    onSuccess: (resumo) => cache.setQueryData(chave(resumo.lemma), resumo),
  });
}
