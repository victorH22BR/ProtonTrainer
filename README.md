# ProtonTrainer

**Your games. Your trainers. Your freedom.**

O ProtonTrainer é um gerenciador desktop open source desenvolvido para facilitar o uso de trainers de jogos Windows no Linux através do Proton.

A proposta é simples: encontrar seu jogo, importar ou criar um trainer e executá-lo sem precisar descobrir AppIDs, localizar prefixos manualmente ou lidar com uma sequência de comandos no terminal.

O projeto nasceu de uma necessidade real: tornar a utilização de trainers no Linux algo simples, acessível e organizado.

**Status: Em desenvolvimento — núcleo Rust e CLI experimental.**

O projeto já possui uma arquitetura modular, documentação, testes automatizados e uma primeira implementação do pipeline de execução através da CLI `ptrain`. A integração real com Steam e Proton e a interface desktop ainda estão em desenvolvimento.

---

## Nossa filosofia

O ProtonTrainer está sendo construído com uma ideia fundamental:

**Todos os usuários devem ter acesso às mesmas funcionalidades.**

Não queremos criar barreiras artificiais para limitar o uso do aplicativo. O usuário não deve pagar para executar seus próprios trainers, criar novos projetos ou utilizar os recursos essenciais do programa.

O ProtonTrainer será:

- **Gratuito:** todas as funcionalidades locais estarão disponíveis para todos.
- **Open source:** qualquer pessoa poderá estudar, contribuir e melhorar o projeto.
- **Comunitário:** conhecimento e compatibilidade poderão ser compartilhados.
- **Linux-first:** desenvolvido pensando nas necessidades de quem joga no Linux.
- **Leve e simples:** uma ferramenta objetiva, sem se transformar em um launcher gigantesco.

## Como queremos que funcione

A experiência principal deve ser direta:

1. O ProtonTrainer identifica os jogos instalados.
2. O usuário seleciona um jogo.
3. Escolhe um trainer existente, importa um arquivo ou cria o próprio.
4. O aplicativo prepara o ambiente necessário.
5. O usuário ativa o trainer.

Sem precisar conhecer o funcionamento interno do Proton, procurar prefixos ou decorar comandos.

### Uma comunidade construída pelos usuários

Nem todo jogo possui um trainer disponível. Queremos que isso deixe de ser um obstáculo.

No futuro, o ProtonTrainer pretende oferecer ferramentas para facilitar a criação de trainers e permitir que os usuários compartilhem suas descobertas com a comunidade.

A ideia é construir um espaço onde seja possível:

- Compartilhar trainers criados pelos próprios usuários.
- Compartilhar referências e fontes de trainers.
- Consultar informações de compatibilidade.
- Publicar instruções e descobertas.
- Ajudar outros jogadores a encontrar soluções.
- Contribuir com informações sobre jogos que ainda não possuem trainers conhecidos.

O catálogo comunitário poderá trabalhar com fontes e distribuidores externos, preservando a origem dos arquivos e respeitando as permissões de distribuição de cada conteúdo.

O objetivo não é centralizar todos os arquivos em um único servidor, mas facilitar o acesso às informações e às fontes disponíveis.

## Gratuito de verdade

Todas as funcionalidades principais do ProtonTrainer serão gratuitas.

Não haverá cobrança por tempo de utilização, limite artificial de execuções ou funcionalidades essenciais bloqueadas atrás de uma assinatura.

O usuário poderá manter sua coleção inteira no próprio computador, sem precisar criar uma conta ou depender de conexão com a internet para utilizar o aplicativo localmente.

### Armazenamento em nuvem (opcional)

A nuvem será um serviço adicional para quem quiser mais praticidade.

Imagine que você passou meses criando, encontrando e organizando trainers. Você pode guardar tudo no seu computador, mas talvez queira uma cópia na nuvem para não perder sua coleção ao trocar de máquina ou formatar o sistema.

É para isso que a nuvem existirá.

Entre os recursos planejados:

- Armazenamento de trainers criados ou importados pelo usuário.
- Backup da coleção pessoal.
- Recuperação dos arquivos em outro computador.
- Sincronização entre dispositivos.

O armazenamento em nuvem poderá ser pago, mas será completamente opcional.

**Quem não quiser pagar poderá continuar usando todas as funcionalidades do ProtonTrainer e guardar seus arquivos localmente.**

A nuvem não será uma condição para utilizar o aplicativo.

## Segurança

Trainers são executáveis de terceiros e podem representar riscos de segurança.

Por isso, o ProtonTrainer deverá priorizar:

- Identificação da origem dos arquivos.
- Confirmação do usuário antes de downloads e execuções.
- Verificação de integridade dos arquivos quando possível.
- Avisos sobre executáveis de terceiros.
- Separação entre fontes comunitárias e arquivos confiáveis.
- Nenhuma execução silenciosa de arquivos desconhecidos.

