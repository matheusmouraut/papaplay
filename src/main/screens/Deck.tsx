import { save } from "@tauri-apps/plugin-dialog";
import { useEffect, useMemo, useState, type ReactNode } from "react";

import { deckExportCsv } from "../../shared/api/core";
import { Screenshot } from "../../shared/components/Screenshot";
import { Botao, CampoDeTexto, TituloDaTela } from "../../shared/components/ui";
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
    <section className="flex h-full flex-col gap-5">
      <TituloDaTela
        nota={
          cards.data
            ? `${cards.data.length} ${cards.data.length === 1 ? "palavra" : "palavras"}`
            : "carregando…"
        }
        acao={<ExportarCsv />}
      >
        Deck
      </TituloDaTela>

      <div className="flex flex-wrap items-center gap-2">
        <CampoDeTexto
          value={busca}
          onChange={(e) => setBusca(e.target.value)}
          placeholder="Buscar palavra ou frase…"
          className="min-w-56 flex-1"
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

        <label className="flex cursor-pointer items-center gap-2 text-sm text-papa-muted">
          <input
            type="checkbox"
            checked={incluirSuspensos}
            onChange={(e) => setIncluirSuspensos(e.target.checked)}
            className="accent-papa-accent"
          />
          Mostrar “já sei”
        </label>
      </div>

      <div className="flex min-h-0 flex-1 gap-5">
        <div className="min-h-0 flex-1 overflow-auto">
          {cards.isError && (
            <p className="rounded-lg border border-papa-erro/30 bg-papa-erro-soft px-4 py-3 text-sm text-papa-erro">
              {String(cards.error)}
            </p>
          )}

          {cards.data?.length === 0 && (
            <p className="max-w-md py-10 text-sm leading-relaxed text-papa-muted">
              {busca || jogo || estado
                ? "Nenhuma palavra com esses filtros."
                : "Nenhuma palavra ainda. Durante o jogo, segure Alt+X, aponte para uma palavra e clique nela para salvar."}
            </p>
          )}

          <ul className="divide-y divide-papa-border/60">
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
      className="cursor-pointer rounded-lg border border-papa-border bg-papa-surface px-3 py-2 text-sm text-papa-muted outline-none transition-colors duration-150 hover:border-papa-border-strong hover:text-papa-text focus:border-papa-accent/50"
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
        className={`group flex w-full flex-col items-start gap-1 rounded-lg px-3 py-3 text-left transition-colors duration-150 ${
          selecionado ? "bg-papa-accent-soft" : "hover:bg-papa-raised"
        }`}
      >
        <div className="flex w-full items-baseline gap-2">
          <span className="font-reading text-base text-papa-text">
            {card.lemma}
          </span>
          {/* Estado e contagem em texto simples, sem pílula: o que se procura
              aqui é a palavra, e cada caixinha rouba um pouco dela. */}
          <span className="text-xs text-papa-faint">
            {ESTADO_LABEL[card.fsrsState]}
            {card.contexts > 1 && ` · ${card.contexts} contextos`}
            {card.suspended && " · já sei"}
          </span>
          <span className="ml-auto shrink-0 text-xs text-papa-faint">
            {card.lastGame ?? data(card.createdAt)}
          </span>
        </div>
        {card.lastSentence && (
          <p className="line-clamp-1 font-reading text-[13px] text-papa-muted">
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
    <aside className="flex min-h-0 w-[26rem] shrink-0 flex-col overflow-auto rounded-xl border border-papa-border bg-papa-surface px-5 py-4">
      {!card ? (
        <p className="text-sm text-papa-muted">
          {detalhe.isPending ? "carregando…" : "Card não encontrado."}
        </p>
      ) : (
        <>
          <header className="flex items-baseline gap-2">
            <h3 className="font-reading text-2xl">{card.lemma}</h3>
            <button
              type="button"
              onClick={onFechar}
              className="ml-auto rounded px-1 text-sm text-papa-faint transition-colors duration-150 hover:text-papa-text"
              aria-label="Fechar detalhe"
            >
              ✕
            </button>
          </header>

          {/* Uma linha só: são três fatos pequenos, e uma tabela de rótulos
              para eles pesaria mais do que a informação. */}
          <p className="mt-1 text-xs leading-relaxed text-papa-faint">
            salvo em {data(card.createdAt)} · {ESTADO_LABEL[card.fsrsState]} ·{" "}
            {card.fsrsReps} {card.fsrsReps === 1 ? "revisão" : "revisões"}
            {card.fsrsLapses > 0 && ` · ${card.fsrsLapses} lapsos`}
            <br />
            próxima revisão em {data(card.fsrsDue)}
          </p>

          <div className="mt-4 flex gap-2">
            <Botao
              className="flex-1"
              disabled={suspender.isPending}
              onClick={() =>
                suspender.mutate({ id: card.id, suspended: !card.suspended })
              }
            >
              {card.suspended ? "Voltar para a fila" : "Já sei esta"}
            </Botao>
            <button
              type="button"
              disabled={excluir.isPending}
              onClick={() => {
                if (confirmando) excluir.mutate(card.id);
                else setConfirmando(true);
              }}
              // Fora do `Botao`: destruir é a única ação do app com cor
              // própria, e ela só existe no segundo clique.
              className={`flex-1 rounded-md border px-3 py-1.5 text-sm transition-colors duration-150 disabled:opacity-50 ${
                confirmando
                  ? "border-papa-erro/50 bg-papa-erro-soft text-papa-erro"
                  : "border-papa-border bg-papa-surface text-papa-text hover:border-papa-border-strong hover:bg-papa-raised"
              }`}
            >
              {confirmando ? "Confirmar exclusão" : "Excluir"}
            </button>
          </div>

          {confirmando && (
            <p className="mt-1.5 text-[11px] leading-relaxed text-papa-faint">
              Apaga a palavra, os contextos e os screenshots. Não dá para
              desfazer.
            </p>
          )}

          <h4 className="mt-7 text-[11px] font-medium tracking-wide text-papa-faint uppercase">
            Onde apareceu
          </h4>
          <ul className="mt-3 space-y-3">
            {detalhe.data?.contexts.map((contexto) => (
              <Contexto key={contexto.id} contexto={contexto} />
            ))}
          </ul>
        </>
      )}
    </aside>
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
    <li className="rounded-lg border border-papa-border px-3.5 py-3">
      <p className="font-reading text-[15px] leading-relaxed text-papa-text">
        {contexto.sentenceEn}
      </p>

      {editando ? (
        <div className="mt-2">
          <textarea
            value={texto}
            onChange={(e) => setTexto(e.target.value)}
            rows={2}
            autoFocus
            className="w-full rounded-lg border border-papa-border bg-papa-bg px-2.5 py-1.5 font-reading text-sm outline-none focus:border-papa-accent/50"
          />
          <div className="mt-1.5 flex gap-2">
            <Botao
              variante="primario"
              tamanho="sm"
              disabled={atualizar.isPending}
              onClick={() =>
                atualizar.mutate(
                  { id: contexto.id, sentencePt: texto.trim() || null },
                  { onSuccess: () => setEditando(false) },
                )
              }
            >
              Salvar
            </Botao>
            <Botao
              variante="sutil"
              tamanho="sm"
              onClick={() => {
                setTexto(contexto.sentencePt ?? "");
                setEditando(false);
              }}
            >
              Cancelar
            </Botao>
          </div>
        </div>
      ) : (
        <button
          type="button"
          onClick={() => setEditando(true)}
          title="Clique para corrigir a tradução"
          className="mt-1 w-full text-left font-reading text-sm leading-relaxed text-papa-muted transition-colors duration-150 hover:text-papa-text"
        >
          {contexto.sentencePt ?? (
            <span className="text-papa-faint">
              sem tradução — clique para escrever
            </span>
          )}
        </button>
      )}

      {contexto.screenshotPath && (
        <div className="mt-2.5">
          <Screenshot
            path={contexto.screenshotPath}
            alt={`Trecho da tela com “${contexto.form}”`}
          />
        </div>
      )}

      <p className="mt-2.5 text-[11px] text-papa-faint">
        {contexto.form} · {contexto.gameName ?? "jogo desconhecido"} ·{" "}
        {data(contexto.capturedAt)}
      </p>
    </li>
  );
}

