import { useCallback, useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";

import {
  lookupRun,
  overlayBench,
  overlaySetMode,
  overlayStatus,
  overlayToggle,
} from "../shared/api/core";
import type {
  LookupResult,
  OverlayBenchReport,
  OverlayModeChange,
} from "../shared/types";
import { WordHighlights } from "./components/WordHighlights";

/**
 * Janela overlay.
 *
 * Em modo passivo desenha só as marcas de canto. Ao entrar em modo lookup
 * (Alt+X) dispara uma consulta — captura do entorno do cursor + OCR — e
 * destaca as palavras encontradas.
 *
 * O painel de diagnóstico (F9) sobrou da spike 01 e sai quando o app tiver
 * uma tela de configurações de verdade.
 */

const BENCH_ITERATIONS = 50;

/** Tecla que mostra/esconde o painel de diagnóstico. */
const HUD_KEY = "F9";

function ms(microseconds: number): string {
  return `${(microseconds / 1000).toFixed(1)} ms`;
}

/**
 * Marcas nos 4 cantos: se as 4 aparecerem coladas nas bordas do monitor, o
 * posicionamento acertou o monitor inteiro (checagem de DPI/multi-monitor).
 * Ficam até sairmos da fase de testes.
 *
 * Com o painel escondido elas são o **único** retorno visual do modo atual —
 * daí a cor mudar entre passivo (discreto) e lookup (aceso).
 */
function CornerMarks({ interactive }: { interactive: boolean }) {
  const corners = [
    "left-0 top-0 border-l-4 border-t-4",
    "right-0 top-0 border-r-4 border-t-4",
    "left-0 bottom-0 border-l-4 border-b-4",
    "right-0 bottom-0 border-r-4 border-b-4",
  ];
  const tint = interactive ? "border-papa-accent/80" : "border-papa-muted/30";
  return (
    <>
      {corners.map((position) => (
        <div
          key={position}
          className={`absolute h-10 w-10 transition-colors ${tint} ${position}`}
        />
      ))}
    </>
  );
}

export function App() {
  const [mode, setMode] = useState<OverlayModeChange | null>(null);
  const [interactive, setInteractive] = useState(false);
  const [hudVisible, setHudVisible] = useState(false);
  const [bench, setBench] = useState<OverlayBenchReport | null>(null);
  const [benchRunning, setBenchRunning] = useState(false);
  const [erro, setErro] = useState<string | null>(null);
  const [lookup, setLookup] = useState<LookupResult | null>(null);
  const [cursor, setCursor] = useState<{ x: number; y: number } | null>(null);

  // Derivado, não estado: entre entrar em lookup e o resultado chegar não há
  // nem resultado nem erro. Um booleano à parte só criaria uma terceira fonte
  // de verdade para sair de sincronia.
  const lendo = interactive && lookup === null && erro === null;

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

  // Entrar em lookup dispara a consulta; sair joga fora o resultado, porque a
  // tela do jogo já mudou e destaque velho aponta para a palavra errada.
  useEffect(() => {
    if (!interactive) return;
    let cancelado = false;
    lookupRun()
      .then((resultado) => {
        if (!cancelado) setLookup(resultado);
      })
      .catch((e: unknown) => {
        if (!cancelado) setErro(String(e));
      });
    return () => {
      cancelado = true;
      setLookup(null);
      setCursor(null);
      setErro(null);
    };
  }, [interactive]);

  // Só em modo lookup: em passivo a janela é click-through e não recebe mouse.
  useEffect(() => {
    if (!interactive) return;
    function onMouseMove(event: MouseEvent) {
      setCursor({ x: event.clientX, y: event.clientY });
    }
    window.addEventListener("mousemove", onMouseMove);
    return () => window.removeEventListener("mousemove", onMouseMove);
  }, [interactive]);

  // Listener da janela, **não** atalho global: só dispara quando a overlay tem
  // o foco, ou seja, em modo lookup. Em modo passivo o F9 vai para o jogo.
  useEffect(() => {
    function onKeyDown(event: KeyboardEvent) {
      if (event.key === HUD_KEY) {
        event.preventDefault();
        setHudVisible((visible) => !visible);
      }
    }
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
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
      <CornerMarks interactive={interactive} />

      {lookup && <WordHighlights resultado={lookup} cursor={cursor} />}

      {lendo && (
        <div className="absolute left-1/2 top-6 -translate-x-1/2 rounded-full border border-papa-border bg-black/80 px-4 py-1.5 text-xs text-papa-muted">
          Lendo a tela…
        </div>
      )}

      {/* Erro fica visível sem o F9: a falha mais provável aqui é "modelos de
          OCR não baixados", e sem aviso a overlay só pareceria não funcionar. */}
      {interactive && erro && (
        <div className="absolute left-1/2 top-6 max-w-xl -translate-x-1/2 rounded-lg border border-red-500/50 bg-black/90 px-4 py-2 text-xs text-red-300">
          {erro}
        </div>
      )}

      {hudVisible && (
        <div className="absolute left-6 top-6 w-80 rounded-lg border border-papa-border bg-black/80 p-4 text-xs text-papa-text shadow-2xl">
          <div className="mb-3 flex items-center justify-between">
            <span className="text-sm font-semibold">
              PapaPlay — diagnóstico
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
            <Linha rotulo="Este painel" valor={`${HUD_KEY} esconde`} />
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

          {lookup && (
            <dl className="mt-3 space-y-1 border-t border-papa-border pt-2">
              <Linha
                rotulo="Palavras"
                valor={`${lookup.words.length} em ${lookup.lines.length} linhas`}
              />
              <Linha
                rotulo="Recorte"
                valor={`${lookup.region.width}×${lookup.region.height} @ ${lookup.region.x},${lookup.region.y}`}
              />
              <Linha
                rotulo="Captura"
                valor={`${lookup.captureMs.toFixed(0)} ms`}
              />
              <Linha rotulo="OCR" valor={`${lookup.ocrMs.toFixed(0)} ms`} />
              {/* Orçamento do doc 03: consulta completa < 1 s. */}
              <Linha
                rotulo="Consulta"
                valor={`${lookup.totalMs.toFixed(0)} ms de 1000`}
              />
            </dl>
          )}

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
      )}
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
