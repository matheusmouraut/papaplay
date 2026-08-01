"""Exporta o tradutor de frases EN->PT para ONNX int8, em `src-tauri/resources/nmt/`.

Modelo: Helsinki-NLP/opus-mt-tc-big-en-pt (Marian/OPUS-MT, CC BY 4.0).

Rodar UMA VEZ, no setup: `powershell -File scripts/export-nmt.ps1`. O núcleo do
app é 100% offline em runtime (regra 2 do CLAUDE.md) -- nada aqui é chamado
durante o uso normal. O wrapper .ps1 é quem monta o venv; este arquivo assume
que torch/transformers/optimum já estão importáveis.

Quatro artefatos saem daqui:

- `encoder.onnx`   -- lê a frase, devolve os estados escondidos.
- `decoder.onnx`   -- variante *merged*: um único grafo que serve tanto o
                      primeiro passo (sem cache) quanto os seguintes (com
                      cache), escolhido pela entrada `use_cache_branch`. Os dois
                      grafos separados que o optimum também gera somam 1,6 GB
                      contra 857 MB do merged, e ambos precisariam ficar na RAM.
- `tokenizer.json` -- ver `construir_tokenizer`: o repo do Helsinki não traz um,
                      porque o `MarianTokenizer` do transformers não tem versão
                      *fast*. Sem este arquivo o lado Rust não tokeniza.
- `meta.json`      -- ids especiais e geometria do cache, para o Rust não
                      precisar repetir constantes que vêm do modelo.

O script falha em vez de gravar um modelo ruim: a validação no fim compara a
tradução do ONNX int8 com a do PyTorch em fp32, palavra por palavra.
"""

from __future__ import annotations

import argparse
import json
import sys
import time
from pathlib import Path

MODELO = "Helsinki-NLP/opus-mt-tc-big-en-pt"
LICENCA = "CC BY 4.0 (Helsinki-NLP / OPUS-MT)"

RAIZ = Path(__file__).resolve().parent.parent
DESTINO_PADRAO = RAIZ / "src-tauri" / "resources" / "nmt"
CACHE = RAIZ / "scripts" / ".cache" / "nmt-export"

# Frases de validação no registro que o app realmente vê: fala de jogo, segunda
# pessoa, imperativo, nomes próprios e pontuação colada.
FRASES_DE_PROVA = [
    "The dread of the deep keeps most sailors ashore.",
    "You must gather your party before venturing forth.",
    "I used to be an adventurer like you, then I took an arrow in the knee.",
    "Press the button to open the gate.",
    "Your journey ends here, wanderer.",
    "The merchant refuses to trade with you until you pay your debt.",
    "A strange light flickers deep within the cave.",
    "She gave up on finding the missing shipment.",
    "Watch out! The bridge is collapsing!",
    "This sword deals extra damage to undead creatures.",
    "Talk to the innkeeper if you need a place to rest.",
    "He was told the war had ended years ago.",
]

# Abaixo disto a quantização estragou o modelo e o build não deve passar.
MINIMO_DE_FRASES_IDENTICAS = 0.75


def log(msg: str) -> None:
    print(msg, flush=True)


# ---------------------------------------------------------------------------
# Passo 1 - export
# ---------------------------------------------------------------------------


def exportar(cache: Path, force: bool) -> Path:
    """Baixa o modelo e escreve os grafos ONNX em fp32."""
    bruto = cache / "fp32"
    if (bruto / "decoder_model_merged.onnx").is_file() and not force:
        log(f"  [ja existe] export fp32 em {bruto}")
        return bruto

    from optimum.exporters.onnx import main_export

    log(f"  exportando {MODELO} (baixa ~900 MB na primeira vez)...")
    # Sem `opset=`: o optimum escolhe o mínimo recomendado para o Marian (18).
    # Fixar 14 aqui gera aviso e um grafo pior.
    main_export(
        model_name_or_path=MODELO,
        output=str(bruto),
        task="text2text-generation-with-past",
    )
    return bruto


# ---------------------------------------------------------------------------
# Passo 2 - quantização
# ---------------------------------------------------------------------------


