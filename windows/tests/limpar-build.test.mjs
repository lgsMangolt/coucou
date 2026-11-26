// A limpeza do que as compilacoes deixam para tras (scripts/limpar-build.mjs).

import { test } from "node:test";
import assert from "node:assert/strict";
import { existsSync, mkdirSync, mkdtempSync, rmSync, utimesSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { limpar, versaoDe } from "../scripts/limpar-build.mjs";

function pasta(caminho, segundos) {
  mkdirSync(join(caminho, "out"), { recursive: true });
  writeFileSync(join(caminho, "out", "x.lib"), "1234");
  utimesSync(caminho, segundos, segundos);
}

test("fica so a pasta de build mais nova de cada pacote do projeto; as de dependencias nao sao tocadas", () => {
  const raiz = mkdtempSync(join(tmpdir(), "limpar-"));
  try {
    const build = join(raiz, "target", "release", "build");
    pasta(join(build, "coucou", "velha"), 1000);
    pasta(join(build, "coucou", "nova"), 3000);
    pasta(join(build, "coucou-hook", "a"), 2000);
    pasta(join(build, "coucou-hook", "b"), 1000);
    pasta(join(build, "sherpa-onnx-sys", "x"), 1000);
    pasta(join(build, "sherpa-onnx-sys", "y"), 2000);
    pasta(join(raiz, "target", "debug", "build", "coucou", "d1"), 1000);
    pasta(join(raiz, "target", "debug", "build", "coucou", "d2"), 2000);

    const { alvos, bytes } = limpar(raiz);
    assert.equal(alvos.length, 3);
    assert.equal(bytes, 12);
    assert.ok(existsSync(join(build, "coucou", "nova")));
    assert.ok(!existsSync(join(build, "coucou", "velha")));
    assert.ok(existsSync(join(build, "coucou-hook", "a")));
    assert.ok(!existsSync(join(build, "coucou-hook", "b")));
    assert.ok(existsSync(join(build, "sherpa-onnx-sys", "x")));
    assert.ok(existsSync(join(raiz, "target", "debug", "build", "coucou", "d2")));
    assert.ok(!existsSync(join(raiz, "target", "debug", "build", "coucou", "d1")));
  } finally {
    rmSync(raiz, { recursive: true, force: true });
  }
});

test("instaladores: ficam as 3 versoes mais novas e os nomes fixos; 0.10 vem depois de 0.9", () => {
  const raiz = mkdtempSync(join(tmpdir(), "limpar-"));
  try {
    const rel = join(raiz, "release");
    mkdirSync(rel, { recursive: true });
    const nomes = [
      "Coucou-BR-0.9.0-gmail-setup.exe", "Coucou-BR-0.9.0-gmail.msi",
      "Coucou-BR-0.10.0-pedidos-setup.exe",
      "Coucou-BR-0.14.1-a-setup.exe", "Coucou-BR-0.14.2-b-setup.exe", "Coucou-BR-0.14.2-b.msi",
      "Coucou-BR-setup.exe", "Coucou-BR.msi", "instalar.log",
    ];
    for (const n of nomes) writeFileSync(join(rel, n), "x");
    mkdirSync(join(rel, "antigas"));

    limpar(raiz);
    for (const n of ["Coucou-BR-0.9.0-gmail-setup.exe", "Coucou-BR-0.9.0-gmail.msi"]) {
      assert.ok(!existsSync(join(rel, n)), n);
    }
    for (const n of nomes.slice(2)) assert.ok(existsSync(join(rel, n)), n);
    assert.ok(existsSync(join(rel, "antigas")));
  } finally {
    rmSync(raiz, { recursive: true, force: true });
  }
});

test("simular so lista, nao apaga", () => {
  const raiz = mkdtempSync(join(tmpdir(), "limpar-"));
  try {
    const build = join(raiz, "target", "release", "build", "coucou");
    pasta(join(build, "velha"), 1000);
    pasta(join(build, "nova"), 2000);
    const { alvos } = limpar(raiz, { simular: true });
    assert.equal(alvos.length, 1);
    assert.ok(existsSync(join(build, "velha")));
  } finally {
    rmSync(raiz, { recursive: true, force: true });
  }
});

test("versaoDe le a versao do nome e ignora os nomes fixos", () => {
  assert.deepEqual(versaoDe("Coucou-BR-0.14.2-codigo-do-comprovante-setup.exe"), [0, 14, 2]);
  assert.equal(versaoDe("Coucou-BR-setup.exe"), null);
});
