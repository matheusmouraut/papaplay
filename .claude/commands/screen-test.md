---
description: Roda o OCR contra as fixtures de screenshots e compara com o gabarito
---

1. Para cada imagem em `tests/fixtures/screens/*.png|jpg` com um `<nome>.expected.json` correspondente, rode o pipeline de OCR do projeto.
2. Compare palavras reconhecidas vs gabarito: calcule recall (palavras do gabarito encontradas), precision (palavras encontradas que existem no gabarito) e erro médio de bbox quando disponível.
3. Reporte tabela por imagem + agregado, e a latência média por frame.
4. Compare com a última execução registrada em `tests/fixtures/screens/baseline.json`; destaque regressões >2%.
5. Se o usuário aprovar, atualize `baseline.json`.

Formato do gabarito (`<nome>.expected.json`): `{ "words": ["dread", "told"], "notes": "fonte estilizada, fundo animado" }`.
