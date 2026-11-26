# Lições do ciclo de melhoria (harness2)

Cada rodada: linha de base → erros classificados → uma mudança pequena por vez → mesma medição → adota só com evidência. Nenhum peso de modelo é alterado (Haiku/Sonnet/Fable são fechados); o que muda é prompt, regra em código e casos de teste.

## Rodada 1 — 26/11/2026 (0.17.1)

**Linha de base** (claude-haiku-5-5): pedidos dev 29,3/30 em 3 rodadas; holdout novo (7 casos de conversas reais não usadas, gabarito = o que o Lucas respondeu) 6/7; gráfica 12/12 e holdout 7/7.

| Erro | Tipo | Mudança | Evidência | Decisão |
|---|---|---|---|---|
| "papel brilhoso tipo foto" ia para o gerente; o Lucas responde couchê 300 | fato (vocabulário) | Instrução: "brilhoso"/"tipo foto" = couchê (pergunta 150 ou 300); papel com nome próprio (folha de desenho, fosco, kraft, fotográfico, adesivo) = gerente | 1ª versão ("mais grosso" também) errou "folha de desenho grossa" no holdout; versão final: dev 30/31/31, holdout 6/6 | adotada |
| colorida sem papel dito às vezes virava ofício sem perguntar | comportamento | Código confere (`orders::paper_unsaid`): colorida sem palavra de papel na conversa vira a pergunta "folha ofício ou couchê 150g / 300g?" | Mudou 2 gabaritos (A3 e A5): o Lucas decidiu "pergunta ofício ou couchê" | adotada (regra do dono) |

**Final:** dev 31, 31, 30 de 31 (a variação que sobra é perguntar "frente e verso?" num TCC — o Lucas também pergunta); holdout 6/6.

**Limites:** o caso do "brilhoso" saiu do holdout e foi para o dev (foi usado para ajustar); o holdout foi olhado 3 vezes nesta rodada, então vale como sinal, não prova. Próxima rodada precisa de casos novos de conversas reais para o holdout.

**Revisar quando:** o Lucas corrigir uma resposta do Coucou no WhatsApp (vira caso novo), ou a tabela ganhar papéis novos.

## Como colher casos novos (0.20.0)
O Coucou anota em `Documentos\Coucou\Memória\.coucou\registro.jsonl` o que respondeu/sugeriu e o que o Lucas mandou depois. `python evals/colher.py` grava em `.coucou/candidatos.jsonl` (no cofre) as vezes em que o Lucas respondeu diferente. Antes de copiar um candidato para `evals/pedidos`, troque nomes, telefones e endereços: esta pasta vai para o fork público.
