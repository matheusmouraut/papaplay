import { useQuery } from "@tanstack/react-query";

import { translateRun } from "../api/core";

/**
 * Tradução de uma frase, com cache por frase.
 *
 * Cabe em cache eterno pelo mesmo motivo do dicionário: a decodificação é
 * gulosa e o modelo é fixo, então a mesma frase sempre devolve o mesmo
 * português. Isso importa porque a linha de diálogo sob o cursor é a mesma para
 * todas as palavras dela — sem cache, cada palavra clicada pagaria a tradução
 * de novo.
 *
 * Diferente do verbete, esta consulta **não** roda no hover: são dezenas de ms
 * por frase e ~800 ms de carga do modelo na primeira do Alt+X, muito acima do
 * orçamento de 300 ms do tooltip (doc 03). Só a palavra fixada por clique pede
 * tradução.
 */
export function useSentenceTranslation(frase: string | null) {
  return useQuery<string>({
    queryKey: ["translate", frase],
    queryFn: () => translateRun(frase ?? ""),
    enabled: frase !== null && frase.trim().length > 0,
    staleTime: Infinity,
    gcTime: Infinity,
    // Uma frase que falhou é quase sempre modelo ausente ou lixo de OCR longo
    // demais: repetir só atrasa a mensagem de erro.
    retry: false,
  });
}
