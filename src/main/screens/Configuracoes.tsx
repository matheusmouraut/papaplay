import { useEffect, useState } from "react";

import { Botao, TituloDaTela } from "../../shared/components/ui";
import { useNmtInstall, useNmtStatus } from "../../shared/hooks/useNmt";
import {
  usePreferences,
  useSetPreferences,
} from "../../shared/hooks/usePreferences";
import {
  useResetShortcuts,
  useSetShortcuts,
  useShortcuts,
} from "../../shared/hooks/useShortcuts";
import type { Shortcuts } from "../../shared/types";

/**
 * Tela Configurações (F6): atalhos globais e estudo.
 *
 * O que falta do F6 (aparência da overlay, notificação diária, pasta do banco,
 * backup) ainda não tem os comandos do core por trás; entra quando eles
 * existirem, em vez de UI sem função atrás.
 */

const ACOES: { chave: keyof Shortcuts; label: string; nota: string }[] = [
  {
    chave: "lookup",
    label: "Espiar",
    nota: "Segure para mostrar as palavras; solte para sumir.",
  },
  {
    chave: "card",
    label: "Abrir o card",
    nota: "Abre o card da palavra sob o cursor sem usar o mouse.",
  },
];

/** Teclas que so fazem sentido como modificador — nunca a tecla principal. */
const CODIGOS_DE_MODIFICADOR = new Set([
  "AltLeft",
  "AltRight",
  "ControlLeft",
  "ControlRight",
  "ShiftLeft",
  "ShiftRight",
  "MetaLeft",
  "MetaRight",
]);

/**
 * `event.code` para o token que o core espera ("KeyX" → "X", "Digit1" → "1").
 * F-keys, `Escape` e `Space` já batem com o parser do core sem conversão.
 */
function token(code: string): string {
  if (code.startsWith("Key")) return code.slice(3);
  if (code.startsWith("Digit")) return code.slice(5);
  return code;
}

/** Monta "Alt+X" a partir do evento — o mesmo formato que o core valida. */
function combinacaoDoEvento(e: KeyboardEvent): string | null {
  if (CODIGOS_DE_MODIFICADOR.has(e.code)) return null;
  const mods = [
    e.ctrlKey && "Ctrl",
    e.altKey && "Alt",
    e.shiftKey && "Shift",
    e.metaKey && "Super",
  ].filter(Boolean);
  return [...mods, token(e.code)].join("+");
}

/** `Esc` sempre cancela a captura em vez de virar a combinação — é o atalho
 * fixo que fecha o card em qualquer tela do app (ver `hotkeys::escape_shortcut`). */
function ehCancelamento(e: KeyboardEvent): boolean {
  return e.code === "Escape";
}

export function Configuracoes() {
  const shortcuts = useShortcuts();
  const definir = useSetShortcuts();
  const restaurar = useResetShortcuts();

  return (
    <section className="flex h-full flex-col gap-5 overflow-y-auto pr-1">
      <TituloDaTela>Configurações</TituloDaTela>

      {shortcuts.isPending && (
        <p className="text-sm text-papa-muted">carregando…</p>
      )}

      {shortcuts.data && (
        <div className="max-w-xl space-y-3">
          <h3 className="text-xs tracking-wide text-papa-faint uppercase">
            Atalhos globais
          </h3>
          {ACOES.map((acao) => (
            <LinhaDeAtalho
              key={acao.chave}
              label={acao.label}
              nota={acao.nota}
              valor={shortcuts.data[acao.chave]}
              pendente={definir.isPending}
              onCapturar={(combinacao) =>
                definir.mutate({ ...shortcuts.data, [acao.chave]: combinacao })
              }
            />
          ))}

          {definir.isError && (
            <p className="rounded-lg border border-papa-erro/30 bg-papa-erro-soft px-4 py-3 text-sm text-papa-erro">
              {String(definir.error)}
            </p>
          )}

          <Botao
            disabled={restaurar.isPending}
            onClick={() => restaurar.mutate()}
          >
            Restaurar padrões
          </Botao>

          <h3 className="pt-4 text-xs tracking-wide text-papa-faint uppercase">
            Estudo
          </h3>
          <NovasPorDia />

          <h3 className="pt-4 text-xs tracking-wide text-papa-faint uppercase">
            Tradutor de frases
          </h3>
          <Tradutor />
        </div>
      )}
    </section>
  );
}

/**
 * Instalação (ou reinstalação) do tradutor de frases.
 *
 * Fica em Configurações, e não só no wizard, porque pular o download é uma
 * escolha legítima: quem pulou precisa de um lugar óbvio para voltar atrás, e
 * quem trocou de máquina precisa saber onde os 332 MB foram parar.
 */
