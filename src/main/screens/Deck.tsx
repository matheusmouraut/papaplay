import { useEffect, useMemo, useState, type ReactNode } from "react";

import { Screenshot } from "../../shared/components/Screenshot";
import {
  useCardDetail,
  useDeckCards,
  useDeckGames,
  useDeleteCard,
  useSuspendCard,
  useUpdateContext,
} from "../../shared/hooks/useDeck";
import type {
  CardContext,
  CardQuery,
  CardRow,
  DeckOrder,
  FsrsState,
} from "../../shared/types";

/**
 * Tela Deck (F4): a lista dos cards salvos e o detalhe de um deles.
 *
 * Filtro, busca e ordenação são todos do SQL — a tela manda o `CardQuery` e
 * desenha o que voltar. É o que mantém a lista correta quando o deck cresce
 * para milhares de cards, e o que evita duas regras de ordenação (uma no banco,
 * outra aqui) discordando entre si.
 */

const ESTADOS: { id: FsrsState; label: string }[] = [
  { id: "new", label: "Novos" },
  { id: "learning", label: "Aprendendo" },
  { id: "review", label: "Em revisão" },
  { id: "relearning", label: "Reaprendendo" },
];

const ESTADO_LABEL: Record<FsrsState, string> = {
  new: "novo",
  learning: "aprendendo",
  review: "revisão",
  relearning: "reaprendendo",
};

const ORDENS: { id: DeckOrder; label: string }[] = [
  { id: "recentes", label: "Mais recentes" },
  { id: "alfabetica", label: "A → Z" },
  { id: "vencimento", label: "Vencimento" },
  { id: "maisDificeis", label: "Mais difíceis" },
];

/** Espera antes de mandar a busca para o core, em ms. */
const ATRASO_DA_BUSCA = 200;

function data(iso: string): string {
  return new Date(iso).toLocaleDateString("pt-BR");
}

export function Deck() {
  const [busca, setBusca] = useState("");
  const [jogo, setJogo] = useState("");
  const [estado, setEstado] = useState("");
  const [ordem, setOrdem] = useState<DeckOrder>("recentes");
  const [incluirSuspensos, setIncluirSuspensos] = useState(false);
  const [selecionado, setSelecionado] = useState<number | null>(null);

  // O campo de busca dispara uma consulta por tecla; o atraso junta a digitação
  // numa consulta só.
  const [buscaAtrasada, setBuscaAtrasada] = useState("");
  useEffect(() => {
    const id = setTimeout(() => setBuscaAtrasada(busca), ATRASO_DA_BUSCA);
    return () => clearTimeout(id);
  }, [busca]);

  const query: CardQuery = useMemo(
    () => ({
      search: buscaAtrasada || null,
      game: jogo || null,
      state: (estado || null) as FsrsState | null,
      includeSuspended: incluirSuspensos,
      order: ordem,
    }),
    [buscaAtrasada, jogo, estado, incluirSuspensos, ordem],
  );

  const cards = useDeckCards(query);
  const jogos = useDeckGames();

  // Um card excluído (ou filtrado para fora) não pode continuar aberto no
  // painel da direita. Derivado da lista em vez de apagado num efeito: assim o
  // card volta a aparecer sozinho quando o filtro que o escondeu sai.
  const aberto =
    selecionado !== null &&
    (cards.data?.some((card) => card.id === selecionado) ?? false)
      ? selecionado
      : null;

  return (
    <section className="flex h-full flex-col gap-4">
      <header>
        <h2 className="text-2xl font-semibold tracking-tight">Deck</h2>
        <p className="mt-1 text-sm text-papa-muted">
          {cards.data
            ? `${cards.data.length} ${cards.data.length === 1 ? "card" : "cards"}`
            : "Carregando…"}
        </p>
      </header>

      <div className="flex flex-wrap items-center gap-2">
        <input
          value={busca}
          onChange={(e) => setBusca(e.target.value)}
          placeholder="Buscar palavra ou frase…"
          className="min-w-56 flex-1 rounded-md border border-papa-border bg-papa-surface px-3 py-2 text-sm outline-none placeholder:text-papa-muted/70 focus:border-papa-accent/60"
        />

        <Select value={jogo} onChange={setJogo} label="Todos os jogos">
          {(jogos.data ?? []).map((nome) => (
            <option key={nome} value={nome}>
              {nome}
            </option>
          ))}
        </Select>

        <Select value={estado} onChange={setEstado} label="Todos os estados">
          {ESTADOS.map((item) => (
            <option key={item.id} value={item.id}>
              {item.label}
            </option>
          ))}
        </Select>

        <Select
          value={ordem}
          onChange={(valor) => setOrdem(valor as DeckOrder)}
          label={null}
        >
          {ORDENS.map((item) => (
            <option key={item.id} value={item.id}>
              {item.label}
            </option>
          ))}
        </Select>

        <label className="flex items-center gap-2 text-sm text-papa-muted">
          <input
            type="checkbox"
            checked={incluirSuspensos}
            onChange={(e) => setIncluirSuspensos(e.target.checked)}
            className="accent-papa-accent"
          />
          Mostrar “já sei”
        </label>
      </div>

      <div className="flex min-h-0 flex-1 gap-4">
        <div className="min-h-0 flex-1 overflow-auto rounded-lg border border-papa-border">
          {cards.isError && (
            <p className="p-4 text-sm text-red-400">{String(cards.error)}</p>
          )}

          {cards.data?.length === 0 && (
            <p className="p-6 text-sm text-papa-muted">
              {busca || jogo || estado
                ? "Nenhum card com esses filtros."
                : "Nenhum card ainda. Salve uma palavra pelo overlay (Alt+X) durante o jogo."}
            </p>
          )}

          <ul className="divide-y divide-papa-border">
            {(cards.data ?? []).map((card) => (
              <LinhaDoCard
                key={card.id}
                card={card}
                selecionado={card.id === selecionado}
                onSelect={() => setSelecionado(card.id)}
              />
            ))}
          </ul>
        </div>

        {aberto !== null && (
          // `key` por card: trocar de card monta um painel novo, o que zera a
          // confirmação de exclusão sem nenhum efeito para isso.
          <Detalhe
            key={aberto}
            cardId={aberto}
            onFechar={() => setSelecionado(null)}
          />
        )}
      </div>
    </section>
  );
}

