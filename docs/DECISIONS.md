# Decisões técnicas

Formato curto de ADR. Decisões marcadas como **provisórias** serão revisitadas
na fase indicada.

## ADR-001 — Workspace Rust com um crate por módulo

**Decisão:** cada módulo do requisito (`steam`, `proton`, `trainers`,
`execution`, `diagnostics`, `database`) é um crate `pt-*`, mais `pt-core` para
tipos compartilhados.
**Motivo:** o compilador impede dependências indevidas (ex.: lógica de negócio
importando `tauri`) e cada módulo é testável isoladamente.
**Custo:** mais arquivos `Cargo.toml`. Aceito.

## ADR-002 — Sem camada "app/services" separada

**Decisão:** a orquestração do fluxo de execução fica em `pt-execution`; os
comandos Tauri apenas adaptam. Não há crate `pt-app` por enquanto.
**Motivo:** evitar uma abstração sem necessidade comprovada.
**Revisitar:** Fase 5, se os comandos Tauri começarem a acumular lógica.

## ADR-003 — Shell Tauri fora do workspace Cargo

**Decisão:** `src-tauri` não é membro do workspace (`exclude`), tem seu próprio
`Cargo.lock` e usa dependências por caminho.
**Motivo:** o Tauri exige bibliotecas do sistema (webkit2gtk, GTK). Mantê-lo fora
permite `cargo test --workspace` em qualquer máquina, inclusive CI mínimo.
**Custo:** dois `Cargo.lock`; CI tem um job dedicado ao shell.

## ADR-004 — Dependências adicionadas quando usadas

**Decisão:** dependências entram junto da funcionalidade. SQLite usa `rusqlite`
com SQLite bundled; VDF é lido por parser pequeno local para limitar a stack e
permitir fixtures determinísticas. Logs estruturados com `tracing` ficam
pendentes de uma necessidade concreta.
**Motivo:** reduzir dependências externas do núcleo e manter builds reproduzíveis.

## ADR-005 — Trainers referenciados por caminho

**Decisão:** o app guarda o caminho do `.exe`, não uma cópia.
**Motivo:** requisito explícito ("salvar o caminho do arquivo") e evita
redistribuir executáveis de terceiros. Hoje são armazenados tamanho e caminho;
hash de conteúdo permanece pendente.

## ADR-006 — `AppId` estrito

**Decisão:** `AppId` é um newtype sobre `u32`, positivo, aceitando só dígitos
ASCII no parsing.
**Motivo:** o valor é argumento de processo externo; rejeitar `+5`, espaços,
`--help` etc. elimina uma classe de injeção de argumentos.

## ADR-007 — Idioma

**Decisão:** documentação e textos da interface em português (pt-BR); código,
identificadores e comentários em inglês.
**Revisitar:** internacionalização da interface antes de uma versão pública.

## ADR-008 — Licença

**Decisão:** MIT (arquivo `LICENSE`, titular "ProtonTrainer contributors").
**Observação:** premissa minha; se preferir GPL-3.0 ou Apache-2.0, a troca é
simples agora, antes de haver contribuições externas.

## ADR-009 — Interface

**Decisão:** React 19 + TypeScript + Tailwind CSS 4 (plugin Vite), tema escuro
único, tokens de design em `frontend/src/index.css`, acento âmbar.
Sem biblioteca de roteamento ou de estado por enquanto (estado local simples).
**Revisitar:** Fase 5.
