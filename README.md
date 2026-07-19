# Open Productivity Deck

Um deck de produtividade livre para Windows 10/11. O aplicativo desktop controla o áudio do computador, abre aplicativos configurados pelo usuário e serve uma interface para o celular diretamente na rede local.

## Recursos

- Controle circular do volume de saída padrão do Windows.
- Botão para mutar e restaurar o som.
- Controle independente para mutar o microfone padrão.
- Painel mobile aberto pelo navegador, sem aplicativo adicional.
- Pareamento por QR Code com token aleatório revogável.
- Atalhos configuráveis para executáveis, arquivos, URLs e protocolos do Windows.
- Ícones personalizados em PNG, JPEG, WebP ou SVG para os aplicativos.
- Execução em segundo plano pela bandeja do Windows.
- Contadores experimentais de mensagens não lidas do Teams e WhatsApp.
- Configuração local, sem conta, nuvem ou telemetria.

## Requisitos

- Windows 10 ou Windows 11.
- Microsoft Edge WebView2, incluído nas versões atuais do Windows.
- Node.js 20 ou mais recente para desenvolvimento.
- Rust estável e ferramentas MSVC para desenvolvimento.
- Computador e celular conectados à mesma rede local.

## Desenvolvimento

```powershell
npm install
npm run tauri dev
```

Na primeira execução, o Windows pode solicitar autorização para comunicação em redes privadas. Autorize apenas em redes confiáveis para permitir o acesso do celular.

## Compilação

```powershell
npm run build
npm run tauri build
```

O instalador NSIS é criado em `src-tauri/target/release/bundle/nsis`.

## Uso

1. Abra o aplicativo no computador.
2. Leia o QR Code usando a câmera do celular.
3. Controle o volume ou use os atalhos pelo navegador.
4. Configure novos botões no computador e selecione **Publicar alterações**.
5. Use **Invalidar e gerar novo QR** para encerrar acessos anteriores.

Fechar a janela pelo X mantém o deck e o servidor local funcionando. Clique no ícone da bandeja do Windows para abrir novamente; use **Sair** no menu da bandeja para encerrar completamente.

Cada atalho pode receber uma imagem de até 256 KB. O conteúdo do ícone é incorporado à configuração, portanto o caminho original da imagem não é enviado ao celular.

Aplicativos instalados pela Microsoft Store podem ser abertos por seus protocolos registrados. Os padrões iniciais usam `msteams:` e `whatsapp:`; caso uma instalação não registre esses protocolos, selecione o executável ou atalho correspondente.

## Segurança

O servidor escuta a porta TCP `37621` na rede local. O token contido no QR Code é necessário em todas as ações. O painel remoto recebe somente identificadores, rótulos e cores dos botões; caminhos e protocolos configurados não são enviados ao celular.

O acesso usa HTTP local, sem certificado TLS. Não use o aplicativo em redes públicas ou não confiáveis. Gere um novo QR Code ao perder o controle de um dispositivo pareado.

A configuração fica no diretório de configuração do usuário, normalmente em `%APPDATA%\OpenProductivity\Open Productivity Deck\config\deck.json`.

## Contadores não lidos

Teams e WhatsApp não oferecem uma API pública estável para consultar mensagens não lidas de contas pessoais no desktop. Esta versão procura números nos títulos das janelas dos processos locais:

- Nenhum conteúdo de mensagem é lido.
- Um aplicativo fechado aparece sem contador.
- Um aplicativo aberto sem número detectável aparece como zero.
- Atualizações do Teams ou WhatsApp podem interromper a detecção.

## Licença

Copyright (C) 2026 Open Productivity Deck contributors.

Este projeto é software livre, distribuído sob a GNU General Public License, versão 3 ou qualquer versão posterior (`GPL-3.0-or-later`). Ele é fornecido sem qualquer garantia. Consulte `LICENSE` e <https://www.gnu.org/licenses/>.
