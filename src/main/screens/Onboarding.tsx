import { useState } from "react";

import logo from "../../assets/logo.svg";
import playground from "../../assets/playground.png";
import { Botao, Cartao } from "../../shared/components/ui";
import { useNmtInstall, useNmtStatus } from "../../shared/hooks/useNmt";
import {
  usePreferences,
  useSetPreferences,
} from "../../shared/hooks/usePreferences";
import { useShortcuts } from "../../shared/hooks/useShortcuts";

/**
 * Primeira execução (F8).
 *
 * Meta declarada no doc 03: primeiro lookup bem-sucedido em menos de 2 minutos.
 * Por isso o passo do playground vem antes de qualquer explicação sobre revisão
 * espaçada — o que convence é a palavra traduzida aparecendo sobre a tela, não
 * um texto dizendo que ela vai aparecer.
 *
 * São quatro passos, e não os três do doc 03: o download do tradutor entrou
 * quando os 332 MB dele saíram do instalador (ver `src-tauri/src/setup.rs`).
 * Ele é o único passo pulável, porque é o único que não impede o app de
 * funcionar.
 */

const PASSOS = ["Atalho", "Tradutor", "Experimente", "Revisão"] as const;

export function Onboarding() {
  const [passo, setPasso] = useState(0);
  const preferencias = usePreferences();
  const definir = useSetPreferences();

  function concluir() {
    if (!preferencias.data) return;
    definir.mutate({ ...preferencias.data, onboardingDone: true });
  }

  return (
    <div className="flex h-full items-center justify-center bg-papa-bg px-10">
      <div className="w-full max-w-2xl">
        <div className="mb-8 flex items-center gap-2.5">
          <img
            src={logo}
            alt=""
            width={32}
            height={32}
            className="rounded-lg"
          />
          <span className="font-reading text-xl tracking-tight text-papa-text">
            PapaPlay
          </span>
        </div>

        <Trilha atual={passo} />

        <div className="mt-8 min-h-[22rem]">
          {passo === 0 && <PassoAtalho />}
          {passo === 1 && <PassoTradutor />}
          {passo === 2 && <PassoPlayground />}
          {passo === 3 && <PassoRevisao />}
        </div>

        <div className="mt-8 flex items-center gap-3">
          {passo > 0 && (
            <Botao variante="sutil" onClick={() => setPasso(passo - 1)}>
              Voltar
            </Botao>
          )}
          <div className="ml-auto flex gap-3">
            {/* Pular fecha o wizard inteiro, e não só o passo: quem já sabe o
                que o app faz não deve precisar de quatro cliques para sair. */}
            {passo < PASSOS.length - 1 && (
              <Botao variante="sutil" onClick={concluir}>
                Pular
              </Botao>
            )}
            <Botao
              variante="primario"
              tamanho="lg"
              onClick={() =>
                passo === PASSOS.length - 1 ? concluir() : setPasso(passo + 1)
              }
              disabled={definir.isPending}
            >
              {passo === PASSOS.length - 1 ? "Começar" : "Continuar"}
            </Botao>
          </div>
        </div>
      </div>
    </div>
  );
}

function Trilha({ atual }: { atual: number }) {
  return (
    <div className="flex items-center gap-2">
      {PASSOS.map((nome, i) => (
        <div key={nome} className="flex flex-1 flex-col gap-2">
          <div
            className={`h-0.5 rounded-full transition-colors duration-150 ${
              i <= atual ? "bg-papa-accent" : "bg-papa-raised"
            }`}
          />
          <span
            className={`text-[11px] ${i <= atual ? "text-papa-muted" : "text-papa-faint"}`}
          >
            {nome}
          </span>
        </div>
      ))}
    </div>
  );
}

function Titulo({ children }: { children: React.ReactNode }) {
  return <h2 className="font-reading text-3xl tracking-tight">{children}</h2>;
}

function PassoAtalho() {
  const atalhos = useShortcuts();
  return (
    <section>
      <Titulo>Segurar, olhar, soltar</Titulo>
      <p className="mt-3 max-w-lg text-sm leading-relaxed text-papa-muted">
        Durante o jogo, segure{" "}
        <kbd className="rounded border border-papa-border px-1.5 py-0.5 font-mono text-papa-text">
          {atalhos.data?.lookup ?? "Alt+X"}
        </kbd>{" "}
        e aponte o mouse para uma palavra em inglês. A tradução aparece sobre a
        tela. Ao soltar, some.
      </p>
      <p className="mt-3 max-w-lg text-sm leading-relaxed text-papa-muted">
        O jogo nunca perde o foco e nada é injetado nele — o PapaPlay só lê a
        imagem da tela. Para abrir o card completo sem o mouse, use{" "}
        <kbd className="rounded border border-papa-border px-1.5 py-0.5 font-mono text-papa-text">
          {atalhos.data?.card ?? "Alt+C"}
        </kbd>
        .
      </p>
      <p className="mt-6 text-xs text-papa-faint">
        Dá para trocar as duas combinações depois, em Configurações.
      </p>
    </section>
  );
}

