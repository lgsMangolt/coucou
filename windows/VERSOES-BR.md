# Versões do Coucou BR

Cada versão tem um número (a partir de 0.7.0, acima do original) e um nome. O nome vai no arquivo do
instalador: `Coucou-BR-<número>-<nome>-setup.exe`. `Coucou-BR-setup.exe` é sempre a mais nova.
Cada versão é um commit marcado com a tag `br-v<número>` neste repositório.

| Versão | Nome | O que traz |
|---|---|---|
| 0.7.0 | Base brasileira | Português do Brasil como padrão. WhatsApp: sugestão de resposta e anotação de gastos para planilha. Aba Modos (Trabalho, Descanso, Música). Pílulas novas: YouTube Music, Impressoras, Tarefas e Bloco de notas. O atualizador olha o fork, não o repositório original. |
| 0.7.1 | Acesso rápido | Seletor de modos no cabeçalho da ilha. Linha "Ligar" sob as pílulas com as opções que estão fora do modo, as mais usadas primeiro. O Coucou aprende o seu uso: uma ferramenta não aberta por 14 dias sai do modo (com aviso e Desfazer), e uma opção oferecida por 30 dias sem uso deixa de aparecer. Pílulas que avisam sozinhas (WhatsApp, e-mail, impressoras) nunca saem por falta de uso. |
| 0.7.2 | Planilha e grupo | As anotações de dinheiro podem vir de um grupo do WhatsApp que você criou (não só da conversa com você mesmo) e vão também para uma Planilha Google, pelo celular ou pelo computador, por um script do próprio Google. O CSV local continua como cópia. Inclui o gerador da planilha (scripts/gerar-planilha.py). |
| 0.8.0 | Dois WhatsApps | Uma segunda conta de WhatsApp, com número, pílula, conversas e gateway próprios (o mesmo módulo compilado duas vezes, então a primeira conta não muda). O modo Descanso mantém só o WhatsApp 2, para você ser avisado e responder com um clique. Cada conta guarda a sessão em pasta separada (GOWA e GOWA2). |
| 0.8.1 | Descanso tranquilo | Ignorar grupos do WhatsApp com mais pessoas que um limite (por conta). Correção: o cartão do YouTube Music não saía de "Entrar" depois do login. O Claude Desktop pode ser a ferramenta principal. A linha "Ligar" não passa mais da borda do cartão. |
| 0.8.2 | Busca na ilha | A busca do YouTube Music cabe no cartão: a lupa abre uma tela só da busca, com o campo e até 8 resultados, e a seta volta para a música. |
| 0.9.0 | Gmail na ilha | Pílula nova do Gmail: os e-mails não lidos (quem mandou e o assunto), aviso quando chega um novo, e um clique abre a mensagem no Gmail. O login é o do Google, numa janela do próprio Coucou (o mesmo do YouTube Music), que fica aberta e escondida na caixa de entrada. O Coucou não envia, não marca e não apaga e-mails. |
| 0.10.0 | Pedidos de impressão | Pedido de impressão pelo WhatsApp: quando um cliente manda um arquivo e pede a impressão, o Coucou lê o pedido, calcula o orçamento pela tabela de preços (`pedidos.json`), escolhe a impressora pelas regras da loja e mostra um cartão. O orçamento só é enviado com um clique; o "sim" do cliente marca o pedido como aprovado; "Pegar arquivo" baixa o arquivo para Documentos\\Coucou\\Pedidos. Só mensagens novas são lidas. O Gmail passa a mostrar só os e-mails que chegam depois de o Coucou abrir. |
| 0.10.1 | Preços de Caxias | A tabela de preços dos pedidos de impressão passa a ter valores para Caxias do Sul: P&B a partir de R$ 1,00 a folha, colorido a partir de R$ 2,50, couchê A4 de R$ 6,00 a R$ 7,50 e A3 de R$ 12,00 a R$ 15,00, encadernação de R$ 9,00 e R$ 12,00, plastificação R$ 6,00 a folha. |
| 0.10.2 | Acabamentos | Preços dos acabamentos definidos pelo Lucas: corte R$ 10,00, encadernação até 100 folhas R$ 15,00, até 200 folhas R$ 20,00, plastificação R$ 10,00 a folha. |

## Como gerar e instalar

```powershell
cd windows
npm run pack                      # gera os instaladores em windows\release
powershell -ExecutionPolicy Bypass -File scripts\instalar-br.ps1 -Instalador release\Coucou-BR-setup.exe
```

`instalar-br.ps1` espera o Coucou fechar, instala em silêncio e abre o app de novo.
