# PapaPlay

Aprenda inglês jogando. Overlay para Windows: segure `Alt+X`, aponte para uma palavra na tela do jogo e veja a tradução na hora. Salve no deck e revise depois, com repetição espaçada. Offline.

**[Baixar para Windows](https://github.com/matheusmouraut/papaplay/releases/latest/download/PapaPlay-setup.exe)** · 30 MB · Windows 10 e 11

**[Site do projeto →](https://matheusmouraut.github.io/papaplay/)**

## O que tem neste repositório

A **landing page** (`site/`) e os **downloads** (na aba Releases). O código-fonte do aplicativo é fechado.

```
site/                     a página, HTML e CSS à mão, sem build
.github/workflows/        publica site/ no GitHub Pages
```

## Releases

| Tag | O que é |
| --- | --- |
| `v0.1.0` (e seguintes) | O instalador. `PapaPlay-setup.exe` é o nome estável para links de download; o arquivo com a versão no nome fica anexado ao lado. |
| `models-v1` | O tradutor de frases (~330 MB), que o app baixa sozinho na primeira execução. Marcado como pre-release para não virar o "latest" no lugar do instalador. |

## Problemas e sugestões

Abra uma [issue](https://github.com/matheusmouraut/papaplay/issues). Se for um erro, ajuda muito saber o jogo, o que apareceu na tela e o que você esperava que aparecesse.

## Créditos

Dicionário derivado do [Wiktionary](https://www.wiktionary.org/) (CC BY-SA). Tradução por [OPUS-MT](https://github.com/Helsinki-NLP/Opus-MT), da Universidade de Helsinque (CC BY).
