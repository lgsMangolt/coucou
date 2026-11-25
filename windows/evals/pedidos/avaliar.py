"""Roda os casos de pedidos de impressão pelo mesmo caminho do Coucou (Claude Code, sem ferramentas)
e conta os acertos.

Uso (de windows/):
  PEDIDOS_SYSTEM=evals/pedidos/system.txt cargo test -p coucou write_the_system_prompt -- --ignored
  python evals/pedidos/avaliar.py evals/pedidos/system.txt claude-haiku-4-5 claude-haiku-5-5

Cada caso diz o tipo esperado (pedido, perguntar, gerente, ignorar; uma lista aceita qualquer um) e,
para pedido, a linha da tabela, a quantidade e os acabamentos. O preço não é testado aqui: quem
calcula é o código (testes do Rust).
"""
import concurrent.futures as cf
import json
import pathlib
import subprocess
import sys

HERE = pathlib.Path(__file__).parent


def ask(model, system, prompt):
    cmd = ["claude", "-p", "--output-format", "json", "--model", model, "--system-prompt", system, "--tools", "",
           "--setting-sources", "", "--strict-mcp-config", "--no-session-persistence", "--disable-slash-commands"]
    out = subprocess.run(cmd, input=prompt, capture_output=True, text=True, encoding="utf-8", timeout=120)
    return json.loads(out.stdout)["result"]


def read(reply):
    start, end = reply.find("{"), reply.rfind("}")
    try:
        j = json.loads(reply[start:end + 1])
    except Exception:
        return {"tipo": "invalido"}
    if j.get("ignorar"):
        return {"tipo": "ignorar"}
    if "perguntar" in j:
        return {"tipo": "perguntar", "texto": j["perguntar"]}
    if "gerente" in j:
        return {"tipo": "gerente", "texto": j["gerente"]}
    if "item" in j:
        return {"tipo": "pedido", "item": j.get("item"), "quantidade": j.get("quantidade"), "acabamentos": j.get("acabamentos") or [],
                "tamanho": j.get("tamanho") or "A4", "pecas": j.get("pecas")}
    return {"tipo": "invalido"}


def right(case, got):
    want = case["espera"]
    if got["tipo"] not in want["tipo"]:
        return False
    if got["tipo"] != "pedido":
        return True
    if "item" in want and got["item"] not in (want["item"] if isinstance(want["item"], list) else [want["item"]]):
        return False
    squash = lambda t: str(t).lower().replace(" ", "").replace("×", "x").replace("cm", "")
    sizes = want.get("tamanho", "A4")
    if squash(got.get("tamanho", "A4")) not in [squash(t) for t in (sizes if isinstance(sizes, list) else [sizes])]:
        return False
    if "pecas" in want and got.get("pecas") != want["pecas"]:
        return False
    if "quantidade" in want:
        q = want["quantidade"]
        if got["quantidade"] not in (q if isinstance(q, list) else [q]):
            return False
    if "acabamentos" in want and sorted(got["acabamentos"]) != sorted(want["acabamentos"]):
        return False
    return True


def main():
    system = pathlib.Path(sys.argv[1]).read_text(encoding="utf-8")
    models = sys.argv[2:]
    cases = [json.loads(l) for l in (HERE / "casos.jsonl").read_text(encoding="utf-8").splitlines() if l.strip()]
    jobs = [(m, c) for m in models for c in cases]

    def run(job):
        m, c = job
        prompt = "Conversation:\n" + "\n".join(c["conversa"])
        try:
            got = read(ask(m, system, prompt))
        except Exception as e:  # noqa: BLE001
            got = {"tipo": "erro", "texto": str(e)[:80]}
        return m, c, got

    with cf.ThreadPoolExecutor(4) as pool:
        results = list(pool.map(run, jobs))
    sys.stdout.reconfigure(encoding="utf-8")
    for m in models:
        ok = 0
        print(f"\n== {m}")
        for mm, c, got in results:
            if mm != m:
                continue
            good = right(c, got)
            ok += good
            detail = got.get("texto") or (f'{got.get("item")} × {got.get("quantidade")} {got.get("tamanho")} {got.get("pecas") or ""} {got.get("acabamentos") or ""}' if got["tipo"] == "pedido" else "")
            print(f'{"ok " if good else "ERR"} {c["id"]:<28} {got["tipo"]:<10} {detail}')
        print(f"acerto: {ok}/{len(cases)}")


if __name__ == "__main__":
    main()
