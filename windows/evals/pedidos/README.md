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
Rascunho: o Coucou sugere e o Lucas copia/corrige/envia. Responder sozinho só depois de os testes passarem no uso real.

## Testes
`casos.jsonl`: 27 casos — 20 tirados de atendimentos reais (sem nome nem telefone), 2 de ataque, 5 sintéticos.

```
PEDIDOS_SYSTEM=evals/pedidos/system.txt cargo test -p coucou write_the_system_prompt -- --ignored
python evals/pedidos/avaliar.py evals/pedidos/system.txt claude-haiku-5-5
```

| Data | Versão | Modelo | Acerto |
|---|---|---|---|
| 2026-11-25 | 0.11.0 | claude-haiku-4-5 | 19/24 |
| 2026-11-25 | 0.11.0 | claude-haiku-5-5 | **24/24** (adotado) |
| 2026-11-25 | 0.12.0 (tamanhos, junção) | claude-haiku-5-5 | **27/27** |

Com 24 casos só dá para enxergar diferenças grandes (~15 pontos ou mais). Ainda não há conjunto separado
(holdout): os próximos casos reais devem ir para `holdout.jsonl` e não ser usados para ajustar o prompt.

## Próximos passos
1. Cada erro visto no uso real vira um caso novo em `casos.jsonl` (ou `holdout.jsonl`), com o que o Lucas teria respondido.
2. Confirmar as faixas 11–100 do A3 P&B (estimativa).
3. Respostas aprovadas para perguntas frequentes (endereço, horário, Pix, entrega) — hoje vão para o gerente.
4. Só então: responder sozinho.