def _quantizar_arquivo(entrada: Path, saida: Path) -> None:
    """int8 dinâmico, só nos MatMul de peso.

    `MatMulConstBOnly` é o detalhe que decide se a tradução sai legível: sem
    ele o quantizador também pega os MatMul de QK^T e PV, onde os dois lados
    são ativações, e o erro se acumula passo a passo na decodificação.
    """
    from onnxruntime.quantization import QuantType, quantize_dynamic

    antes = entrada.stat().st_size
    log(f"  quantizando {entrada.name} ({antes / 1e6:.0f} MB)...")
    quantize_dynamic(
        model_input=str(entrada),
        model_output=str(saida),
        weight_type=QuantType.QUInt8,
        extra_options={"MatMulConstBOnly": True},
    )
    depois = saida.stat().st_size
    log(f"    -> {saida.name}: {depois / 1e6:.0f} MB ({antes / depois:.1f}x menor)")


def quantizar(origem: Path, destino: Path, cache: Path) -> None:
    """Grava `encoder.onnx` e `decoder.onnx` já em int8.

    O decoder não pode ser quantizado direto do `decoder_model_merged.onnx`:
    naquele grafo os pesos moram dentro dos dois ramos do nó `If`, e o
    quantizador do onnxruntime não desce em subgrafo -- o arquivo sai do outro
    lado com os mesmos 857 MB, sem um único peso convertido, e sem erro nenhum.

    A ordem certa é quantizar os dois decoders soltos e fundir depois. A fusão
    deduplica os pesos (215 MB + 202 MB viram 215 MB), então o merged sai do
    mesmo tamanho de um decoder só -- que é a razão de usá-lo.
    """
    from optimum.onnx.graph_transformations import merge_decoders

    _quantizar_arquivo(origem / "encoder_model.onnx", destino / "encoder.onnx")

    q8 = cache / "q8"
    q8.mkdir(parents=True, exist_ok=True)
    for nome in ("decoder_model.onnx", "decoder_with_past_model.onnx"):
        _quantizar_arquivo(origem / nome, q8 / nome)

    log("  fundindo os dois decoders num grafo só...")
    # strict=False porque o decoder com cache não tem as saídas
    # `present.*.encoder.*`: ele não recalcula a cross-attention. A fusão
    # completa a contagem com constantes -- e é justamente por serem constantes
    # que `traduzir_onnx` só pode aproveitar essas saídas no primeiro passo.
    merge_decoders(
        decoder=q8 / "decoder_model.onnx",
        decoder_with_past=q8 / "decoder_with_past_model.onnx",
        save_path=destino / "decoder.onnx",
        strict=False,
    )
    log(f"    -> decoder.onnx: {(destino / 'decoder.onnx').stat().st_size / 1e6:.0f} MB")


# ---------------------------------------------------------------------------
# Passo 3 - tokenizer
# ---------------------------------------------------------------------------


def construir_tokenizer(origem: Path, destino: Path) -> None:
    """Monta um `tokenizer.json` que o crate `tokenizers` sabe ler.

    O Marian não tem tokenizer *fast* no transformers, então o repo do modelo
    só traz as peças cruas: `source.spm` (o modelo SentencePiece, com os scores
    de cada peça) e `vocab.json` (peça -> id). O id **não** é o índice interno
    do .spm -- é o do vocabulário conjunto de origem e destino --, então os dois
    arquivos precisam ser casados aqui: a tabela do Unigram é indexada por id do
    `vocab.json` e recebe o score da mesma peça no `source.spm`.

    Peças que só existem no lado português não aparecem no `source.spm`. Elas
    entram com um score abaixo do mínimo real para nunca vencerem a segmentação
    do inglês, mas continuam no vocabulário -- é por elas que a *saída* é
    reconstruída.
    """
    from sentencepiece import sentencepiece_model_pb2

    from tokenizers import Tokenizer, decoders, models, normalizers, pre_tokenizers, processors

    spm = sentencepiece_model_pb2.ModelProto()
    spm.ParseFromString((origem / "source.spm").read_bytes())
    scores = {peca.piece: peca.score for peca in spm.pieces}

    vocab: dict[str, int] = json.loads((origem / "vocab.json").read_text(encoding="utf-8"))
    piso = min(scores.values()) - 10.0

    tabela: list[tuple[str, float]] = [("", 0.0)] * (max(vocab.values()) + 1)
    buracos = 0
    for token, indice in vocab.items():
        tabela[indice] = (token, scores.get(token, piso))
    for indice, (token, _) in enumerate(tabela):
        if token == "":
            # Id sem token no vocab.json: precisa de um lugar na tabela mesmo
            # assim, senão todos os ids seguintes andam uma casa.
            tabela[indice] = (f"<vago{indice}>", piso)
            buracos += 1
    if buracos:
        log(f"  {buracos} id(s) sem token no vocab.json, preenchidos com marcador")

    tok = Tokenizer(models.Unigram(tabela, unk_id=vocab["<unk>"], byte_fallback=False))
    tok.normalizer = normalizers.Precompiled(spm.normalizer_spec.precompiled_charsmap)
    # WhitespaceSplit antes do Metaspace: o SentencePiece do Marian foi treinado
    # sem peças que atravessem espaço, e separar por espaço primeiro é o que
    # reproduz os ids dele exatamente (conferido em `validar_tokenizer`).
    tok.pre_tokenizer = pre_tokenizers.Sequence(
        [
            pre_tokenizers.WhitespaceSplit(),
            pre_tokenizers.Metaspace(replacement="▁", prepend_scheme="always"),
        ]
    )
    tok.post_processor = processors.TemplateProcessing(
        single="$A </s>",
        pair="$A </s> $B </s>",
        special_tokens=[("</s>", vocab["</s>"])],
    )
    tok.decoder = decoders.Metaspace(replacement="▁", prepend_scheme="always")

    tok.save(str(destino / "tokenizer.json"))
    log(f"  tokenizer.json: {len(tabela):,} tokens")


