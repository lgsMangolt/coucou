"""Roda os pedidos do grupo da gráfica (anonimizados) pelo mesmo caminho do Coucou e confere os
campos que o modelo extrai. As regras (vias, folhas, encaixe) são do código e têm testes no Rust.

Uso (de windows/):
  GRAFICA_SYSTEM=evals/grafica/system.txt cargo test -p coucou grafica::tests::write_the_system_prompt -- --ignored
  python evals/grafica/avaliar.py evals/grafica/system.txt claude-haiku-5-5
"""
import concurrent.futures as cf, json, pathlib, subprocess, sys

HERE = pathlib.Path(__file__).parent

def ask(model, system, prompt):
    cmd = ["claude", "-p", "--output-format", "json", "--model", model, "--system-prompt", system, "--tools", "",
           "--setting-sources", "", "--strict-mcp-config", "--no-session-persistence", "--disable-slash-commands"]
    out = subprocess.run(cmd, input=prompt, capture_output=True, text=True, encoding="utf-8", timeout=180)
    return json.loads(out.stdout)["result"]

def jobs(reply):
    s, e = reply.find("{"), reply.rfind("}")
    try:
        return json.loads(reply[s:e + 1]).get("servicos", [])
    except Exception:
        return None

def check(want, got):
    if got is None:
        return ["resposta inválida"]
    errs = []
    if len(got) != len(want):
        errs.append(f"{len(got)} serviços, esperava {len(want)}")
    for w in want:
        g = next((x for x in got if str(x.get("arquivo", "")).strip() == w["arquivo"]), None)
        if g is None:
            errs.append(f"faltou {w['arquivo']}")
            continue
        for k, v in w.items():
            if k == "arquivo":
                continue
            if k == "medida":
                if sorted([g.get("largura_cm"), g.get("altura_cm")], key=lambda x: x or 0) != sorted(v):
                    errs.append(f"medida {g.get('largura_cm')}x{g.get('altura_cm')}")
                continue
            have = g.get(k)
            if k == "os" and have is not None:
                have = "".join(ch for ch in str(have) if ch.isdigit())
            if k == "produto" and have:
                have = have.lower()
            if have != v:
                errs.append(f"{k}={have!r} (esperava {v!r})")
    return errs

def main():
    system = pathlib.Path(sys.argv[1]).read_text(encoding="utf-8")
    model = sys.argv[2] if len(sys.argv) > 2 else "claude-haiku-5-5"
    cases = [json.loads(l) for l in (HERE / __import__("os").environ.get("CASOS", "casos.jsonl")).read_text(encoding="utf-8").splitlines() if l.strip()]
    def run(c):
        try:
            return c, check(c["espera"], jobs(ask(model, system, "Group messages:\n" + "\n".join(c["conversa"]))))
        except Exception as e:
            return c, [f"erro: {e}"]
    with cf.ThreadPoolExecutor(4) as pool:
        results = list(pool.map(run, cases))
    sys.stdout.reconfigure(encoding="utf-8")
    ok = 0
    for c, errs in results:
        ok += not errs
        print(("ok  " if not errs else "ERR ") + c["id"] + ("" if not errs else "  " + "; ".join(errs)))
    print(f"acerto: {ok}/{len(cases)}")

if __name__ == "__main__":
    main()
