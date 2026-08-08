import { useEffect, useState } from "react";

import { mediaScreenshot } from "../api/core";

/**
 * Screenshot de um contexto, lido do disco pelo core.
 *
 * A imagem vem como bytes pela IPC e vira uma URL de blob — não dá para apontar
 * um `<img src>` para o disco do usuário sem abrir o protocolo `asset://` para
 * uma pasta inteira, e o caminho do banco é configurável.
 */
export function Screenshot({ path, alt }: { path: string; alt: string }) {
  const [url, setUrl] = useState<string | null>(null);
  const [erro, setErro] = useState(false);

  useEffect(() => {
    let vivo = true;
    let criada: string | null = null;

    mediaScreenshot(path)
      .then((bytes) => {
        if (!vivo) return;
        criada = URL.createObjectURL(new Blob([bytes], { type: "image/webp" }));
        setUrl(criada);
      })
      .catch(() => {
        if (vivo) setErro(true);
      });

    return () => {
      vivo = false;
      // Sem o revoke, cada card aberto deixaria alguns megabytes presos na
      // memória da janela até ela ser fechada.
      if (criada) URL.revokeObjectURL(criada);
      setUrl(null);
      setErro(false);
    };
  }, [path]);

  // O arquivo pode ter sumido (backup restaurado sem a pasta media/, limpeza
  // manual). O card continua válido sem ele, então isto é um aviso, não um erro.
  if (erro) {
    return (
      <p className="rounded-md border border-dashed border-papa-border px-3 py-2 text-xs text-papa-muted">
        Screenshot não encontrado.
      </p>
    );
  }

  if (!url) {
    return <div className="h-16 animate-pulse rounded-md bg-papa-raised" />;
  }

  return (
    <img
      src={url}
      alt={alt}
      className="max-h-40 w-full rounded-md border border-papa-border object-contain"
    />
  );
}
