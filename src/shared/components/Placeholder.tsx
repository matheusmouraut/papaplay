import type { ReactNode } from "react";

interface PlaceholderProps {
  title: string;
  description: string;
  /** Documento em `docs/` que especifica esta tela. */
  spec?: string;
  children?: ReactNode;
}

/** Tela ainda não implementada — some conforme as features chegam. */
export function Placeholder({
  title,
  description,
  spec,
  children,
}: PlaceholderProps) {
  return (
    <section className="max-w-2xl">
      <h2 className="text-2xl font-semibold tracking-tight">{title}</h2>
      <p className="mt-2 text-sm leading-relaxed text-papa-muted">
        {description}
      </p>
      {spec && (
        <p className="mt-1 text-xs text-papa-muted/70">Especificação: {spec}</p>
      )}
      {children && <div className="mt-6">{children}</div>}
    </section>
  );
}