function Tradutor() {
  const status = useNmtStatus();
  const { instalar, progresso } = useNmtInstall();

  const porcentagem = progresso
    ? Math.round((progresso.baixado / progresso.total) * 100)
    : 0;

  return (
    <div className="rounded-lg border border-papa-border bg-papa-surface px-4 py-3">
      <div className="flex items-center gap-3">
        <div className="flex-1">
          <p className="text-sm text-papa-text">
            {status.data?.installed ? "Instalado" : "Não instalado"}
          </p>
          <p className="text-xs leading-relaxed text-papa-faint">
            {status.data?.installed
              ? "A tradução da frase de contexto está disponível, offline."
              : `Sem ele, o card mostra a frase em inglês sem tradução. São ${
                  status.data
                    ? Math.round(status.data.downloadBytes / 1_000_000)
                    : 330
                } MB, baixados uma vez só.`}
          </p>
        </div>
        {!status.data?.installed && !instalar.isPending && (
          <Botao variante="primario" onClick={() => instalar.mutate()}>
            Baixar
          </Botao>
        )}
      </div>

      {instalar.isPending && (
        <div className="mt-3">
          <div className="flex items-baseline justify-between text-xs text-papa-muted">
            <span>{progresso?.arquivo ?? "conectando…"}</span>
            <span className="tabular-nums">{porcentagem}%</span>
          </div>
          <div className="mt-1.5 h-0.5 w-full rounded-full bg-papa-border">
            <div
              className="h-full rounded-full bg-papa-accent transition-[width] duration-200"
              style={{ width: `${porcentagem}%` }}
            />
          </div>
        </div>
      )}

      {instalar.isError && (
        <p className="mt-3 text-xs leading-relaxed text-papa-erro">
          {String(instalar.error)}
        </p>
      )}

      {status.data && (
        <p className="mt-3 font-mono text-[11px] break-all text-papa-faint">
          {status.data.dir}
        </p>
      )}
    </div>
  );
}

/**
 * Cota diária de palavras novas.
 *
 * Só grava quando o campo perde o foco ou o usuário confirma: gravar a cada
 * tecla mandaria "1", "15", "150" ao core enquanto se digita "150", e cada um
 * desses valores intermediários volta limitado, brigando com o cursor.
 */
function NovasPorDia() {
  const preferencias = usePreferences();
  const definir = useSetPreferences();
  const salvo = preferencias.data?.newPerDay;
  const [rascunho, setRascunho] = useState<string | null>(null);

  const valor = rascunho ?? (salvo === undefined ? "" : String(salvo));

  function confirmar() {
    setRascunho(null);
    if (!preferencias.data) return;
    const numero = Number(valor);
    if (!Number.isFinite(numero) || numero === salvo) return;
    definir.mutate({ ...preferencias.data, newPerDay: numero });
  }

  return (
    <div className="flex items-center gap-3 rounded-lg border border-papa-border bg-papa-surface px-4 py-3">
      <div className="flex-1">
        <p className="text-sm text-papa-text">Palavras novas por dia</p>
        <p className="text-xs text-papa-faint">
          Quantas palavras inéditas a fila de revisão apresenta por dia.
        </p>
      </div>
      <input
        type="number"
        min={1}
        max={200}
        value={valor}
        disabled={preferencias.isPending || definir.isPending}
        onChange={(e) => setRascunho(e.target.value)}
        onBlur={confirmar}
        onKeyDown={(e) => {
          if (e.key === "Enter") e.currentTarget.blur();
        }}
        className="w-20 rounded-md border border-papa-border bg-transparent px-3 py-1.5 text-center font-mono text-sm text-papa-text transition-colors duration-150 outline-none focus:border-papa-accent/50 disabled:opacity-50"
      />
    </div>
  );
}

function LinhaDeAtalho({
  label,
  nota,
  valor,
  pendente,
  onCapturar,
}: {
  label: string;
  nota: string;
  valor: string;
  pendente: boolean;
  onCapturar: (combinacao: string) => void;
}) {
  const [capturando, setCapturando] = useState(false);

  useEffect(() => {
    if (!capturando) return;

    function aoTeclar(e: KeyboardEvent) {
      e.preventDefault();
      if (ehCancelamento(e)) {
        setCapturando(false);
        return;
      }
      const combinacao = combinacaoDoEvento(e);
      // Um modificador sozinho não fecha a captura — o usuário ainda está no
      // meio do gesto (ex.: segurou Alt, esperando pela tecla principal).
      if (!combinacao) return;
      setCapturando(false);
      onCapturar(combinacao);
    }

    window.addEventListener("keydown", aoTeclar);
    return () => window.removeEventListener("keydown", aoTeclar);
  }, [capturando, onCapturar]);

  return (
    <div className="flex items-center gap-3 rounded-lg border border-papa-border bg-papa-surface px-4 py-3">
      <div className="flex-1">
        <p className="text-sm text-papa-text">{label}</p>
        <p className="text-xs text-papa-faint">{nota}</p>
      </div>
      <button
        type="button"
        disabled={pendente}
        onClick={() => setCapturando(true)}
        className={`min-w-32 rounded-md border px-3 py-1.5 text-center font-mono text-sm transition-colors duration-150 disabled:opacity-50 ${
          capturando
            ? "border-papa-accent/50 text-papa-accent"
            : "border-papa-border text-papa-muted hover:border-papa-border-strong hover:text-papa-text"
        }`}
      >
        {capturando ? "pressione uma tecla…" : valor}
      </button>
    </div>
  );
}
