#!/usr/bin/env python3
"""Gera a planilha de controle financeiro do Coucou BR e o script que a liga ao Coucou.

  python gerar-planilha.py --saida "C:\\Users\\voce\\Documents\\Coucou"

Escreve, na pasta dada:
  Controle Financeiro Coucou.xlsx    a planilha (Lançamentos, Resumo, Como usar); abre no Excel,
                                     no LibreOffice e no Google Planilhas (Arquivo > Importar)
  apps-script-coucou.gs              o script que o Google Planilhas usa para receber os lançamentos
O código que liga os dois é sorteado aqui (ou dado em --codigo) e vai só dentro do .gs: o mesmo
código é digitado uma vez em Configurações > WhatsApp > Anotações de dinheiro. Apague o .gs depois
de colar no Google. Requer: pip install xlsxwriter
"""

import argparse
import os
import secrets
import sys

try:
    import xlsxwriter
except ImportError:
    sys.exit("Falta a biblioteca xlsxwriter: pip install xlsxwriter")

ABA = "Lançamentos"
DADOS = f"'{ABA}'"
CATEGORIAS = ["Mercado", "Alimentação", "Transporte", "Luz", "Água", "Internet", "Aluguel", "Saúde", "Lazer", "Material", "Salário", "Cliente"]
LINHAS_PESSOAS = 8


