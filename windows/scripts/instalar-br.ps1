# Instala o Coucou (versao BR) em silencio: espera o Coucou fechar, roda o instalador com /S
# e abre o app de novo. Escreve o que fez em instalar.log, ao lado do instalador.
#
# Uso: powershell -ExecutionPolicy Bypass -File instalar-br.ps1 -Instalador <caminho do setup.exe>
#
# Obs.: dentro do app Claude (que e um app da loja) o Windows desvia o que um instalador grava em
# AppData. Para instalar de verdade, este script deve rodar fora dele, por exemplo por uma tarefa
# agendada que se apaga depois (ver instalar-via-tarefa.ps1).
param(
  [Parameter(Mandatory = $true)][string]$Instalador,
  [int]$EsperaSegundos = 120,
  [switch]$NaoAbrir
)
$ErrorActionPreference = 'Stop'
$log = Join-Path (Split-Path -Parent $Instalador) 'instalar.log'
function Log($mensagem) { ("{0:HH:mm:ss} {1}" -f (Get-Date), $mensagem) | Out-File -FilePath $log -Append -Encoding ascii }
Set-Content -Path $log -Value '' -Encoding ascii

if (-not (Test-Path $Instalador)) { Log "instalador nao encontrado: $Instalador"; exit 2 }

$aberto = Get-Process coucou -ErrorAction SilentlyContinue
if ($aberto) {
  Log "Coucou aberto: esperando fechar (ate $EsperaSegundos s)"
  $aberto | Wait-Process -Timeout $EsperaSegundos -ErrorAction SilentlyContinue
}
if (Get-Process coucou -ErrorAction SilentlyContinue) { Log 'Coucou ainda aberto: nao instalei'; exit 3 }

Log ("instalando " + (Split-Path -Leaf $Instalador))
# Espera so o instalador (nao a arvore de processos dele): -Wait ficava preso se ele abrisse o app.
$p = Start-Process -FilePath $Instalador -ArgumentList '/S' -PassThru
if (-not $p.WaitForExit($EsperaSegundos * 1000)) { Log 'o instalador demorou demais'; exit 4 }
Log ("instalador terminou com codigo " + $p.ExitCode)
if ($p.ExitCode -ne 0) { exit $p.ExitCode }

$exe = Join-Path $env:LOCALAPPDATA 'Coucou\coucou.exe'
Log ("programa instalado: " + $exe + " (" + (Get-Item $exe).Length + " bytes)")
if (-not $NaoAbrir -and (Test-Path $exe)) { Start-Process -FilePath $exe; Log 'Coucou aberto' }
Log 'fim'
