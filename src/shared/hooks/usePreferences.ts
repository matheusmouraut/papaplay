import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";

import { settingsGetPreferences, settingsSetPreferences } from "../api/core";
import type { Preferences } from "../types";

/** Preferências de estudo e de primeira execução (F6/F8). */

const RAIZ = ["settings", "preferences"] as const;

export function usePreferences() {
  return useQuery<Preferences>({
    queryKey: RAIZ,
    queryFn: settingsGetPreferences,
  });
}

export function useSetPreferences() {
  const cache = useQueryClient();
  return useMutation<Preferences, unknown, Preferences>({
    mutationFn: settingsSetPreferences,
    // O core devolve o valor já limitado (1..200): escrever a resposta no cache,
    // e não o que foi enviado, evita a tela mostrar um número que o banco
    // recusou.
    onSuccess: (preferencias) => cache.setQueryData(RAIZ, preferencias),
  });
}