O projeto é destinado a jogos offline e experiências single-player. Não deve ser utilizado para obter vantagens em jogos online ou contornar sistemas antitrapaça.

O usuário deve sempre verificar a procedência dos trainers que importar ou utilizar.

---

## Stack tecnológica

| Tecnologia | Responsabilidade |
|---|---|
| Rust | Núcleo e lógica de execução |
| Tauri 2 | Aplicativo desktop |
| React | Interface |
| TypeScript | Tipagem do frontend |
| Tailwind CSS | Estilização |
| SQLite | Persistência local |

## Estrutura do projeto

| Caminho | Responsabilidade |
|---|---|
| `crates/pt-core` | Tipos de domínio compartilhados |
| `crates/pt-steam` | Instalações, bibliotecas e jogos Steam |
| `crates/pt-proton` | Prefixos e versões do Proton |
| `crates/pt-trainers` | Importação e associação de trainers |
| `crates/pt-execution` | Pipeline de execução |
| `crates/pt-diagnostics` | Diagnósticos e logs |
| `crates/pt-database` | SQLite e migrations |
| `crates/pt-cli` | Interface de linha de comando |
| `src-tauri` | Aplicativo desktop Tauri |
| `frontend` | Interface React |
| `docs` | Arquitetura e decisões técnicas |

## CLI experimental

O projeto já conta com uma primeira versão da CLI `ptrain`.

Com ela, é possível utilizar o comando:

```bash
ptrain run <APP_ID> <TRAINER>
```

Exemplo:

```bash
ptrain run 2322010 "/home/user/Downloads/Trainer.exe"
```

Também é possível configurar o tempo de espera pelo início do jogo:

```bash
ptrain run 2322010 "/home/user/Downloads/Trainer.exe" --wait 180
```

A CLI e o pipeline inicial estão implementados, mas a execução real com diferentes jogos, versões do Proton e ambientes Steam ainda precisa ser validada.

---

## Desenvolvimento

### Requisitos

- Rust estável (≥ 1.80)
- Node.js 22
- pnpm 10

### Testes do núcleo Rust

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

### Frontend

```bash
cd frontend
pnpm install
pnpm typecheck
pnpm build
```

### Aplicativo Tauri

É necessário instalar as dependências de sistema do Tauri 2.

**Ubuntu/Debian:**

```bash
sudo apt install libwebkit2gtk-4.1-dev build-essential libxdo-dev libssl-dev \
  libayatana-appindicator3-dev librsvg2-dev
```

**Arch Linux / CachyOS:**

```bash
sudo pacman -S webkit2gtk-4.1 base-devel xdotool openssl \
  libayatana-appindicator librsvg
```

**Fedora:**

```bash
sudo dnf install webkit2gtk4.1-devel openssl-devel libxdo-devel \
  libappindicator-gtk3-devel librsvg2-devel
```

Instale também o CLI do Tauri:

```bash
cargo install tauri-cli --version "^2"
```

Para executar em modo de desenvolvimento:

```bash
cd src-tauri
cargo tauri dev
```

---

## Roadmap

O desenvolvimento será dividido em etapas para manter o projeto modular e evitar complexidade desnecessária.

- [x] **Fase 0** — análise, arquitetura e estrutura inicial.
- [x] **CLI experimental** — primeira interface de linha de comando e pipeline inicial de execução.
- [ ] **Fase 1** — detecção da Steam, bibliotecas e jogos.
- [ ] **Fase 2** — identificação de prefixos e versões do Proton.
- [ ] **Fase 3** — importação, organização e persistência local de trainers.
- [ ] **Fase 4** — validação da execução via Proton, monitoramento e logs.
- [ ] **Fase 5** — interface desktop simples e funcional.
- [ ] **Fase 6** — documentação, instalação e validação em diferentes ambientes.
- [ ] **Fase 7** — sistema de fontes e integrações comunitárias.
- [ ] **Fase 8** — ferramentas para criação e compartilhamento de trainers.
- [ ] **Fase 9** — catálogo comunitário e informações de compatibilidade.
- [ ] **Fase 10** — armazenamento em nuvem e sincronização opcionais.

As etapas futuras são objetivos de desenvolvimento, não funcionalidades já disponíveis.

---

## Contribuindo

O ProtonTrainer é um projeto open source e contribuições são bem-vindas.

Você pode ajudar com:

- Desenvolvimento em Rust.
- Interface e experiência do usuário.
- Testes em diferentes distribuições Linux.
- Compatibilidade com Steam e Proton.
- Documentação.
- Relatos de bugs e sugestões.
- Informações de compatibilidade para a comunidade.

Consulte a documentação do projeto antes de propor mudanças arquiteturais.

## Licença

Este projeto é distribuído sob a licença [MIT](LICENSE).

---

**ProtonTrainer — Free to use. Open to everyone.**

Suas ferramentas, sua coleção e sua liberdade. Sem barreiras artificiais.
