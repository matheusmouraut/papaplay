import { describe, expect, it } from "vitest";

import { decodeMsgpack, ranksDoWordfreq } from "./msgpack.mjs";

const bytes = (...b) => Buffer.from(b);

describe("decodeMsgpack", () => {
  it("lê inteiros curtos dos dois sinais", () => {
    expect(decodeMsgpack(bytes(0x00))).toBe(0);
    expect(decodeMsgpack(bytes(0x7f))).toBe(127);
    expect(decodeMsgpack(bytes(0xff))).toBe(-1);
    expect(decodeMsgpack(bytes(0xe0))).toBe(-32);
  });

  it("lê inteiros largos", () => {
    expect(decodeMsgpack(bytes(0xcc, 0xff))).toBe(255);
    expect(decodeMsgpack(bytes(0xcd, 0x01, 0x00))).toBe(256);
    expect(decodeMsgpack(bytes(0xce, 0x00, 0x01, 0x00, 0x00))).toBe(65536);
    expect(decodeMsgpack(bytes(0xd1, 0xff, 0x00))).toBe(-256);
  });

  it("lê strings curtas e longas em utf-8", () => {
    // 0xa3 = string de 3 bytes.
    expect(
      decodeMsgpack(Buffer.concat([bytes(0xa3), Buffer.from("run")])),
    ).toBe("run");
    const longa = "á".repeat(40); // 80 bytes em utf-8, passa do fixstr
    const buf = Buffer.concat([bytes(0xd9, 80), Buffer.from(longa)]);
    expect(decodeMsgpack(buf)).toBe(longa);
  });

  it("lê arrays aninhados", () => {
    // [1, [2, 3]]
    expect(decodeMsgpack(bytes(0x92, 0x01, 0x92, 0x02, 0x03))).toEqual([
      1,
      [2, 3],
    ]);
  });

  it("lê o cabeçalho do wordfreq (mapa dentro de array)", () => {
    // [{"format": "cB"}, []] — a forma real do arquivo.
    const buf = Buffer.concat([
      bytes(0x92, 0x81, 0xa6),
      Buffer.from("format"),
      bytes(0xa2),
      Buffer.from("cB"),
      bytes(0x90),
    ]);
    expect(decodeMsgpack(buf)).toEqual([{ format: "cB" }, []]);
  });

  it("lê array16, que é o que o arquivo de verdade usa", () => {
    const buf = Buffer.concat([bytes(0xdc, 0x00, 0x02), bytes(0x01, 0x02)]);
    expect(decodeMsgpack(buf)).toEqual([1, 2]);
  });

  it("falha alto em byte não suportado", () => {
    // Melhor quebrar o build do que devolver ranking silenciosamente errado.
    expect(() => decodeMsgpack(bytes(0xc7))).toThrow(/não suportado/);
  });
});

describe("ranksDoWordfreq", () => {
  it("numera as palavras na ordem das faixas, da mais comum para a mais rara", () => {
    const dados = [{ format: "cB" }, ["the", "of"], ["run", "dread"]];
    const ranks = ranksDoWordfreq(dados);
    expect(ranks.get("the")).toBe(1);
    expect(ranks.get("of")).toBe(2);
    expect(ranks.get("run")).toBe(3);
    expect(ranks.get("dread")).toBe(4);
  });

  it("mantém a primeira aparição quando a palavra se repete", () => {
    // Faixas anteriores são mais frequentes: a repetição não pode rebaixar.
    const ranks = ranksDoWordfreq([{}, ["run"], ["run"]]);
    expect(ranks.get("run")).toBe(1);
    expect(ranks.size).toBe(1);
  });

  it("recusa dados fora do formato esperado", () => {
    expect(() => ranksDoWordfreq([{ format: "cB" }])).toThrow(/faixas/);
    expect(() => ranksDoWordfreq(null)).toThrow(/faixas/);
  });
});