def planilha(caminho: str) -> None:
    wb = xlsxwriter.Workbook(caminho)
    wb.set_properties({"title": "Controle financeiro Coucou", "comments": "Gerada por gerar-planilha.py (Coucou BR)"})

    dinheiro = '"R$" #,##0.00;[Red]-"R$" #,##0.00'
    f = {
        "titulo": wb.add_format({"bold": True, "font_size": 18, "font_color": "#111827"}),
        "dica": wb.add_format({"italic": True, "font_color": "#6B7280"}),
        "h2": wb.add_format({"bold": True, "font_size": 12, "bottom": 1, "font_color": "#111827"}),
        "cab": wb.add_format({"bold": True, "bg_color": "#1F2937", "font_color": "#FFFFFF", "align": "center", "valign": "vcenter"}),
        "rot": wb.add_format({"bold": True}),
        "din": wb.add_format({"num_format": dinheiro}),
        "din_forte": wb.add_format({"num_format": dinheiro, "bold": True, "font_size": 12}),
        "mes": wb.add_format({"num_format": "mmm/yyyy", "align": "left"}),
        "mes_escolha": wb.add_format({"num_format": "mmmm/yyyy", "bg_color": "#FEF3C7", "border": 1, "bold": True, "align": "left"}),
        "campo": wb.add_format({"bg_color": "#FEF3C7", "border": 1}),
        "data": wb.add_format({"num_format": "dd/mm/yyyy", "align": "left"}),
        "texto": wb.add_format({"num_format": "@"}),
        "quebra": wb.add_format({"text_wrap": True, "valign": "top"}),
        "negrito_quebra": wb.add_format({"text_wrap": True, "valign": "top", "bold": True}),
        "vermelho": wb.add_format({"font_color": "#B91C1C"}),
    }

    resumo = wb.add_worksheet("Resumo")
    lanc = wb.add_worksheet(ABA)
    ajuda = wb.add_worksheet("Como usar")

    # ── Lançamentos: o que o Coucou escreve, uma linha por anotação ──
    colunas = ["Data", "Tipo", "Valor", "Categoria", "Pessoa", "Descrição"]
    for i, nome in enumerate(colunas):
        lanc.write(0, i, nome, f["cab"])
    lanc.set_row(0, 22)
    lanc.set_column("A:A", 13, f["data"])
    lanc.set_column("B:B", 11)
    lanc.set_column("C:C", 15, f["din"])
    # Texto puro: o que vem do celular nunca vira fórmula.
    lanc.set_column("D:E", 20, f["texto"])
    lanc.set_column("F:F", 44, f["texto"])
    lanc.freeze_panes(1, 0)
    lanc.autofilter(0, 0, 0, 5)
    lanc.data_validation("B2:B5000", {"validate": "list", "source": ["Gasto", "Entrada"], "error_type": "warning"})
    lanc.write(0, 7, "O Coucou acrescenta as linhas aqui embaixo. Gasto é negativo, Entrada é positivo.", f["dica"])

    # ── Resumo ──
    resumo.set_column("A:A", 24)
    resumo.set_column("B:D", 16)
    resumo.set_column("E:E", 3)
    resumo.write("A1", "Controle financeiro", f["titulo"])
    resumo.write("A2", "Os lançamentos vêm do Coucou (WhatsApp). Veja a aba “Como usar”.", f["dica"])

    resumo.write("A4", "Mês", f["rot"])
    resumo.write_formula("B4", "=DATE(YEAR(TODAY()),MONTH(TODAY()),1)", f["mes_escolha"])
    resumo.write("C4", "← digite outra data para ver outro mês", f["dica"])

    def soma_mes(celula_mes, sinal, extra=""):
        return (f"SUMIFS({DADOS}!$C:$C,{DADOS}!$A:$A,\">=\"&{celula_mes},{DADOS}!$A:$A,\"<\"&EDATE({celula_mes},1),"
                f"{DADOS}!$C:$C,\"{sinal}\"{extra})")

    resumo.write("A6", "Entradas do mês", f["rot"])
    resumo.write_formula("B6", "=" + soma_mes("$B$4", ">0"), f["din_forte"])
    resumo.write("A7", "Gastos do mês", f["rot"])
    resumo.write_formula("B7", "=-" + soma_mes("$B$4", "<0"), f["din_forte"])
    resumo.write("A8", "Saldo do mês", f["rot"])
    resumo.write_formula("B8", "=B6-B7", f["din_forte"])
    resumo.write("A9", "Saldo de tudo", f["rot"])
    resumo.write_formula("B9", f"=SUM({DADOS}!$C:$C)", f["din_forte"])
    resumo.conditional_format("B8:B9", {"type": "cell", "criteria": "<", "value": 0, "format": f["vermelho"]})

    # Mês a mês: os 12 meses que terminam no mês escolhido.
    resumo.write("A11", "Mês a mês (12 meses até o escolhido)", f["h2"])
    for i, nome in enumerate(["Mês", "Entradas", "Gastos", "Saldo"]):
        resumo.write(11, i, nome, f["cab"])
    for i in range(12):
        r = 12 + i
        xl = r + 1
        resumo.write_formula(r, 0, f"=EDATE($B$4,{i - 11})", f["mes"])
        resumo.write_formula(r, 1, "=" + soma_mes(f"$A{xl}", ">0"), f["din"])
        resumo.write_formula(r, 2, "=-" + soma_mes(f"$A{xl}", "<0"), f["din"])
        resumo.write_formula(r, 3, f"=B{xl}-C{xl}", f["din"])
    resumo.conditional_format("D13:D24", {"type": "cell", "criteria": "<", "value": 0, "format": f["vermelho"]})

    # Por categoria, no mês escolhido. A lista da coluna A é sua: troque os nomes à vontade.
    resumo.write("A26", "Por categoria (no mês escolhido)", f["h2"])
    for i, nome in enumerate(["Categoria", "Gastos", "Entradas"]):
        resumo.write(26, i, nome, f["cab"])
    for i, nome in enumerate(CATEGORIAS):
        r = 27 + i
        xl = r + 1
        resumo.write(r, 0, nome, f["campo"])
        resumo.write_formula(r, 1, "=-" + soma_mes("$B$4", "<0", f",{DADOS}!$D:$D,$A{xl}"), f["din"])
        resumo.write_formula(r, 2, "=" + soma_mes("$B$4", ">0", f",{DADOS}!$D:$D,$A{xl}"), f["din"])
    ultima = 27 + len(CATEGORIAS)
    resumo.write(ultima, 0, "Outras categorias", f["rot"])
    resumo.write_formula(ultima, 1, f"=$B$7-SUM(B28:B{ultima})", f["din"])
    resumo.write_formula(ultima, 2, f"=$B$6-SUM(C28:C{ultima})", f["din"])

    # Por pessoa, desde o início: quem recebeu e quem pagou.
    topo = ultima + 3
    resumo.write(topo - 1, 0, "Por pessoa (desde o início)", f["h2"])
    for i, nome in enumerate(["Pessoa", "Gastos", "Entradas", "Saldo"]):
        resumo.write(topo, i, nome, f["cab"])
    for i in range(LINHAS_PESSOAS):
        r = topo + 1 + i
        xl = r + 1
        resumo.write_blank(r, 0, None, f["campo"])
        resumo.write_formula(r, 1, f"=IF($A{xl}=\"\",\"\",-SUMIFS({DADOS}!$C:$C,{DADOS}!$E:$E,$A{xl},{DADOS}!$C:$C,\"<0\"))", f["din"])
        resumo.write_formula(r, 2, f"=IF($A{xl}=\"\",\"\",SUMIFS({DADOS}!$C:$C,{DADOS}!$E:$E,$A{xl},{DADOS}!$C:$C,\">0\"))", f["din"])
        resumo.write_formula(r, 3, f"=IF($A{xl}=\"\",\"\",C{xl}-B{xl})", f["din"])
    resumo.write(topo + 1 + LINHAS_PESSOAS, 0, "Escreva nos campos amarelos os nomes que você usa nas anotações.", f["dica"])

    # Gráficos
    g1 = wb.add_chart({"type": "column"})
    g1.add_series({"name": "Entradas", "categories": "=Resumo!$A$13:$A$24", "values": "=Resumo!$B$13:$B$24", "fill": {"color": "#16A34A"}})
    g1.add_series({"name": "Gastos", "categories": "=Resumo!$A$13:$A$24", "values": "=Resumo!$C$13:$C$24", "fill": {"color": "#DC2626"}})
    g1.set_title({"name": "Entradas e gastos por mês"})
    g1.set_legend({"position": "bottom"})
    g1.set_size({"width": 560, "height": 300})
    resumo.insert_chart("F4", g1)

    g2 = wb.add_chart({"type": "bar"})
    g2.add_series({
        "name": "Gastos", "categories": f"=Resumo!$A$28:$A${ultima + 1}", "values": f"=Resumo!$B$28:$B${ultima + 1}",
        "fill": {"color": "#DC2626"},
    })
    g2.set_title({"name": "Gastos por categoria (mês escolhido)"})
    g2.set_legend({"none": True})
    g2.set_y_axis({"reverse": True})
    g2.set_size({"width": 560, "height": 340})
    resumo.insert_chart("F20", g2)

    # ── Como usar ──
    ajuda.set_column("A:A", 3)
    ajuda.set_column("B:B", 104)
    linhas = [
        ("t", "Como esta planilha funciona"),
        ("", "O Coucou lê as suas anotações no WhatsApp e, quando você toca em Salvar no cartão da ilha, acrescenta uma linha na aba “Lançamentos”. A aba “Resumo” soma tudo sozinha."),
        ("t", "1. Anotando"),
        ("", "No grupo que você criou para as anotações, escreva do seu jeito: “gastei 35 mercado”, “entrou 800 cliente Rafael”, “paguei 120 de luz pro João”. O Coucou lê só o que você escreve, e só mensagens com número."),
        ("", "Aparece na ilha um cartão com tipo, valor e categoria. Toque em Salvar para gravar, ou em Descartar."),
        ("t", "2. Ligando o Coucou a esta planilha no Google Planilhas (celular e computador)"),
        ("", "a) Arquivo > Importar > Enviar, e escolha este arquivo (substituir a planilha, se já houver uma)."),
        ("", "b) Nesta planilha: Extensões > Apps Script. Apague o que estiver lá e cole o conteúdo do arquivo apps-script-coucou.gs."),
        ("", "c) No Apps Script: Implantar > Nova implantação > tipo “App da Web” > Executar como: Eu > Quem pode acessar: Qualquer pessoa > Implantar. Autorize quando o Google pedir. Copie o endereço que termina em /exec."),
        ("", "d) No Coucou: Configurações > Integrações > WhatsApp > Anotações de dinheiro. Cole o endereço em “Endereço da Planilha Google”, digite o código que está na primeira linha do script (CODIGO) em “Código da planilha” e toque em Salvar e depois em Testar a planilha."),
        ("", "O código impede que qualquer pessoa com o endereço escreva na sua planilha. Não compartilhe o endereço nem o código."),
        ("t", "3. Sem o Google Planilhas"),
        ("", "O Coucou também guarda tudo no arquivo lancamentos.csv (na pasta Documentos\\Coucou). É uma cópia local, que abre em qualquer programa de planilha."),
        ("t", "4. Ajustes"),
        ("", "Na aba Resumo, os campos amarelos são seus: o mês que você quer ver, os nomes das categorias e os nomes das pessoas. Se uma categoria não estiver na lista, ela entra em “Outras categorias”."),
        ("", "Gasto é negativo e Entrada é positivo na coluna Valor. Se corrigir um lançamento à mão, mantenha esse sinal."),
    ]
    r = 0
    for tipo, texto in linhas:
        if tipo == "t":
            ajuda.write(r + 1, 1, texto, f["h2"])
        else:
            ajuda.write(r + 1, 1, texto, f["quebra"])
            ajuda.set_row(r + 1, 15 * (1 + len(texto) // 105))
        r += 1
    resumo.activate()
    wb.close()


SCRIPT = '''// Coucou BR -> Planilha Google.
//
// Como usar: nesta planilha, Extensoes > Apps Script. Apague o que houver, cole este codigo e
// va em Implantar > Nova implantacao > "App da Web" > Executar como: Eu > Quem pode acessar:
// Qualquer pessoa. Copie o endereco (termina em /exec) para o Coucou, junto com o CODIGO abaixo.
// Guarde o CODIGO como uma senha: ele impede que outra pessoa escreva na sua planilha.

const CODIGO = "%(codigo)s";
const ABA = "Lançamentos";

function doPost(e) {
  const trava = LockService.getScriptLock();
  try {
    const dados = JSON.parse(e.postData.contents);
    if (dados.code !== CODIGO) return resposta({ ok: false, error: "code" });
    if (dados.ping) return resposta({ ok: true, pong: true });

    const aba = SpreadsheetApp.getActive().getSheetByName(ABA);
    if (!aba) return resposta({ ok: false, error: "aba" });

    trava.waitLock(10000);
    const linha = aba.getLastRow() + 1;
    // Texto puro nas colunas de texto: nada que venha do celular vira formula.
    aba.getRange(linha, 4, 1, 3).setNumberFormat("@");
    aba.getRange(linha, 1, 1, 6).setValues([[
      new Date(dados.date + "T12:00:00"),
      String(dados.kind || ""),
      Number(dados.value),
      String(dados.category || ""),
      String(dados.person || ""),
      String(dados.note || ""),
    ]]);
    aba.getRange(linha, 1).setNumberFormat("dd/mm/yyyy");
    aba.getRange(linha, 3).setNumberFormat('"R$" #,##0.00;[Red]-"R$" #,##0.00');
    SpreadsheetApp.flush();
    return resposta({ ok: true });
  } catch (erro) {
    return resposta({ ok: false, error: String(erro).slice(0, 40) });
  } finally {
    try { trava.releaseLock(); } catch (_) { /* nao tinha a trava */ }
  }
}

function resposta(objeto) {
  return ContentService.createTextOutput(JSON.stringify(objeto)).setMimeType(ContentService.MimeType.JSON);
}
'''


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--saida", required=True, help="pasta onde gravar os dois arquivos")
    ap.add_argument("--codigo", help="código que liga o Coucou ao script (se faltar, é sorteado)")
    args = ap.parse_args()
    os.makedirs(args.saida, exist_ok=True)
    codigo = args.codigo or secrets.token_urlsafe(18)
    xlsx = os.path.join(args.saida, "Controle Financeiro Coucou.xlsx")
    gs = os.path.join(args.saida, "apps-script-coucou.gs")
    planilha(xlsx)
    with open(gs, "w", encoding="utf-8", newline="\n") as out:
        out.write(SCRIPT % {"codigo": codigo})
    print(xlsx)
    print(gs)
    print("CODIGO:", codigo)


if __name__ == "__main__":
    main()
