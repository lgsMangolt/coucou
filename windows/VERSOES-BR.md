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

## Como gerar e instalar

```powershell
cd windows
npm run pack                      # gera os instaladores em windows\release
powershell -ExecutionPolicy Bypass -File scripts\instalar-br.ps1 -Instalador release\Coucou-BR-setup.exe
```

`instalar-br.ps1` espera o Coucou fechar, instala em silêncio e abre o app de novo.