function Select({
  value,
  onChange,
  label,
  children,
}: {
  value: string;
  onChange: (valor: string) => void;
  /** Texto da opção vazia; `null` quando o filtro não tem "todos". */
  label: string | null;
  children: ReactNode;
}) {
  return (
    <select
      value={value}
      onChange={(e) => onChange(e.target.value)}
      className="rounded-md border border-papa-border bg-papa-surface px-3 py-2 text-sm outline-none focus:border-papa-accent/60"
    >
      {label !== null && <option value="">{label}</option>}
      {children}
    </select>
  );
}

function LinhaDoCard({
  card,
  selecionado,
  onSelect,
}: {
  card: CardRow;
  selecionado: boolean;
  onSelect: () => void;
}) {
  return (
    <li>
      <button
        type="button"
        onClick={onSelect}
        className={`flex w-full flex-col items-start gap-1 px-4 py-3 text-left transition-colors ${
          selecionado ? "bg-papa-accent/10" : "hover:bg-white/5"
        }`}
      >
        <div className="flex w-full items-center gap-2">
          <span className="font-medium">{card.lemma}</span>
          <span className="rounded bg-white/10 px-1.5 py-0.5 text-[10px] text-papa-muted">
            {ESTADO_LABEL[card.fsrsState]}
          </span>
          {card.suspended && (
            <span className="rounded bg-white/10 px-1.5 py-0.5 text-[10px] text-papa-muted">
              já sei
            </span>
          )}
          {card.contexts > 1 && (
            <span className="text-xs text-papa-muted">
              {card.contexts} contextos
            </span>
          )}
          <span className="ml-auto shrink-0 text-xs text-papa-muted">
            {card.lastGame ?? data(card.createdAt)}
          </span>
        </div>
        {card.lastSentence && (
          <p className="line-clamp-1 text-xs text-papa-muted">
            {card.lastSentence}
          </p>
        )}
      </button>
    </li>
  );
}

