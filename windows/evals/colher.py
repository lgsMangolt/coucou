r"""Colhe correções reais para o ciclo de melhoria (harness2).

O Coucou anota em Documentos\\Coucou\\Memória\\.coucou\\registro.jsonl, uma linha por evento:
  {"quando": ms, "conta": 1, "chat": "...", "tipo": "coucou" | "sugestao" | "voce", "texto": "...", "conversa": [...]}
- "coucou": o que ele mandou sozinho; "sugestao": o que sugeriu na ilha; "voce": o que você mandou.

Este script acha os momentos em que você respondeu DIFERENTE do Coucou na mesma conversa (até 30 min
depois da resposta/sugestão dele) e grava candidatos, cada um com a
conversa, o que o Coucou disse e o que você disse, em Memória\.coucou\candidatos.jsonl (no cofre, só
neste PC). Você revisa, anonimiza (nomes, telefones, endereços), escreve o "espera" e copia para
evals/pedidos/casos.jsonl (desenvolvimento) ou holdout.jsonl (prova).

Uso (de windows/):  python evals/colher.py
"""
import json
import os
import pathlib
import sys

JANELA_MS = 30 * 60_000
REGISTRO = pathlib.Path(os.path.expanduser("~")) / "Documents" / "Coucou" / "Memória" / ".coucou" / "registro.jsonl"
# Conversas reais de clientes: ficam no cofre (só neste PC), nunca na pasta do projeto, que vai para o
# fork público. Para virar caso de teste, troque nomes, telefones e endereços antes de copiar.
SAIDA = REGISTRO.parent / "candidatos.jsonl"


def parecido(a: str, b: str) -> bool:
    norm = lambda t: " ".join(t.lower().split())
    a, b = norm(a), norm(b)
    return a == b or (len(a) > 20 and (a in b or b in a))


def main():
    sys.stdout.reconfigure(encoding="utf-8")
    if not REGISTRO.exists():
        print(f"Ainda não há registro em {REGISTRO}. Ele aparece quando o Coucou responder ou sugerir algo.")
        return
    eventos = []
    for linha in REGISTRO.read_text(encoding="utf-8").splitlines():
        try:
            eventos.append(json.loads(linha))
        except json.JSONDecodeError:
            continue
    eventos.sort(key=lambda e: e.get("quando", 0))
    ja = set()
    if SAIDA.exists():
        for l in SAIDA.read_text(encoding="utf-8").splitlines():
            try:
                ja.add(json.loads(l)["id"])
            except Exception:
                pass
    novos = []
    for i, e in enumerate(eventos):
        if e.get("tipo") not in ("coucou", "sugestao"):
            continue
        for depois in eventos[i + 1:]:
            if depois.get("quando", 0) - e.get("quando", 0) > JANELA_MS:
                break
            if depois.get("chat") != e.get("chat") or depois.get("tipo") != "voce":
                continue
            if parecido(depois.get("texto", ""), e.get("texto", "")):
                break  # você mandou o que ele disse: não é correção
            cid = f"real-{e['quando']}"
            if cid not in ja:
                novos.append({
                    "id": cid,
                    "fonte": f"registro real ({e['tipo']}) — revisar e escrever o \"espera\"",
                    "conversa": e.get("conversa", []),
                    "coucou": e.get("texto", ""),
                    "voce": depois.get("texto", ""),
                    "espera": None,
                })
            break
    with SAIDA.open("a", encoding="utf-8", newline="\n") as out:
        for c in novos:
            out.write(json.dumps(c, ensure_ascii=False) + "\n")
    print(f"{len(eventos)} eventos lidos; {len(novos)} correções novas em {SAIDA}")
    for c in novos[:10]:
        print(f"- {c['id']}\n  Coucou: {c['coucou'][:120]}\n  Você:   {c['voce'][:120]}")


if __name__ == "__main__":
    main()
