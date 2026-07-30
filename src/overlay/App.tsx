import { useCallback, useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";

import {
  overlayBench,
  overlaySetMode,
  overlayStatus,
  overlayToggle,
} from "../shared/api/core";
import type { OverlayBenchReport, OverlayModeChange } from "../shared/types";

/**
 * Janela overlay — versão da spike 01 (`docs/spikes/spike-01-overlay.md`).
 *
 * Serve para responder três perguntas com o jogo rodando por baixo:
 * 1. o fundo é mesmo transparente e o always-on-top segura sobre o jogo?
 * 2. em modo passivo o clique atravessa para o jogo?
 * 3. quanto custa alternar entre passivo e lookup?
 *
 * A implementação real (destaques por bbox, tooltip, popup de lookup) entra
 * depois que a spike fechar em GO.
 */

const BENCH_ITERATIONS = 50;

function ms(microseconds: number): string {
  return `${(microseconds / 1000).toFixed(1)} ms`;
}

/** Marcas nos 4 cantos: se as 4 aparecerem coladas nas bordas do monitor, o
 *  posicionamento acertou o monitor inteiro (checagem de DPI/multi-monitor). */
function CornerMarks() {
  const corners = [
    "left-0 top-0 border-l-4 border-t-4",
    "right-0 top-0 border-r-4 border-t-4",
    "left-0 bottom-0 border-l-4 border-b-4",
    "right-0 bottom-0 border-r-4 border-b-4",
  ];
  return (
    <>
      {corners.map((position) => (
        <div
          key={position}
          className={`absolute h-10 w-10 border-papa-accent/80 ${position}`}
        />
      ))}
    </>
  );
}

export function App() {
  const [mode, setMode] = useState<OverlayModeChange | null>(null);
  const [interactive, setInteractive] = useState(false);
  const [clicks, setClicks] = useState(0);
  const [bench, setBench] = useState<OverlayBenchReport | null>(null);
  const [benchRunning, setBenchRunning] = useState(false);
  const [erro, setErro] = useState<string | null>(null);

  // O modo também muda por hotkey (Alt+X / Esc), fora do controle da UI —
  // por isso o estado vem do evento do core, não do retorno do comando.
  useEffect(() => {
    const unlisten = listen<OverlayModeChange>("overlay://mode", (event) => {
      setMode(event.payload);
      setInteractive(event.payload.interactive);
    });
    overlayStatus()
      .then((status) => setInteractive(status.interactive))
      .catch((e: unknown) => setErro(String(e)));
    return () => {
      void unlisten.then((off) => off());
    };
  }, []);

  const rodarBench = useCallback(async () => {
    setBenchRunning(true);
    setErro(null);
    try {
      setBench(await overlayBench(BENCH_ITERATIONS));
    } catch (e: unknown) {
      setErro(String(e));
    } finally {
      setBenchRunning(false);
    }
  }, []);

  return (
    <div className="relative h-full w-full">
      <CornerMarks />

      {/* Retângulo de teste: precisa deixar o jogo visível por baixo. */}
      <div className="absolute left-1/2 top-1/2 h-64 w-[32rem] -translate-x-1/2 -translate-y-1/2 rounded-xl border-2 border-papa-accent/70 bg-papa-accent/20 backdrop-blur-[2px]">
        <div className="flex h-full flex-col items-center justify-center gap-3">
          <p className="text-sm text-papa-text/90">
            Retângulo de teste — o jogo deve aparecer por baixo
          </p>
          <button
            type="button"
            onClick={() => setClicks((n) => n + 1)}
            className="rounded-md bg-papa-accent px-4 py-2 text-sm font-medium text-black shadow-lg hover:brightness-110"
          >
            Cliquei {clicks}×
          </button>
          <p className="text-xs text-papa-muted">
            Em modo passivo este botão não deve reagir — o clique vai para o
            jogo
          </p>
        </div>
      </div>

      {/* HUD com os números da spike. */}
      <div className="absolute left-6 top-6 w-80 rounded-lg border border-papa-border bg-black/80 p-4 text-xs text-papa-text shadow-2xl">
        <div className="mb-3 flex items-center justify-between">
          <span className="text-sm font-semibold">
            PapaPlay — spike overlay
          </span>
          <span
            className={`rounded px-2 py-0.5 text-[11px] font-medium ${
              interactive
                ? "bg-papa-accent/20 text-papa-accent"
                : "bg-white/10 text-papa-muted"
            }`}
          >
            {interactive ? "LOOKUP" : "PASSIVO"}
          </span>
        </div>

        <dl className="space-y-1">
          <Linha rotulo="Alternar" valor="Alt+X · Esc volta p/ passivo" />
          <Linha
            rotulo="Última troca"
            valor={mode ? ms(mode.elapsedUs) : "—"}
          />
          <Linha rotulo="Janela em foco" valor={mode?.windowTitle ?? "—"} />
          <Linha
            rotulo="Monitor"
            valor={
              mode?.monitor
                ? `${mode.monitor.width}×${mode.monitor.height} @ ${mode.monitor.x},${mode.monitor.y}`
                : "—"
            }
          />
          <Linha
            rotulo="Escala DPI"
            valor={mode ? `${mode.scaleFactor}×` : "—"}
          />
        </dl>

        <div className="mt-3 flex gap-2">
          <button
            type="button"
            onClick={() => void overlayToggle()}
            className="flex-1 rounded border border-papa-border px-2 py-1 hover:bg-white/5"
          >
            Alternar
          </button>
          <button
            type="button"
            onClick={() => void overlaySetMode(false)}
            className="flex-1 rounded border border-papa-border px-2 py-1 hover:bg-white/5"
          >
            Passivo
          </button>
        </div>

        <button
          type="button"
          onClick={() => void rodarBench()}
          disabled={benchRunning}
          className="mt-2 w-full rounded border border-papa-accent/50 px-2 py-1 text-papa-accent hover:bg-papa-accent/10 disabled:opacity-50"
        >
          {benchRunning
            ? "Medindo…"
            : `Benchmark: ${BENCH_ITERATIONS} alternâncias`}
        </button>

        {bench && (
          <dl className="mt-3 space-y-1 border-t border-papa-border pt-2">
            <Linha rotulo="Alternâncias" valor={`${bench.iterations}`} />
            <Linha rotulo="Falhas" valor={`${bench.failures}`} />
            <Linha rotulo="Média" valor={ms(bench.meanUs)} />
            <Linha rotulo="p50" valor={ms(bench.p50Us)} />
            <Linha rotulo="p95" valor={ms(bench.p95Us)} />
            <Linha
              rotulo="min / máx"
              valor={`${ms(bench.minUs)} / ${ms(bench.maxUs)}`}
            />
          </dl>
        )}

        {erro && <p className="mt-2 text-red-400">{erro}</p>}
      </div>
    </div>
  );
}

function Linha({ rotulo, valor }: { rotulo: string; valor: string }) {
  return (
    <div className="flex justify-between gap-3">
      <dt className="shrink-0 text-papa-muted">{rotulo}</dt>
      <dd className="truncate text-right" title={valor}>
        {valor}
      </dd>
    </div>
  );
}
