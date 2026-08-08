import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";

import {
  settingsGetShortcuts,
  settingsResetShortcuts,
  settingsSetShortcuts,
} from "../api/core";
import type { Shortcuts } from "../types";

/** Consultas e mutações dos atalhos configuráveis (F6). */

const RAIZ = ["settings", "shortcuts"] as const;

export function useShortcuts() {
  return useQuery<Shortcuts>({
    queryKey: RAIZ,
    queryFn: settingsGetShortcuts,
  });
}

export function useSetShortcuts() {
  const cache = useQueryClient();
  return useMutation<Shortcuts, unknown, Shortcuts>({
    mutationFn: settingsSetShortcuts,
    onSuccess: (shortcuts) => cache.setQueryData(RAIZ, shortcuts),
  });
}

export function useResetShortcuts() {
  const cache = useQueryClient();
  return useMutation<Shortcuts, unknown, void>({
    mutationFn: settingsResetShortcuts,
    onSuccess: (shortcuts) => cache.setQueryData(RAIZ, shortcuts),
  });
}
