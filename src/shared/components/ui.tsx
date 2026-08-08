import type {
  ButtonHTMLAttributes,
  InputHTMLAttributes,
  ReactNode,
} from "react";

/**
 * Os primitivos da linguagem visual (F7).
 *
 * Existem porque o critério de aceite da F7 é comparativo: um print do Deck e um
 * do card da overlay têm que parecer o mesmo produto. Com as classes repetidas à
 * mão em cada tela, "o mesmo produto" dura até a próxima tela — a terceira vez
 * que alguém escreve `rounded-lg border border-papa-border` é onde a diferença
 * começa.
 *
 * O que **não** está aqui: a overlay tem superfície própria (`papa-vidro`), que
 * é vidro escuro sobre a cena do jogo em vez de cartão sobre fundo. A diferença
 * é intencional — o que precisa ser igual nos dois lados é a tipografia, o raio,
 * o espaçamento e o acento, e esses vêm do `theme.css` que as duas janelas
 * importam.
 */

type Variante = "primario" | "secundario" | "sutil";

/** Base comum: raio de 6–8px, transição de 150ms, foco pelo `index.css`. */
const BASE =
  "rounded-md transition-colors duration-150 disabled:opacity-50 disabled:cursor-default";

const VARIANTES: Record<Variante, string> = {
  // Verde cheio com texto branco — o mesmo botão da landing (`site/estilo.css`,
  // `.botao-principal`), para quem baixou o app reconhecer o botão que clicou
  // no site. O acento significa "a ação desta tela", e por isso ele nunca
  // aparece em dois botões ao mesmo tempo.
  primario: "bg-papa-accent text-white hover:bg-[#1f6d52]",
  secundario:
    "border border-papa-border bg-papa-surface text-papa-text hover:border-papa-border-strong hover:bg-papa-raised",
  // Sem borda: para ações que não competem com o conteúdo (voltar, cancelar).
  sutil: "text-papa-muted hover:bg-papa-raised hover:text-papa-text",
};

const TAMANHOS = {
  sm: "px-2.5 py-1 text-xs",
  md: "px-3 py-1.5 text-sm",
  lg: "px-4 py-2.5 text-sm",
} as const;

export function Botao({
  variante = "secundario",
  tamanho = "md",
  className = "",
  ...props
}: ButtonHTMLAttributes<HTMLButtonElement> & {
  variante?: Variante;
  tamanho?: keyof typeof TAMANHOS;
}) {
  return (
    <button
      type="button"
      {...props}
      className={`${BASE} ${VARIANTES[variante]} ${TAMANHOS[tamanho]} ${className}`}
    />
  );
}

/**
 * Bloco de conteúdo sobre o fundo da janela.
 *
 * Um degrau de superfície, uma borda de 1px, nunca sombra: profundidade por
 * empilhamento de sombras é o vocabulário de "painel de ferramenta", que é
 * exatamente o que a F7 diz para não parecer.
 */
export function Cartao({
  children,
  className = "",
  padding = "md",
}: {
  children: ReactNode;
  className?: string;
  padding?: "sm" | "md" | "lg";
}) {
  const espaco = { sm: "px-4 py-3", md: "px-5 py-4", lg: "px-8 py-7" }[padding];
  return (
    <div
      className={`rounded-xl border border-papa-border bg-papa-surface ${espaco} ${className}`}
    >
      {children}
    </div>
  );
}

export function CampoDeTexto({
  className = "",
  ...props
}: InputHTMLAttributes<HTMLInputElement>) {
  return (
    <input
      {...props}
      className={`rounded-lg border border-papa-border bg-papa-surface px-3 py-2 text-sm outline-none transition-colors duration-150 placeholder:text-papa-faint hover:border-papa-border-strong focus:border-papa-accent/50 disabled:opacity-50 ${className}`}
    />
  );
}

/**
 * Cabeçalho de tela.
 *
 * A serifada no título é o que dá à janela principal o ar de editor de texto em
 * vez de painel: o produto é leitura, e o título é a primeira coisa que diz
 * isso.
 */
export function TituloDaTela({
  children,
  nota,
  acao,
}: {
  children: ReactNode;
  /** Uma linha de contexto ao lado do título (contagens, estado). */
  nota?: ReactNode;
  /** Ação da tela, alinhada à direita. */
  acao?: ReactNode;
}) {
  return (
    <header className="flex items-baseline gap-3">
      <h2 className="font-reading text-3xl tracking-tight">{children}</h2>
      {nota && <p className="text-sm text-papa-faint">{nota}</p>}
      {acao && <div className="ml-auto">{acao}</div>}
    </header>
  );
}

/**
 * Estado vazio: um título, uma frase e, no máximo, uma saída.
 *
 * Toda tela do app pode estar vazia no primeiro dia de uso, e um app que
 * responde ao vazio com uma área em branco parece quebrado.
 */
export function Vazio({
  titulo,
  children,
  acao,
}: {
  titulo: string;
  children: ReactNode;
  acao?: ReactNode;
}) {
  return (
    <section className="max-w-md">
      <h3 className="font-reading text-2xl tracking-tight">{titulo}</h3>
      <p className="mt-2 text-sm leading-relaxed text-papa-muted">{children}</p>
      {acao && <div className="mt-6">{acao}</div>}
    </section>
  );
}
