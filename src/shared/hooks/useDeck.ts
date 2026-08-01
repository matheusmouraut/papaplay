import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";

import {
  deckCardDetail,
  deckDeleteCard,
  deckGames,
  deckListCards,
  deckSetSuspended,
  deckUpdateContext,
} from "../api/core";
import type { CardDetail, CardQuery, CardRow } from "../types";

/**
 * Consultas da tela Deck.
 *
 * Toda mutação invalida `["deck"]` inteiro em vez de remendar o cache: a lista
 * depende de filtros, ordenação e contagens que o SQL calcula: reproduzir isso
 * no cliente é reimplementar a consulta — e errar nela em silêncio.
 */

const RAIZ = ["deck"] as const;

export function useDeckCards(query: CardQuery) {
  return useQuery<CardRow[]>({
    queryKey: [...RAIZ, "cards", query],
    queryFn: () => deckListCards(query),
    // Mantém a lista anterior visível enquanto a busca nova chega: sem isso a
    // tela pisca em branco a cada tecla digitada no campo de busca.
    placeholderData: (anterior) => anterior,
  });
}

export function useCardDetail(cardId: number | null) {
  return useQuery<CardDetail | null>({
    queryKey: [...RAIZ, "detail", cardId],
    queryFn: () => deckCardDetail(cardId ?? 0),
    enabled: cardId !== null,
  });
}

export function useDeckGames() {
  return useQuery<string[]>({
    queryKey: [...RAIZ, "games"],
    queryFn: deckGames,
  });
}

function useMutacaoDoDeck<T>(fn: (entrada: T) => Promise<void>) {
  const cache = useQueryClient();
  return useMutation<void, unknown, T>({
    mutationFn: fn,
    onSuccess: () => cache.invalidateQueries({ queryKey: RAIZ }),
  });
}

export function useSuspendCard() {
  return useMutacaoDoDeck(
    ({ id, suspended }: { id: number; suspended: boolean }) =>
      deckSetSuspended(id, suspended),
  );
}

export function useDeleteCard() {
  return useMutacaoDoDeck((id: number) => deckDeleteCard(id));
}

export function useUpdateContext() {
  return useMutacaoDoDeck(
    ({ id, sentencePt }: { id: number; sentencePt: string | null }) =>
      deckUpdateContext(id, sentencePt),
  );
}
