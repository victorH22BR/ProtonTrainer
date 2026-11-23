# Arquitetura do ProtonTrainer

> Estado: MVP em implementação. Descoberta Steam/Proton, catálogo local de
> trainers, persistência e lançamento explícito estão implementados. Formatos e
> processos da Steam ainda precisam de validação em instalações reais variadas.

## Princípios

1. A lógica de negócio fica em crates Rust independentes do Tauri.
2. Caminhos e AppIDs são descobertos e validados; o aplicativo não inventa prefixos.
3. Processos externos são iniciados com argumentos separados, sem shell.
4. Trainers só são importados por referência e só executados após ação explícita.
5. Fixtures temporárias cobrem parsers e persistência sem depender da Steam real.

## Componentes

| Componente | Responsabilidade | Estado |
|---|---|---|
| `pt-core` | tipos compartilhados (`AppId`) | implementado |
| `pt-steam` | encontrar raízes, bibliotecas e manifests VDF/ACF | implementado; parser local, fixtures |
| `pt-proton` | validar prefixo Wine e consultar versão/ferramenta selecionada | implementado; fixtures |
| `pt-trainers` | validar `.exe` local e manter referência | implementado |
| `pt-database` | SQLite, jogos, trainers, associações, configurações e histórico | schema inicial implementado |
| `pt-execution` | conferir jogo/prefixo, aguardar processo Steam e chamar Protontricks | implementado |
| `pt-diagnostics` | verificações e mensagens de ambiente | reservado para expansão |
| `pt-cli` | interface `ptrain run` | implementado |
| `src-tauri` | comandos IPC, estado da aplicação e banco por usuário | implementado |
| `frontend` | biblioteca, trainers, histórico e configurações | fluxo inicial implementado |

`src-tauri` fica fora do Cargo workspace para que os testes do núcleo não exijam
WebKit/GTK. Ele mantém seu próprio `Cargo.lock`; CI compila ambos os projetos.

## Fluxos

### Steam e jogos

O detector considera caminho configurado, `$XDG_DATA_HOME/Steam`, raízes Steam
usuais e instalação Flatpak. Aceita raízes que contenham
`steamapps/libraryfolders.vdf`, resolve symlinks e remove duplicatas. As
bibliotecas e jogos são lidos de `libraryfolders.vdf` e `appmanifest_*.acf`.
Manifestos de jogo inválidos são ignorados para que um arquivo ruim não
interrompa a listagem. Caminhos de `libraryfolders.vdf` precisam ser absolutos;
`installdir` precisa ser um único componente, sem separadores ou traversal.
Entradas VDF são limitadas a 8 MiB, 200 mil tokens e 64 níveis de aninhamento.
A implementação cobre o formato VDF chave/valor com objetos, strings,
comentários e escapes usados por esses arquivos; fixtures reais ainda podem
revelar variações adicionais.

### Proton

O prefixo é derivado da biblioteca que contém o manifesto:
`steamapps/compatdata/<AppID>/pfx`. Só é exposto quando contém `system.reg`.
A versão vem de `compatdata/<AppID>/version`, com fallback para
`CompatToolMapping` em `config/config.vdf`.

### Trainers e execução

O banco armazena o caminho canônico, tamanho e associação com o jogo; o arquivo
nunca é copiado. Importação requer caminho absoluto, arquivo regular e extensão
`.exe`. Na importação e antes da execução, o backend valida o marcador `MZ`, o
offset do cabeçalho DOS e a assinatura `PE\0\0`. Também confere o jogo instalado
e o prefixo, compara o tamanho com o registrado e revalida o arquivo após a
espera do jogo. Em seguida aguarda o wrapper `reaper` da Steam e executa
`protontricks-launch --appid <AppID> <caminho>`. Se o executável direto não
estiver no `PATH`, tenta `flatpak run --command=protontricks-launch`. O caminho
Steam configurado na interface é enviado ao fluxo de execução.

A execução é iniciada exclusivamente por ação do usuário e registrada no banco.
A aplicação avisa que trainers são software de terceiros e desaconselha o uso
em jogos online ou com antitrapaça. A espera e o processo rodam em uma tarefa
`spawn_blocking`, sem bloquear o event loop da janela. Cancelamento e progresso
em tempo real permanecem como melhorias futuras. Os demais comandos Tauri que
fazem I/O são despachados como comandos assíncronos. A saída do Protontricks é
descartada no modo GUI; o CLI mantém a saída no terminal e usa mensagens de erro
que não incluem caminhos absolutos.

## Persistência

SQLite fica no diretório de dados da aplicação. O schema inicial guarda jogos,
trainers, associações, configurações e execuções com chaves estrangeiras e
cascatas. `schema_migrations` controla a versão; a criação inicial é transacional
e falhas são revertidas. Bancos de versão futura são rejeitados sem alteração.
No Linux, o diretório de dados usa modo `0700` e o arquivo do banco `0600`.
Próximas mudanças devem ser migrations incrementais, sem alterar bancos
existentes destrutivamente.

## Segurança e limites atuais

- `AppId` aceita apenas inteiro positivo e serializa como número.
- Os argumentos dos processos nunca passam por um shell.
- O arquivo `.exe` não é baixado, copiado ou modificado.
- O banco armazena nome e caminho absoluto do trainer; falhas no histórico usam
  texto genérico para não persistir detalhes de caminho. Não há logger estruturado.
- A UI não oferece execução automática nem integração com antitrapaça.
- Os marcadores PE validam a estrutura inicial, mas não analisam malware nem
  provam compatibilidade com um jogo ou segurança do executável.
- Alterações no trainer com tamanho diferente são detectadas; alterações de
  mesmo tamanho não são, pois hash de conteúdo ainda não foi implementado.
- O monitoramento depende do formato de argumentos do `reaper` do Steam em
  Linux. Flatpak, permissões `/proc` e variantes de cliente precisam de testes
  em máquinas reais.
- A importação usa caminho digitado; seletor nativo de arquivos ainda é uma
  melhoria de interface pendente.
- A detecção de jogo usa fixtures; compatibilidade real com cada distribuição,
  biblioteca, modo Flatpak, variante Proton e trainer ainda não foi validada.
- O monitoramento só espera o jogo iniciar. Não encerra o trainer se o jogo
  fechar, e não acompanha progresso depois que o processo foi criado.

## Verificação

CI roda `fmt`, Clippy com warnings como erro, testes do workspace Rust, typecheck
e build do frontend, além de `cargo check` do shell Tauri em Ubuntu 24.04.
Execução de ponta a ponta requer Steam, Proton e Protontricks instalados e deve
ser validada manualmente em diferentes distribuições antes de uma versão pública.
