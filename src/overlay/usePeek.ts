import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";

import { peekState } from "../shared/api/core";
import type { PeekFocus, PeekState } from "../shared/types";

/**
 * A sessão de espiada, vista pela overlay.
 *
 * Tudo vem do core por evento, e não de comandos: quem manda no ciclo é o laço
 * que segue o cursor (`src-tauri/src/peek.rs`), porque em click-through o
 * webview não recebe mouse nenhum. A UI aqui é só a superfície que desenha o
 * que o core encontrou.
 */
export function usePeek() {
  const [state, setState] = useState<PeekState>("idle");
  const [focus, setFocus] = useState<PeekFocus | null>(null);
  const [erro, setErro] = useState<string | null>(null);

  useEffect(() => {
    const inscricoes = [
      listen<PeekState>("peek://state", (evento) => {
        setState(evento.payload);
        if (evento.payload === "idle") {
          setFocus(null);
          setErro(null);
        }
      }),
      listen<PeekFocus | null>("peek://focus", (evento) =>
        setFocus(evento.payload),
      ),
      // O card congela a palavra: o `peek://focus` continua chegando enquanto o
      // laço não morre, e sem isso o conteúdo do card trocaria embaixo do
      // cursor no caminho até ele.
      listen<PeekFocus>("peek://card", (evento) => setFocus(evento.payload)),
      listen<string>("peek://error", (evento) => setErro(evento.payload)),
    ];

    // A janela pode recarregar (hot reload em dev) no meio de uma espiada.
    peekState()
      .then(setState)
      .catch(() => undefined);

    return () => {
      for (const inscricao of inscricoes) {
        void inscricao.then((off) => off());
      }
    };
  }, []);

  return { state, focus, erro };
}
