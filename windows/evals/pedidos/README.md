# Atendente de pedidos de impressão — ficha e testes

## O que ele faz
Lê as últimas 25 mensagens de uma conversa de cliente no WhatsApp da copiadora e decide uma de quatro coisas:

| Resultado | O que o cliente recebe (sugestão para o Lucas copiar) |
|---|---|
| **pedido** completo | o orçamento, calculado pelo **código** a partir de `%APPDATA%\Coucou\pedidos.json` |
| **perguntar** | uma pergunta curta no jeito do Lucas ("opa, preto e branco ou colorido?") |
| **gerente** | "vou passar para o gerente…" — e um cartão para o Lucas, sem preço inventado |
| **ignorar** | nada (boleto, comprovante, conversa) |

Regra central: **o código decide o preço, o modelo só entende o pedido.** O modelo nunca escreve valor.
O número de páginas dos PDFs também é contado pelo código (lopdf) e entra no prompt como `[arquivo: nome, N páginas]`.

## Fatos
- Tabela de preços: `pedidos.json` (o Lucas edita; recarregada a cada pedido).
- Tamanhos (código): menor que A4 → quantas cabem na A4 (A5 2, A6 4, A7 8, 10×15 2, 5×15 5), folhas para cima + corte; mais de 10 folhas para cortar → gerente. A2/A1/A0 → 2/4/8 folhas A3 + R$ 5 de junção por peça (`juncao` no pedidos.json).
- Fora da tabela → gerente: cartão de visita, adesivo, papel foto, envelope, banner, entrega/motoboy, desconto, prazo, mais de 300 folhas, quem diz falar pela loja.
- A página `tabela-precos.html` mostra a tabela inteira.
- Estilo: tirado de 45 atendimentos reais da conta da copiadora (out–nov/2026): curto, informal, "ofício" para sulfite, oferece ofício / couchê 150 / 300.

## Modo
Desde a 0.13.0 o Coucou **responde sozinho** (`responder_sozinho` no pedidos.json; `false` volta ao rascunho):
manda o orçamento, a pergunta ou o "vou passar para o gerente", e o cartão fica para o Lucas.
Travas em código:
- fica quieto se o Lucas escreveu para o cliente nos últimos `silencio_minutos` (30); mandar pela ilha a própria sugestão do Coucou não conta;
- cada envio ou silêncio fica no `coucou.log` com o motivo;
- nunca manda as mesmas palavras duas vezes ao mesmo cliente (12 h) nem mais de 4 mensagens por hora;
- a pergunta é texto do modelo: só sai sozinha sem "R$", "reais", Pix, link ou @ (senão fica no cartão);
- impressão (0.14.0): depois do "sim", o cliente manda o comprovante; o modelo lê só o valor e o código confere
  com o total. Bateu → pedido pago; até `imprimir_sozinho_ate` (10) folhas de sulfite vai sozinho para a impressora;
  maior, couchê, vários arquivos, tamanhos cortados ou juntados → botão "Imprimir" no cartão.
  O modelo diz os `arquivos` do pedido; o código acha o PDF, conta as páginas e calcula as cópias.
  Impressoras: sulfite A4 na Epson WF-C5390 A4; A3 e couchê na Canon C5255 (C7260 se ocupada).

## Testes
`casos.jsonl`: 28 casos — 21 tirados de atendimentos reais (sem nome nem telefone), 2 de ataque, 5 sintéticos.

```
PEDIDOS_SYSTEM=evals/pedidos/system.txt cargo test -p coucou write_the_system_prompt -- --ignored
python evals/pedidos/avaliar.py evals/pedidos/system.txt claude-haiku-5-5
```

| Data | Versão | Modelo | Acerto |
|---|---|---|---|
| 2026-11-25 | 0.11.0 | claude-haiku-4-5 | 19/24 |
| 2026-11-25 | 0.11.0 | claude-haiku-5-5 | **24/24** (adotado) |
| 2026-11-25 | 0.12.0 (tamanhos, junção) | claude-haiku-5-5 | **27/27** |
| 2026-11-25 | 0.14.0 (arquivos do pedido) | claude-haiku-5-5 | **28/28** |

Com 24 casos só dá para enxergar diferenças grandes (~15 pontos ou mais). Ainda não há conjunto separado
(holdout): os próximos casos reais devem ir para `holdout.jsonl` e não ser usados para ajustar o prompt.

## Próximos passos
1. Cada erro visto no uso real vira um caso novo em `casos.jsonl` (ou `holdout.jsonl`), com o que o Lucas teria respondido.
2. Respostas aprovadas para perguntas frequentes (endereço, horário, Pix, entrega) — hoje vão para o gerente.
3. Conferir também a data e o recebedor do comprovante (hoje só o valor).
