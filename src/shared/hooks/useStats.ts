import { useQuery } from "@tanstack/react-query";

import { statsSummary } from "../api/core";
import type { StatsSummary } from "../types";

/**
 * Números da tela de Estatísticas.
 *
 * O deslocamento do fuso vai junto a cada chamada em vez de ser lido uma vez no
 * start: ele muda com horário de verão, e um app que fica aberto por dias
 * passaria a contar o dia errado.
 */
export function useStats(days: number) {
  return useQuery<StatsSummary>({
    queryKey: ["stats", days],
    queryFn: () =>
      statsSummary({
        now: new Date().toISOString(),
        tzOffsetMinutes: -new Date().getTimezoneOffset(),
        days,
      }),
  });
}
