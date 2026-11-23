# ProtonTrainer

Gerenciador desktop open source para usar trainers de jogos Windows no Linux, através do Proton, sem precisar descobrir AppIDs, prefixos ou comandos no terminal.

> **Status: MVP em implementação.** Descoberta Steam/Proton, importação local, persistência e execução explícita já estão conectadas ao shell desktop. A compatibilidade ainda precisa ser validada com instalações reais variadas.

## Aviso importante

Trainers são executáveis de terceiros e podem representar riscos de segurança. O ProtonTrainer nunca baixa nem executa trainers automaticamente, não distribui trainers e não deve ser usado em jogos online ou com sistemas antitrapaça.

## Dependências de execução

O uso do desktop exige Steam para Linux, um jogo instalado e iniciado ao menos uma vez para que exista um prefixo Proton, além de Protontricks. O aplicativo procura `protontricks-launch` no `PATH` e aceita o launcher do Flatpak como fallback. Nenhum desses cenários substitui a validação manual em cada distribuição suportada.

## Stack

Rust · Tauri · React + TypeScript · Tailwind CSS · SQLite

## Estrutura

| Caminho | Conteúdo |
|---|---|
| `crates/pt-core` | tipos de domínio compartilhados (`AppId`) |
| `crates/pt-steam` | Steam: instalações, bibliotecas, jogos |
| `crates/pt-proton` | prefixos e versões do Proton |
| `crates/pt-trainers` | validação e associação por caminho de trainers |
| `crates/pt-execution` | pipeline de execução com Protontricks |
| `crates/pt-diagnostics` | verificações de ambiente e logs (em expansão) |
| `crates/pt-database` | SQLite, schema inicial e histórico |
| `crates/pt-cli` | comando `ptrain run` |
| `src-tauri` | shell Tauri |
| `frontend` | interface React |
| `docs` | [arquitetura](docs/ARCHITECTURE.md) e [decisões](docs/DECISIONS.md) |

## Desenvolvimento

Requisitos: Rust estável (≥ 1.80), Node 22, pnpm 10.

```bash
# Núcleo Rust (não exige bibliotecas gráficas)
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace

# CLI: executa um trainer local em um jogo Steam já iniciado
cargo run -p pt-cli -- run <APPID> /caminho/para/trainer.exe

# Frontend
cd frontend
pnpm install
pnpm typecheck
pnpm build
```

### Shell Tauri

Exige as dependências de sistema do Tauri 2. No Ubuntu/Debian:

```bash
sudo apt install libwebkit2gtk-4.1-dev build-essential libxdo-dev libssl-dev \
  libayatana-appindicator3-dev librsvg2-dev
```

No Arch/CachyOS: `webkit2gtk-4.1`, `base-devel`, `xdotool`, `openssl`, `libayatana-appindicator`, `librsvg`. No Fedora: `webkit2gtk4.1-devel`, `openssl-devel`, `libxdo-devel`, `libappindicator-gtk3-devel`, `librsvg2-devel`.

Para rodar em modo de desenvolvimento (precisa do CLI do Tauri, p.ex. `cargo install tauri-cli --version "^2"`):

```bash
cd src-tauri
cargo tauri dev
```

## Roadmap

- [x] **Fase 0** — análise, arquitetura e estrutura inicial
- [x] **Fase 1** — detecção da Steam, bibliotecas e jogos (VDF/ACF)
- [x] **Fase 2** — prefixos e versões do Proton
- [x] **Fase 3** — importação referenciada e persistência SQLite
- [x] **Fase 4** — execução explícita via Protontricks e histórico
- [ ] **Fase 5** — polimento visual, diálogos nativos e acessibilidade
- [ ] **Fase 6** — validação em distribuições e instalações Steam reais

As fases 1–4 têm cobertura automatizada e simulada. Ainda não há teste ponta a ponta que execute um trainer real; compatibilidade entre trainer, jogo e versão do Proton não é inferida pelo aplicativo.

## Licença

[MIT](LICENSE)
