# Open Productivity Deck

Um deck de produtividade livre para Windows 10/11. O aplicativo desktop controla o áudio do computador, abre aplicativos configurados pelo usuário e serve uma interface para o celular diretamente na rede local.

## Recursos

- Controle circular do volume com marcador, área neutra e feedback tátil no celular.
- Botão para mutar e restaurar o som.
- Controle independente para mutar o microfone padrão.
- Painel mobile aberto pelo navegador, sem aplicativo adicional.
- Layout mobile otimizado para uso horizontal, sem barra superior.
- Instalação como PWA por HTTPS local.
- Pareamento por QR Code com token aleatório revogável.
- Atalhos configuráveis para executáveis, arquivos, URLs e protocolos do Windows.
- Ícones personalizados em PNG, JPEG, WebP ou SVG para os aplicativos.
- Importação automática do ícone ao selecionar um executável ou atalho do Windows.
- Migração automática dos logos instalados do Teams e WhatsApp.
- Execução em segundo plano pela bandeja do Windows.
- Contadores experimentais de mensagens não lidas do Teams e WhatsApp.
- Atualização em tempo real dos badges pelo canal SSE autenticado.
- Player do Spotify com título, artista, pausar, avançar e voltar.
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
3. Na primeira vez, baixe e instale o certificado local indicado pela página.
4. Abra o endereço seguro e instale a PWA pelo navegador.
5. Controle o volume ou use os atalhos pelo aplicativo instalado.
6. Configure novos botões no computador e selecione **Publicar alterações**.
7. Use **Invalidar e gerar novo QR** para encerrar acessos anteriores.

Fechar a janela pelo X mantém o deck e o servidor local funcionando. Clique no ícone da bandeja do Windows para abrir novamente; use **Sair** no menu da bandeja para encerrar completamente.

Ao selecionar um executável ou atalho, o aplicativo tenta importar automaticamente seu ícone associado. Também é possível escolher manualmente uma imagem de até 256 KB em PNG, JPEG, WebP ou SVG. O conteúdo do ícone é incorporado à configuração, portanto o caminho original não é enviado ao celular.

O player do Spotify aparece abaixo dos controles de áudio no celular. Ele seleciona especificamente a sessão de mídia do Spotify no Windows, mostra a faixa atual e permite pausar, reproduzir, avançar ou voltar. Se o Spotify estiver fechado ou ainda não tiver iniciado uma música, o card permanece desabilitado.

Aplicativos instalados pela Microsoft Store podem ser abertos por seus protocolos registrados. Os padrões iniciais usam `msteams:` e `whatsapp:`; caso uma instalação não registre esses protocolos, selecione o executável ou atalho correspondente.

## Instalação PWA

A PWA utiliza duas portas na rede local:

- `37621`: página HTTP usada somente para baixar o certificado e iniciar o pareamento.
- `37622`: aplicação e API protegidas por HTTPS.

No Android, instale o arquivo `.cer` como certificado de CA nas configurações de segurança. Depois, abra o aplicativo seguro e use **Instalar** no Chrome.

No iPhone ou iPad, instale o perfil baixado e habilite a confiança em **Ajustes > Geral > Sobre > Ajustes de Confiança de Certificados**. Em seguida, use **Compartilhar > Adicionar à Tela de Início**.

A autoridade certificadora e sua chave privada são criadas no computador. Somente o certificado público é disponibilizado para download. Se o IP local mudar, o computador gera automaticamente um novo certificado de servidor assinado pela mesma autoridade.

## Autenticação persistente

O token é armazenado em arquivo no computador e no `localStorage` da origem HTTPS no celular. Reiniciar o computador, fechar a PWA ou remover apenas o WebAPK no Android não gera um novo token. Ao reinstalar sem limpar os dados do site, a autenticação anterior é reutilizada.

Limpar os dados do navegador, trocar o IP local ou usar **Invalidar e gerar novo QR** exige um novo pareamento. No iOS, o sistema pode remover os dados locais junto com o aplicativo da Tela de Início, portanto essa persistência após desinstalar não é garantida pela Apple.

## Segurança

O servidor escuta as portas TCP `37621` e `37622` na rede local. O token contido no QR Code é necessário em todas as ações. O painel remoto recebe somente identificadores, rótulos, ícones e cores dos botões; caminhos e protocolos configurados não são enviados ao celular.

O bootstrap usa HTTP, mas nenhuma ação de controle é aceita nele. A aplicação e a API funcionam somente no servidor HTTPS local. Não instale o certificado em dispositivos que você não controla e não use o aplicativo em redes públicas ou não confiáveis.

A configuração fica no diretório de configuração do usuário, normalmente em `%APPDATA%\OpenProductivity\Open Productivity Deck\config\deck.json`.

## Contadores não lidos

Teams e WhatsApp não oferecem uma API pública estável para consultar mensagens não lidas de contas pessoais no desktop. Esta versão consulta somente os badges numéricos mantidos pela central de notificações do Windows e usa os títulos das janelas como fallback:

- O conteúdo das notificações e das mensagens não é consultado.
- O contador continua disponível quando o aplicativo está fechado, desde que o badge permaneça registrado pelo Windows.
- Se o Windows não oferecer um badge numérico, o aplicativo aberto aparece como zero.
- Atualizações do Teams ou WhatsApp podem interromper a detecção.

O aplicativo monitora as alterações da central de notificações e envia o novo contador imediatamente à PWA por Server-Sent Events. Se o monitoramento de arquivos não estiver disponível, a recuperação consulta o estado a cada dois segundos.

## Licença

Copyright (C) 2026 Open Productivity Deck contributors.

Este projeto é software livre, distribuído sob a GNU General Public License, versão 3 ou qualquer versão posterior (`GPL-3.0-or-later`). Ele é fornecido sem qualquer garantia. Consulte `LICENSE` e <https://www.gnu.org/licenses/>.