/**
 * Export CSV do deck inteiro (F4).
 *
 * O deck é do usuário: ele tem que conseguir levá-lo para o Anki, para uma
 * planilha ou para outro app sem pedir licença. Exporta tudo, e não o filtro da
 * tela — quem exporta quer backup, e um backup parcial e silencioso é pior que
 * nenhum.
 */
function ExportarCsv() {
  const [estado, setEstado] = useState<
    | { tipo: "ocioso" }
    | { tipo: "gravando" }
    | { tipo: "pronto"; linhas: number }
    | { tipo: "erro"; msg: string }
  >({ tipo: "ocioso" });

  async function exportar() {
    const hoje = new Date().toISOString().slice(0, 10);
    const caminho = await save({
      defaultPath: `papaplay-deck-${hoje}.csv`,
      filters: [{ name: "CSV", extensions: ["csv"] }],
    });
    // `null` é o usuário fechando o diálogo — não é erro, e não merece aviso.
    if (!caminho) return;

    setEstado({ tipo: "gravando" });
    try {
      setEstado({ tipo: "pronto", linhas: await deckExportCsv(caminho) });
    } catch (erro) {
      setEstado({ tipo: "erro", msg: String(erro) });
    }
  }

  return (
    <div className="flex items-center gap-3">
      {estado.tipo === "pronto" && (
        <span className="text-xs text-papa-accent">
          {estado.linhas} {estado.linhas === 1 ? "linha" : "linhas"} exportadas
        </span>
      )}
      {estado.tipo === "erro" && (
        <span className="text-xs text-papa-erro">{estado.msg}</span>
      )}
      <Botao
        tamanho="sm"
        disabled={estado.tipo === "gravando"}
        onClick={exportar}
      >
        {estado.tipo === "gravando" ? "gravando…" : "Exportar CSV"}
      </Botao>
    </div>
  );
}