def validar_tokenizer(origem_modelo: str, destino: Path, frases: list[str]) -> None:
    """Exige id por id igual ao `MarianTokenizer`, senão o modelo lê outra frase."""
    from transformers import AutoTokenizer

    from tokenizers import Tokenizer

    referencia = AutoTokenizer.from_pretrained(origem_modelo)
    nosso = Tokenizer.from_file(str(destino / "tokenizer.json"))

    divergentes = []
    for frase in frases:
        esperado = referencia(frase)["input_ids"]
        obtido = nosso.encode(frase).ids
        if esperado != obtido:
            divergentes.append((frase, esperado, obtido))

    if divergentes:
        log(f"\n  FALHA: {len(divergentes)}/{len(frases)} frases tokenizam diferente do Marian")
        for frase, esperado, obtido in divergentes[:3]:
            log(f"    {frase!r}")
            log(f"      esperado: {esperado}")
            log(f"      obtido  : {obtido}")
        raise SystemExit(1)
    log(f"  tokenizer confere com o MarianTokenizer em {len(frases)}/{len(frases)} frases")


# ---------------------------------------------------------------------------
# Passo 4 - metadados
# ---------------------------------------------------------------------------


def escrever_meta(origem: Path, destino: Path) -> dict:
    cfg = json.loads((origem / "config.json").read_text(encoding="utf-8"))
    cabecas = cfg["decoder_attention_heads"]
    meta = {
        "model": MODELO,
        "license": LICENCA,
        "decoderStartTokenId": cfg["decoder_start_token_id"],
        "eosTokenId": cfg["eos_token_id"],
        "padTokenId": cfg["pad_token_id"],
        "vocabSize": cfg["vocab_size"],
        "decoderLayers": cfg["decoder_layers"],
        "decoderAttentionHeads": cabecas,
        "headDim": cfg["d_model"] // cabecas,
    }
    (destino / "meta.json").write_text(
        json.dumps(meta, indent=2, ensure_ascii=False) + "\n", encoding="utf-8"
    )
    return meta


# ---------------------------------------------------------------------------
# Passo 5 - validação
# ---------------------------------------------------------------------------


def abrir_sessoes(destino: Path):
    """Tokenizer + as duas sessões ONNX. Carregar custa segundos; reaproveitar."""
    import onnxruntime as ort

    from tokenizers import Tokenizer

    opcoes = ort.SessionOptions()
    return (
        Tokenizer.from_file(str(destino / "tokenizer.json")),
        ort.InferenceSession(
            str(destino / "encoder.onnx"), opcoes, providers=["CPUExecutionProvider"]
        ),
        ort.InferenceSession(
            str(destino / "decoder.onnx"), opcoes, providers=["CPUExecutionProvider"]
        ),
    )


