// Limpa o que cada compilacao deixa para tras e enche o disco.
//
// Cada `tauri build` com algo diferente (versao nova, outra variavel de ambiente,
// `cargo test` x `pack`) cria uma pasta nova em target/<perfil>/build/<pacote>/<hash>,
// e a pasta do app chega a 1,3 GB. O cargo nunca apaga as antigas: em dois dias
// eram 49 pastas (17 GB). Aqui fica so a mais nova de cada pacote do projeto
// (apagar uma que ainda servia so faz o cargo refazer aquele passo).
//
// Tambem apaga os instaladores de versoes antigas em windows/release/, mantendo
// as ultimas MANTER_VERSOES e os nomes fixos (Coucou-BR-setup.exe, Coucou-BR.msi).
//
// Roda sozinho no fim do `npm run pack`. A mao: `npm run limpar` (ou
// `node scripts/limpar-build.mjs --simular` para so listar).

import { readdirSync, rmSync, statSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

/** Pacotes do proprio projeto cujas pastas de build se acumulam. */
export const PACOTES = ["coucou", "coucou-hook"];
/** Quantas versoes de instalador ficam em windows/release/. */
export const MANTER_VERSOES = 3;

function tamanho(caminho) {
  let st;
  try {
    st = statSync(caminho);
  } catch {
    return 0;
  }
  if (!st.isDirectory()) return st.size;
  let total = 0;
  for (const nome of readdirSync(caminho)) total += tamanho(join(caminho, nome));
  return total;
}

function subpastas(dir) {
  try {
    return readdirSync(dir, { withFileTypes: true })
      .filter((e) => e.isDirectory())
      .map((e) => join(dir, e.name));
  } catch {
    return [];
  }
}

/** Pastas de build antigas: em target/<perfil>/build/<pacote>/, todas menos a mais nova. */
export function buildsAntigas(target, pacotes = PACOTES) {
  const antigas = [];
  for (const perfil of subpastas(target)) {
    for (const pacote of pacotes) {
      const hashes = subpastas(join(perfil, "build", pacote))
        .map((p) => ({ p, t: statSync(p).mtimeMs }))
        .sort((a, b) => b.t - a.t);
      antigas.push(...hashes.slice(1).map((h) => h.p));
    }
  }
  return antigas;
}

/** Versao de um instalador "Coucou-BR-0.14.2-nome-setup.exe" -> [0, 14, 2]; null nos nomes fixos. */
export function versaoDe(arquivo) {
  const m = /^Coucou-BR-(\d+)\.(\d+)\.(\d+)-/.exec(arquivo);
  return m ? m.slice(1).map(Number) : null;
}

/** Instaladores de versoes antigas em release/: tudo fora das `manter` versoes mais novas. */
export function instaladoresAntigos(release, manter = MANTER_VERSOES) {
  let arquivos = [];
  try {
    arquivos = readdirSync(release).filter((f) => statSync(join(release, f)).isFile());
  } catch {
    return [];
  }
  const chave = (v) => v.join(".");
  const versoes = [...new Map(arquivos.map(versaoDe).filter(Boolean).map((v) => [chave(v), v])).values()]
    .sort((a, b) => b[0] - a[0] || b[1] - a[1] || b[2] - a[2]);
  const ficam = new Set(versoes.slice(0, manter).map(chave));
  return arquivos
    .filter((f) => {
      const v = versaoDe(f);
      return v && !ficam.has(chave(v));
    })
    .map((f) => join(release, f));
}

export function limpar(raiz, { simular = false } = {}) {
  const alvos = [...buildsAntigas(join(raiz, "target")), ...instaladoresAntigos(join(raiz, "release"))];
  let bytes = 0;
  for (const alvo of alvos) {
    bytes += tamanho(alvo);
    if (!simular) rmSync(alvo, { recursive: true, force: true });
  }
  return { alvos, bytes };
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const raiz = resolve(dirname(fileURLToPath(import.meta.url)), "..");
  const simular = process.argv.includes("--simular");
  const { alvos, bytes } = limpar(raiz, { simular });
  const gb = (bytes / 1024 ** 3).toFixed(2);
  console.log(`\n  Limpeza de builds${simular ? " (simulacao)" : ""}: ${alvos.length} item(ns), ${gb} GB${simular ? " seriam liberados" : " liberados"}\n`);
}
