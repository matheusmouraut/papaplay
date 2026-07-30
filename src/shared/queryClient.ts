import { QueryClient } from "@tanstack/react-query";

/**
 * Cliente compartilhado. Tudo aqui e local (SQLite/OCR), entao nao ha
 * refetch por foco de janela nem retry agressivo — so custa CPU durante o jogo.
 */
export function createQueryClient(): QueryClient {
  return new QueryClient({
    defaultOptions: {
      queries: {
        retry: false,
        refetchOnWindowFocus: false,
        staleTime: 30_000,
      },
    },
  });
}