def traduzir_onnx(sessoes, meta: dict, frase: str, maximo: int = 128) -> str:
    """Decodificação gulosa sobre os grafos exportados.

    É também a especificação executável do que `src-tauri/src/translate.rs`
    precisa fazer -- se os dois divergirem, este é o lado certo.
    """
    import numpy as np

    tok, enc_sess, dec_sess = sessoes

    ids = np.array([tok.encode(frase).ids], dtype=np.int64)
    mask = np.ones_like(ids)
    estados = enc_sess.run(None, {"input_ids": ids, "attention_mask": mask})[0]

    camadas = meta["decoderLayers"]
    cabecas = meta["decoderAttentionHeads"]
    dim = meta["headDim"]
    vazio = np.zeros((1, cabecas, 0, dim), dtype=np.float32)
    passado = {}
    for i in range(camadas):
        for lado in ("decoder", "encoder"):
            for parte in ("key", "value"):
                passado[f"past_key_values.{i}.{lado}.{parte}"] = vazio

    nomes_de_saida = [s.name for s in dec_sess.get_outputs()]
    token = meta["decoderStartTokenId"]
    saida: list[int] = []
    usa_cache = False
    for _ in range(maximo):
        entradas = {
            "input_ids": np.array([[token]], dtype=np.int64),
            "encoder_attention_mask": mask,
            "encoder_hidden_states": estados,
            "use_cache_branch": np.array([usa_cache], dtype=bool),
            **passado,
        }
        resultado = dec_sess.run(None, entradas)
        logits = resultado[0]
        token = int(logits[0, -1].argmax())
        if token == meta["eosTokenId"]:
            break
        saida.append(token)

        for nome, valor in zip(nomes_de_saida, resultado):
            if not nome.startswith("present."):
                continue
            miolo = nome[len("present.") :]
            # ARMADILHA: o KV do encoder so presta no primeiro passo. Do segundo
            # em diante o grafo entra no ramo com cache, que nao recalcula a
            # cross-attention -- os `present.*.encoder.*` de la sao degenerados,
            # e realimenta-los explode com "cannot broadcast on dim 0" no passo
            # seguinte. Cross-attention nao muda durante a decodificacao: o
            # valor do primeiro passo vale ate o fim.
            if ".encoder." in miolo and usa_cache:
                continue
            passado["past_key_values." + miolo] = valor
        usa_cache = True

    return tok.decode(saida)


def validar_traducao(destino: Path, meta: dict, frases: list[str]) -> None:
    """Compara ONNX int8 com o PyTorch fp32 na mesma decodificação gulosa."""
    import torch
    from transformers import AutoTokenizer, MarianMTModel

    log("\n  carregando o modelo de referência em fp32 para comparar...")
    tok = AutoTokenizer.from_pretrained(MODELO)
    modelo = MarianMTModel.from_pretrained(MODELO).eval()
    sessoes = abrir_sessoes(destino)

    iguais = 0
    for frase in frases:
        with torch.no_grad():
            gerado = modelo.generate(**tok(frase, return_tensors="pt"), num_beams=1, do_sample=False)
        esperado = tok.decode(gerado[0], skip_special_tokens=True).strip()

        comeco = time.perf_counter()
        obtido = traduzir_onnx(sessoes, meta, frase).strip()
        ms = (time.perf_counter() - comeco) * 1000

        marca = "ok " if obtido == esperado else "DIF"
        log(f"    [{marca}] {ms:6.0f} ms  {frase}")
        log(f"           int8: {obtido}")
        if obtido != esperado:
            log(f"           fp32: {esperado}")
        iguais += obtido == esperado

    taxa = iguais / len(frases)
    log(f"\n  {iguais}/{len(frases)} traduções idênticas ao fp32 ({taxa:.0%})")
    if taxa < MINIMO_DE_FRASES_IDENTICAS:
        log(f"  FALHA: abaixo do mínimo de {MINIMO_DE_FRASES_IDENTICAS:.0%}")
        raise SystemExit(1)


# ---------------------------------------------------------------------------


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--out", type=Path, default=DESTINO_PADRAO)
    ap.add_argument("--force", action="store_true", help="refaz o export fp32")
    ap.add_argument("--skip-validation", action="store_true")
    args = ap.parse_args()

    destino: Path = args.out
    destino.mkdir(parents=True, exist_ok=True)
    CACHE.mkdir(parents=True, exist_ok=True)

    log(f"Destino: {destino}\n")

    log("1/5 export ONNX fp32")
    bruto = exportar(CACHE, args.force)

    log("\n2/5 quantização int8")
    quantizar(bruto, destino, CACHE)

    log("\n3/5 tokenizer")
    construir_tokenizer(bruto, destino)
    validar_tokenizer(MODELO, destino, FRASES_DE_PROVA)

    log("\n4/5 metadados")
    meta = escrever_meta(bruto, destino)
    log(f"  meta.json: {meta}")

    if args.skip_validation:
        log("\n5/5 validação PULADA (--skip-validation)")
    else:
        log("\n5/5 validação da tradução")
        validar_traducao(destino, meta, FRASES_DE_PROVA)

    total = sum(p.stat().st_size for p in destino.iterdir() if p.is_file())
    log(f"\nPronto. {total / 1e6:.0f} MB em {destino}")


if __name__ == "__main__":
    sys.exit(main())
