# ProtonTrainer

Gerenciador desktop open source para usar trainers de jogos Windows no Linux,
através do Proton, sem precisar descobrir AppIDs, prefixos ou comandos no
terminal.

> **Status: Fase 0 (estrutura inicial).** Ainda não é possível listar jogos nem
> executar trainers. O que existe hoje é o esqueleto do projeto, a arquitetura
> documentada e a base de testes/CI. Veja o [roadmap](#roadmap).

## Aviso importante

Trainers são executáveis de terceiros e podem representar riscos de segurança.
O ProtonTrainer nunca baixa nem executa trainers automaticamente, não distribui
trainers e não deve ser usado em jogos online ou com sistemas antitrapaça.

## Stack

Rust · Tauri · React + TypeScript · Tailwind CSS · SQLite

## Estrutura

| Caminho | Conteúdo |
|---|---|
| `crates/pt-core` | tipos de domínio compartilhados (`AppId`) |
| `crates/pt-steam` | Steam: instalações, bibliotecas, jogos |
| `crates/pt-proton` | prefixos e versões do Proton |
| `crates/pt-trainers` | importação e associação de trainers |
| `crates/pt-execution` | pipeline de execução |
| `crates/pt-diagnostics` | verificações de ambiente e logs |
| `crates/pt-database` | SQLite e migrations |
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

No Arch/CachyOS: `webkit2gtk-4.1`, `base-devel`, `xdotool`, `openssl`,
`libayatana-appindicator`, `librsvg`. No Fedora: `webkit2gtk4.1-devel`,
`openssl-devel`, `libxdo-devel`, `libappindicator-gtk3-devel`, `librsvg2-devel`.

Para rodar em modo de desenvolvimento (precisa do CLI do Tauri, p.ex.
`cargo install tauri-cli --version "^2"`):

```bash
cd src-tauri
cargo tauri dev
```

## Roadmap

- [x] **Fase 0** — análise, arquitetura e estrutura inicial
- [ ] **Fase 1** — detecção da Steam, bibliotecas e jogos
- [ ] **Fase 2** — prefixos e versões do Proton
- [ ] **Fase 3** — importação e persistência de trainers
- [ ] **Fase 4** — execução via Protontricks, monitoramento e logs
- [ ] **Fase 5** — interface completa
- [ ] **Fase 6** — validação, instalação e uso documentados

## Licença

[MIT](LICENSE)
