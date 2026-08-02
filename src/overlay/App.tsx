import { Card } from "./components/Card";
import { Tooltip } from "./components/Tooltip";
import { usePeek } from "./usePeek";

/**
 * Janela overlay.
 *
 * Em repouso não desenha absolutamente nada — nem marca de canto, nem borda,
 * nem indicador. A tela é do jogo. Segurar `Alt+X` traz o tooltip da palavra
 * sob o cursor; clicar abre o card.
 *
 * Quem decide o que está sob o cursor é o core (`src-tauri/src/peek.rs`): em
 * click-through o webview não recebe mouse, então esta janela é uma superfície
 * de desenho, não uma tela interativa.
 */
export function App() {
  const { state, focus, erro } = usePeek();

  return (
    <div className="relative h-full w-full">
      {erro && <Aviso texto={erro} />}

      {state === "peek" && focus && <Tooltip foco={focus} />}
      {state === "card" && focus && <Card foco={focus} />}

      {/* Espiando sem palavra sob o cursor: uma dica discreta, e só depois de
          o core ter lido a tela. Sem isso, apontar para o céu do jogo pareceria
          a ferramenta ter travado. */}
      {state === "peek" && !focus && !erro && <Rastro />}
    </div>
  );
}

function Rastro() {
  return (
    <div className="papa-surge absolute bottom-8 left-1/2 -translate-x-1/2">
      <div className="papa-vidro rounded-full px-3 py-1.5 text-xs text-papa-muted">
        aponte para uma palavra
      </div>
    </div>
  );
}

/**
 * Falhas do core (modelos de OCR ausentes, captura recusada).
 *
 * Fica visível sem depender de nenhum atalho: sem isto, uma instalação sem os
 * modelos baixados só pareceria não funcionar.
 */
function Aviso({ texto }: { texto: string }) {
  return (
    <div className="papa-surge absolute bottom-8 left-1/2 max-w-xl -translate-x-1/2">
      <div className="papa-vidro rounded-lg border-red-500/30 px-4 py-2.5 text-xs leading-relaxed text-red-200/90">
        {texto}
      </div>
    </div>
  );
}
