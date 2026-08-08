# site/

A landing do PapaPlay. HTML e CSS escritos à mão, sem build e sem dependência.

No ar em **https://matheusmouraut.github.io/papaplay/**.

```
site/
  index.html    a página inteira
  estilo.css    os estilos, com os tokens do produto no topo
  img/          logo.svg, a captura de jogo do herói e og.png
```

O `og.png` é o cartão que aparece quando o link é compartilhado. Ele é
gerado, não desenhado à mão: um HTML de 1200×630 renderizado pelo Edge em modo
headless, com o mesmo logo e a mesma manchete da página. Refazer é
reescrever esse HTML e tirar um print dele — não há script para isso porque
até hoje foi preciso uma vez só.

Para ver: abra o `index.html` no navegador. Não há servidor de desenvolvimento
porque não há nada para compilar.

## Por que sem framework

A página é um arquivo. Astro, Vite ou Tailwind aqui adicionariam uma segunda
toolchain ao repositório para economizar algumas dezenas de linhas de CSS — e
seriam mais uma coisa para atualizar quando o site ficar seis meses parado.

## As cores estão duplicadas

Os tokens no topo do `estilo.css` são cópia dos de
`src/shared/styles/theme.css`. O app compila com Tailwind e o site não, então
não há como importar um do outro sem arrastar o build do app para cá. **Mudou a
paleta no app, mude aqui também** — é o único ponto de manutenção manual entre
os dois.

O mock do herói usa o desenho real do tooltip (mesmo vidro, mesmo verde, mesmo
sublinhado de 2px), pelo mesmo motivo: um print desatualizado do produto é pior
do que nenhum.

## Publicação

`.github/workflows/site.yml` publica esta pasta no GitHub Pages a cada push na
`main` que toque em `site/`. Antes do primeiro deploy, em **Settings › Pages**,
marcar **Source: GitHub Actions**.

## O botão de download e a armadilha do `latest`

O botão aponta para `releases/latest`, que o GitHub resolve como **o release
mais recente que não é pre-release** — de qualquer tag, inclusive as que não
são versões do app.

Por isso o release dos modelos (`models-v1`, e qualquer `models-vN` futuro)
**precisa estar marcado como pre-release**. Sem isso ele vira o "latest" e o
botão manda o visitante para uma página com dois arquivos `.onnx` e nenhum
instalador — foi exatamente o que aconteceu na primeira publicação.

Marcar é em Releases › o release › _Edit_ › **Set as a pre-release**. Ele
continua público e o app continua baixando dele normalmente: a URL que o
`setup.rs` usa é a da tag, não a do `latest`.
