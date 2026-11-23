# Arquitetura do ProtonTrainer

> Status: **Fase 0** — esta é a arquitetura *proposta*. Itens marcados com
> **(a validar)** dependem de fixtures reais da Steam/Proton e serão confirmados
> nas fases indicadas. Nada aqui descreve funcionalidade já implementada.

## 1. Princípios

1. **Lógica de negócio fora da interface.** Tudo que pode ser testado sem janela
   vive em crates `pt-*`. O shell Tauri apenas adapta comandos.
2. **Detecção real, nunca palpite.** Prefixos, AppIDs e versões vêm de arquivos
   da Steam/Proton validados; se não for possível confirmar, o erro é mostrado.
3. **Sem shell.** Processos externos são criados com `Command` e argumentos
   separados. Entradas são validadas antes de chegar lá (ex.: `AppId` só aceita
   dígitos ASCII, então `--help` nunca vira argumento).
4. **Ação explícita.** Nada é baixado ou executado sem clique do usuário.
5. **Sem abstrações antecipadas.** Pontos de extensão (Lutris, Heroic, Bottles)
   são documentados, mas só viram traits quando existir a segunda implementação.

## 2. Estrutura do repositório

```
protontrainer/
├── Cargo.toml              workspace Rust (apenas crates/*)
├── crates/
│   ├── pt-core/            tipos de domínio compartilhados (AppId)
│   ├── pt-steam/           instalações, bibliotecas e jogos da Steam
│   ├── pt-proton/          prefixos e versões do Proton
│   ├── pt-trainers/        importação, validação e associação de trainers
│   ├── pt-execution/       pipeline de execução e monitoramento
│   ├── pt-diagnostics/     verificações de ambiente, erros e logs
│   └── pt-database/        SQLite, migrations e repositórios
├── src-tauri/              shell Tauri (comandos finos; fora do workspace)
├── frontend/               React + TypeScript + Tailwind (Vite)
├── docs/
└── .github/workflows/ci.yml
```

## 3. Dependências entre crates

```
pt-core  ◄── pt-steam ◄── pt-proton ◄──┐
   ▲                                   │
   ├── pt-diagnostics ◄────────────────┤
   ├── pt-database ◄── pt-trainers     ├── pt-execution
   └─────────────────────────────────────┘

src-tauri ──► todos os crates pt-*      frontend ──► src-tauri (IPC)
```

Regras: crates `pt-*` nunca dependem de `tauri`; `pt-core` não depende de nenhum
outro crate do workspace; não há dependências circulares.

## 4. Módulos

### `pt-steam` — Fase 1

- **Raízes da Steam:** nenhum caminho fixo. Estratégia: (1) caminho configurado
  pelo usuário; (2) candidatos conhecidos (`$XDG_DATA_HOME/Steam`,
  `~/.steam/steam`, `~/.steam/root`, Flatpak em
  `~/.var/app/com.valvesoftware.Steam/.local/share/Steam`); (3) cada candidato
  só é aceito se for validado (existe `steamapps/` e `libraryfolders.vdf`).
  Links simbólicos são resolvidos e duplicatas removidas.
- **Bibliotecas:** leitura de `steamapps/libraryfolders.vdf` **(a validar)**.
- **Jogos:** leitura de `steamapps/appmanifest_<AppID>.acf` (`appid`, `name`,
  `installdir`) **(a validar)**. Manifestos corrompidos geram diagnóstico, não
  derrubam a listagem inteira.
- **Parser VDF:** decisão (crate existente × parser próprio) fica para a Fase 1,
  após testar com fixtures reais.

### `pt-proton` — Fase 2

- **Prefixo:** `<biblioteca>/steamapps/compatdata/<AppID>/pfx`, onde
  `<biblioteca>` é a biblioteca que contém o manifesto do jogo. Um prefixo só é
  considerado válido se o diretório existir e tiver marcadores de prefixo Wine
  (ex.: `system.reg`) **(a validar)**. Se não existir (jogo nunca executado),
  o resultado é "prefixo ausente", nunca um caminho inventado.
- **Versão do Proton:** `compatdata/<AppID>/version` e o mapeamento em
  `config/config.vdf` (`CompatToolMapping`) **(a validar)**.

### `pt-trainers` — Fase 3

- Trainers são **referenciados por caminho** (não copiados, não baixados).
- Validação: caminho absoluto, arquivo regular, extensão `.exe`, legível;
  detecção de arquivo movido/removido depois da importação.
