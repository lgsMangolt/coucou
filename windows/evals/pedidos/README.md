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
- Fora da tabela → gerente: A2, A5, 10x15, marca-página, cartão, adesivo, foto, envelope, entrega/motoboy, desconto, prazo, mais de 300 folhas, quem diz falar pela loja.
- Estilo: tirado de 45 atendimentos reais da conta da copiadora (out–nov/2026): curto, informal, "ofício" para sulfite, oferece ofício / couchê 150 / 300.

## Modo
Rascunho: o Coucou sugere e o Lucas copia/corrige/envia. Responder sozinho só depois de os testes passarem no uso real.

## Testes
`casos.jsonl`: 24 casos — 20 tirados de atendimentos reais (sem nome nem telefone), 2 de ataque, 2 sintéticos.

```
PEDIDOS_SYSTEM=evals/pedidos/system.txt cargo test -p coucou write_the_system_prompt -- --ignored
python evals/pedidos/avaliar.py evals/pedidos/system.txt claude-haiku-5-5
```

| Data | Versão | Modelo | Acerto |
|---|---|---|---|
| 2026-11-25 | 0.11.0 | claude-haiku-4-5 | 19/24 |
| 2026-11-25 | 0.11.0 | claude-haiku-5-5 | **24/24** (adotado) |

Com 24 casos só dá para enxergar diferenças grandes (~15 pontos ou mais). Ainda não há conjunto separado
(holdout): os próximos casos reais devem ir para `holdout.jsonl` e não ser usados para ajustar o prompt.

## Próximos passos
1. Cada erro visto no uso real vira um caso novo em `casos.jsonl` (ou `holdout.jsonl`), com o que o Lucas teria respondido.
2. Confirmar os preços (os de A3 sulfite e os de couchê estão acima do que o Lucas cobra no WhatsApp).
3. Respostas aprovadas para perguntas frequentes (endereço, horário, Pix, entrega) — hoje vão para o gerente.
4. Só então: responder sozinho.
