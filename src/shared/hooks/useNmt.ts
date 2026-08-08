import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { listen } from "@tauri-apps/api/event";
import { useEffect, useState } from "react";

import { nmtInstall, nmtStatus } from "../api/core";
import type { NmtProgress, NmtStatus } from "../types";

/**
 * O tradutor de frases, que não vem no instalador.
 *
 * Ele é 332 MB dos 390 MB de recursos do app. Deixá-lo fora reduz o instalador
 * para ~55 MB e evita reenviar os mesmos 332 MB a cada atualização — ver
 * `src-tauri/src/setup.rs`.
 */

const RAIZ = ["nmt"] as const;

export function useNmtStatus() {
  return useQuery<NmtStatus>({ queryKey: RAIZ, queryFn: nmtStatus });
}

/**
 * Dispara o download e acompanha o progresso.
 *
 * O progresso vem por evento do core, e não pelo retorno da promessa: são
 * ~1300 atualizações ao longo de minutos, e a promessa só resolve no fim.
 */
export function useNmtInstall() {
  const cache = useQueryClient();
  const [progresso, setProgresso] = useState<NmtProgress | null>(null);

  useEffect(() => {
    const inscricao = listen<NmtProgress>("setup://nmt", (evento) =>
      setProgresso(evento.payload),
    );
    return () => {
      inscricao.then((cancelar) => cancelar());
    };
  }, []);

  const instalar = useMutation<NmtStatus, unknown, void>({
    mutationFn: nmtInstall,
    onSuccess: (status) => {
      setProgresso(null);
      cache.setQueryData(RAIZ, status);
    },
  });

  return { instalar, progresso };
}