- Associação N:N entre jogos e trainers, com configurações por jogo.

### `pt-execution` — Fase 4

Pipeline (cada etapa devolve um erro tipado e acionável):

1. Jogo existe e está instalado → 2. AppID → 3. prefixo válido →
4. Protontricks disponível → 5. jogo em execução? → 6. executar → 7. registrar
resultado.

- **Comando preferencial:** `protontricks-launch --appid <AppID> <trainer.exe>`,
  montado como `Command::new(..).arg(..)`, sem shell.
- **Protontricks via Flatpak:** a forma de invocação será tratada na Fase 4
  **(a validar)**.
- **Jogo em execução:** inspeção de `/proc` (processos `reaper` com
  `AppId=<AppID>` na linha de comando) **(a validar)**.
- Processos externos ficam atrás de uma interface pequena para permitir mocks
  nos testes (sem exigir Steam/Proton instalados).

### `pt-diagnostics`

Verificações de ambiente (Protontricks, Wine), mensagens de erro compreensíveis
e configuração de logs estruturados (`tracing`, decidido quando houver o
primeiro uso).

### `pt-database` — Fase 3

SQLite com migrations versionadas e `PRAGMA foreign_keys = ON`. Esquema proposto:

| Tabela | Campos principais | Integridade |
|---|---|---|
| `games` | `id`, `source`, `app_id`, `name`, `install_dir`, `library_path`, `last_seen_at` | `UNIQUE(source, app_id)` |
| `trainers` | `id`, `name`, `exe_path`, `file_size`, `sha256`, `created_at` | — |
| `game_trainers` | `game_id`, `trainer_id`, `launcher`, `enabled` | PK composta; FKs com `ON DELETE CASCADE` |
| `settings` | `key`, `value` | PK `key` |
| `executions` | `id`, `game_id`, `trainer_id`, `started_at`, `finished_at`, `status`, `exit_code`, `launcher`, `proton_version`, `argv`, `output_tail` | FK `game_id` cascade; `trainer_id` `SET NULL` |
| `compatibility` | `id`, `game_id`, `trainer_id`, `proton_version`, `outcome`, `notes`, `last_tested_at` | `UNIQUE(game_id, trainer_id, proton_version)` |

O hash do trainer permite avisar se o arquivo mudou desde a importação.
A coluna `source` já existe em `games` para a expansão futura (P2), mas só o
valor `steam` é aceito no MVP.

## 5. Shell Tauri e frontend

- Comandos Tauri são finos: validam entrada, chamam um crate `pt-*`, convertem o
  resultado em um tipo serializável.
- O frontend acessa o backend **somente** por `frontend/src/lib/backend.ts`, que
  concentra nomes de comandos e tipos.
- Permissões do Tauri (`capabilities/default.json`) começam mínimas
  (`core:default`) e crescem apenas quando uma fase exigir.
- Telas: Dashboard, Biblioteca, Detalhes do jogo, Trainers, Histórico,
  Configurações. Na Fase 0 existem apenas como navegação com estado vazio.

## 6. Segurança

- Sem shell, sem interpolação; argumentos separados e validados.
- Nenhum download ou execução automática de trainers; nenhuma distribuição de
  trainers de terceiros.
- Sem privilégios elevados; sem armazenamento de credenciais.
- Aviso visível ao usuário: trainers são executáveis de terceiros e podem
  representar risco.
- Não interferir em jogos online ou mecanismos antitrapaça: a interface avisará
  e a documentação desaconselha o uso nesses cenários.

## 7. Estratégia de testes

- Testes unitários em cada crate; fixtures de árvores Steam/Proton falsas em
  diretórios temporários, sem depender de instalação real.
- Processos externos atrás de uma interface mockável (Fase 4).
- CI: `fmt`, `clippy -D warnings`, `cargo test --workspace`, typecheck e build do
  frontend, `cargo check` do shell Tauri.
- Uma funcionalidade só é considerada pronta quando seus testes passam, não
  apenas quando compila.

## 8. Pontos de extensão (P2 — não implementados)

Lutris, Heroic, Bottles, outras fontes de trainers, catálogo e verificação
comunitária. A preparação nesta fase é só: coluna `games.source`, módulos
independentes e a regra de extrair traits apenas quando houver uma segunda
implementação concreta.