/** Formata bytes em MB inteiros — a única grandeza que interessa aqui. */
function mb(bytes: number): string {
  return `${Math.round(bytes / 1_000_000)} MB`;
}

function PassoTradutor() {
  const status = useNmtStatus();
  const { instalar, progresso } = useNmtInstall();

  if (status.data?.installed) {
    return (
      <section>
        <Titulo>Tradutor pronto</Titulo>
        <p className="mt-3 max-w-lg text-sm leading-relaxed text-papa-muted">
          O tradutor de frases já está instalado nesta máquina. Nada a fazer
          aqui.
        </p>
      </section>
    );
  }

  const porcentagem = progresso
    ? Math.round((progresso.baixado / progresso.total) * 100)
    : 0;

  return (
    <section>
      <Titulo>Tradutor de frases</Titulo>
      <p className="mt-3 max-w-lg text-sm leading-relaxed text-papa-muted">
        O dicionário de palavras já veio instalado. A tradução da{" "}
        <em>frase inteira</em> usa um modelo de{" "}
        {status.data ? mb(status.data.downloadBytes) : "~330 MB"} que fica de
        fora do instalador para ele não pesar isso tudo.
      </p>
      <p className="mt-3 max-w-lg text-sm leading-relaxed text-papa-muted">
        É o único momento em que o PapaPlay usa a internet. Depois de baixado,
        tudo funciona offline — e você pode pular agora e instalar depois em
        Configurações.
      </p>

      <div className="mt-6">
        {instalar.isPending ? (
          <Cartao>
            <div className="flex items-baseline justify-between text-xs text-papa-muted">
              <span>{progresso?.arquivo ?? "conectando…"}</span>
              <span className="tabular-nums">{porcentagem}%</span>
            </div>
            <div className="mt-2 h-0.5 w-full rounded-full bg-papa-border">
              <div
                className="h-full rounded-full bg-papa-accent transition-[width] duration-200"
                style={{ width: `${porcentagem}%` }}
              />
            </div>
          </Cartao>
        ) : (
          <Botao variante="primario" onClick={() => instalar.mutate()}>
            Baixar agora
          </Botao>
        )}
        {instalar.isError && (
          <p className="mt-3 max-w-lg text-xs leading-relaxed text-papa-erro">
            {String(instalar.error)}
          </p>
        )}
      </div>
    </section>
  );
}

function PassoPlayground() {
  const atalhos = useShortcuts();
  return (
    <section>
      <Titulo>Experimente agora</Titulo>
      <p className="mt-3 max-w-lg text-sm leading-relaxed text-papa-muted">
        Esta é uma tela de jogo de verdade. Segure{" "}
        <kbd className="rounded border border-papa-border px-1.5 py-0.5 font-mono text-papa-text">
          {atalhos.data?.lookup ?? "Alt+X"}
        </kbd>{" "}
        e aponte para <strong className="text-papa-text">constellation</strong>{" "}
        ou <strong className="text-papa-text">carved</strong>. Clique na palavra
        para abrir o card e salvá-la.
      </p>
      {/* Uma imagem, e não um texto renderizado pelo app: o pipeline é OCR, e
          treinar em texto do DOM ensinaria o gesto num caso que não existe. */}
      <img
        src={playground}
        alt="Trecho de uma tela de jogo em inglês, para treinar o gesto de espiar"
        className="mt-6 w-full rounded-lg border border-papa-border"
      />
    </section>
  );
}

function PassoRevisao() {
  return (
    <section>
      <Titulo>O que acontece depois</Titulo>
      <p className="mt-3 max-w-lg text-sm leading-relaxed text-papa-muted">
        Cada palavra salva vira um card com a frase e o recorte da tela onde ela
        apareceu. Na aba <strong className="text-papa-text">Revisar</strong>,
        eles voltam espaçados no tempo — pouco antes de você esquecer.
      </p>
      <p className="mt-3 max-w-lg text-sm leading-relaxed text-papa-muted">
        Você vê a frase, tenta lembrar o significado e responde com uma das
        quatro notas. O agendamento se ajusta ao que você acerta e ao que erra.
      </p>
      <p className="mt-3 max-w-lg text-sm leading-relaxed text-papa-muted">
        Quinze palavras novas por dia, por padrão. Dez minutos de revisão dão
        conta.
      </p>
    </section>
  );
}