function Detalhe({
  cardId,
  onFechar,
}: {
  cardId: number;
  onFechar: () => void;
}) {
  const detalhe = useCardDetail(cardId);
  const suspender = useSuspendCard();
  const excluir = useDeleteCard();
  const [confirmando, setConfirmando] = useState(false);

  const card = detalhe.data?.card;

  return (
    <aside className="flex min-h-0 w-96 shrink-0 flex-col overflow-auto rounded-lg border border-papa-border bg-papa-surface p-4">
      {!card ? (
        <p className="text-sm text-papa-muted">
          {detalhe.isPending ? "Carregando…" : "Card não encontrado."}
        </p>
      ) : (
        <>
          <div className="flex items-baseline gap-2">
            <h3 className="text-xl font-semibold">{card.lemma}</h3>
            <button
              type="button"
              onClick={onFechar}
              className="ml-auto text-sm text-papa-muted hover:text-papa-text"
              aria-label="Fechar detalhe"
            >
              ✕
            </button>
          </div>

          <dl className="mt-3 space-y-1 text-xs text-papa-muted">
            <Info rotulo="Salvo em" valor={data(card.createdAt)} />
            <Info
              rotulo="Estado"
              valor={`${ESTADO_LABEL[card.fsrsState]} · ${card.fsrsReps} revisões · ${card.fsrsLapses} lapsos`}
            />
            <Info rotulo="Próxima revisão" valor={data(card.fsrsDue)} />
          </dl>

          <div className="mt-4 flex gap-2">
            <button
              type="button"
              disabled={suspender.isPending}
              onClick={() =>
                suspender.mutate({ id: card.id, suspended: !card.suspended })
              }
              className="flex-1 rounded-md border border-papa-border px-3 py-1.5 text-sm hover:bg-white/5 disabled:opacity-50"
            >
              {card.suspended ? "Voltar para a fila" : "Já sei esta"}
            </button>
            <button
              type="button"
              disabled={excluir.isPending}
              onClick={() => {
                if (confirmando) excluir.mutate(card.id);
                else setConfirmando(true);
              }}
              className={`flex-1 rounded-md px-3 py-1.5 text-sm disabled:opacity-50 ${
                confirmando
                  ? "border border-red-500/60 bg-red-500/10 text-red-300"
                  : "border border-papa-border hover:bg-white/5"
              }`}
            >
              {confirmando ? "Confirmar exclusão" : "Excluir"}
            </button>
          </div>

          {confirmando && (
            <p className="mt-1 text-[11px] text-papa-muted">
              Apaga o card, os contextos e os screenshots. Não dá para desfazer.
            </p>
          )}

          <h4 className="mt-6 text-xs font-medium uppercase tracking-wide text-papa-muted">
            Contextos
          </h4>
          <ul className="mt-2 space-y-4">
            {detalhe.data?.contexts.map((contexto) => (
              <Contexto key={contexto.id} contexto={contexto} />
            ))}
          </ul>
        </>
      )}
    </aside>
  );
}

function Info({ rotulo, valor }: { rotulo: string; valor: string }) {
  return (
    <div className="flex justify-between gap-3">
      <dt>{rotulo}</dt>
      <dd className="text-right text-papa-text">{valor}</dd>
    </div>
  );
}

/**
 * Uma ocorrência da palavra: a frase, a tradução (editável) e o recorte da tela
 * onde ela apareceu.
 */
function Contexto({ contexto }: { contexto: CardContext }) {
  const atualizar = useUpdateContext();
  const [editando, setEditando] = useState(false);
  const [texto, setTexto] = useState(contexto.sentencePt ?? "");

  return (
    <li className="rounded-md border border-papa-border p-3">
      <p className="text-sm text-papa-text">{contexto.sentenceEn}</p>

      {editando ? (
        <div className="mt-2">
          <textarea
            value={texto}
            onChange={(e) => setTexto(e.target.value)}
            rows={2}
            className="w-full rounded-md border border-papa-border bg-papa-bg px-2 py-1 text-sm outline-none focus:border-papa-accent/60"
          />
          <div className="mt-1 flex gap-2">
            <button
              type="button"
              disabled={atualizar.isPending}
              onClick={() =>
                atualizar.mutate(
                  { id: contexto.id, sentencePt: texto.trim() || null },
                  { onSuccess: () => setEditando(false) },
                )
              }
              className="rounded border border-papa-accent/50 px-2 py-0.5 text-xs text-papa-accent hover:bg-papa-accent/10 disabled:opacity-50"
            >
              Salvar
            </button>
            <button
              type="button"
              onClick={() => {
                setTexto(contexto.sentencePt ?? "");
                setEditando(false);
              }}
              className="rounded border border-papa-border px-2 py-0.5 text-xs text-papa-muted hover:bg-white/5"
            >
              Cancelar
            </button>
          </div>
        </div>
      ) : (
        <button
          type="button"
          onClick={() => setEditando(true)}
          title="Clique para corrigir a tradução"
          className="mt-1 w-full text-left text-sm text-papa-muted hover:text-papa-text"
        >
          {contexto.sentencePt ?? "Sem tradução — clique para escrever uma."}
        </button>
      )}

      {contexto.screenshotPath && (
        <div className="mt-2">
          <Screenshot
            path={contexto.screenshotPath}
            alt={`Trecho da tela com “${contexto.form}”`}
          />
        </div>
      )}

      <p className="mt-2 text-[11px] text-papa-muted/80">
        {contexto.form} · {contexto.gameName ?? "jogo desconhecido"} ·{" "}
        {data(contexto.capturedAt)}
      </p>
    </li>
  );
}
