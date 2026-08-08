# PapaPlay

Aprenda inglês jogando. Overlay para Windows: passe o mouse numa palavra no jogo, veja a tradução PT-BR com contexto, salve num deck e revise com repetição espaçada (FSRS). Offline.

- **Documentação completa:** [`docs/`](docs/README.md)
- **Instruções para o Claude Code:** [`CLAUDE.md`](CLAUDE.md)
- **Landing page:** [`site/`](site/README.md)
- **Estado:** Fase 1 (MVP) completa — os 11 itens de [`docs/05-roadmap.md`](docs/05-roadmap.md).

## Instalar (uso normal)

Baixe o instalador (`PapaPlay_x.y.z_x64-setup.exe`, ~55 MB) e execute. Na primeira abertura o app oferece baixar o tradutor de frases (~330 MB, uma vez por máquina) — dá para pular e instalar depois em Configurações. Sem ele o dicionário palavra-a-palavra continua funcionando; só a tradução da frase inteira fica indisponível.

Depois desse download, nada mais toca a rede.

## Desenvolvimento

Três artefatos ficam fora do git por serem grandes e regeráveis. Sem eles o app sobe, mas a consulta falha com uma mensagem dizendo qual comando rodar. O deck (`%APPDATA%/papaplay/papaplay.db`) não está nessa lista: ele é criado e migrado sozinho no primeiro salvamento.

```powershell
pnpm install
powershell -File scripts/fetch-ocr-models.ps1   # modelos de OCR, ~11 MB
pnpm run build:dict                             # dict.db, ~42 MB (baixa ~500 MB de dumps, só na 1a vez)
powershell -File scripts/export-nmt.ps1         # tradutor EN->PT em ONNX int8, ~330 MB (precisa de Python)
pnpm tauri dev
```

Em desenvolvimento o tradutor é lido direto de `src-tauri/resources/nmt/`, sem download.

### Publicar uma versão

```powershell
pnpm tauri build     # instalador em src-tauri/target/release/bundle/nsis/
```

Só NSIS. O alvo `msi` saiu do `tauri.conf.json` em 2026-08-08: ele dobrava o tempo de empacotamento, baixava o toolchain do WiX e produzia um artefato que ninguém baixa — a landing e o fluxo de atualização falam só do `setup.exe`. Volta se aparecer uma instalação corporativa que exija MSI.

O `encoder.onnx` e o `decoder.onnx` **não** entram no instalador de propósito. Eles precisam estar publicados numa URL pública, com os nomes e os hashes SHA-256 que estão em `src-tauri/src/setup.rs` (constante `ARTEFATOS`). A base da URL é a constante `URL_BASE` do mesmo arquivo, e a variável de ambiente `PAPAPLAY_MODELS_URL` a sobrescreve para testar antes de publicar.

Na prática, uma vez só:

```powershell
gh release create models-v1 `
  src-tauri/resources/nmt/encoder.onnx `
  src-tauri/resources/nmt/decoder.onnx `
  --title "Tradutor EN->PT (OPUS-MT, ONNX int8)" `
  --notes "Baixado pelo app no primeiro uso. Tag separada da versao do app: so muda quando o modelo mudar."
```

**Marque o release dos modelos como pre-release.** O `releases/latest` do GitHub
é o release mais recente que não é pre-release, de qualquer tag — sem essa marca
o `models-v1` vira o "latest" e o botão de download da landing manda o visitante
para uma página com dois `.onnx` e nenhum instalador. O app não se importa: ele
baixa pela URL da tag, não pela do `latest`.

Depois disso, cada versão do app é um release normal com o instalador anexado:

```powershell
gh release create v0.1.0 `
  src-tauri/target/release/bundle/nsis/PapaPlay_0.1.0_x64-setup.exe `
  --title "PapaPlay 0.1.0" --notes-file NOTAS.md
```

## Como usar

- **Segure `Alt+X`** — espia: lê o entorno do cursor e mostra a tradução da palavra sob ele. Soltar volta ao repouso.
- **Clique** (ou **`Alt+C`**) enquanto espia — abre o card: acepções, IPA, a frase de contexto, a tradução dela e o botão de salvar no deck.
- **`Esc`** — fecha o card.
- **Revisar** — a fila do dia na janela principal: espaço revela a resposta, `1`–`4` avaliam.
- **Fechar a janela encerra o app.** Para espiar durante o jogo, deixe-o aberto (minimizado basta); o ícone da bandeja tem "Abrir" e "Sair".

Os atalhos são configuráveis em Configurações.
