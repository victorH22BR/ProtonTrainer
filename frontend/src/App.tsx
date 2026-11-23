import { useEffect, useState } from "react";
import { getAppInfo, isTauri, type AppInfo } from "./lib/backend";

interface Section {
  id: string;
  label: string;
  description: string;
  phase: number;
}

// The six screens required by the MVP. Each one is implemented in the phase
// listed here; until then it only shows an empty state (no mock data).
const SECTIONS: readonly Section[] = [
  { id: "dashboard", label: "Dashboard", description: "Visão geral dos jogos, trainers e execuções recentes.", phase: 5 },
  { id: "library", label: "Biblioteca", description: "Jogos Steam detectados neste computador.", phase: 5 },
  { id: "game", label: "Detalhes do jogo", description: "AppID, prefixo Proton e trainers associados.", phase: 5 },
  { id: "trainers", label: "Trainers", description: "Importar, substituir e remover trainers.", phase: 5 },
  { id: "history", label: "Histórico", description: "Resultados e erros das execuções anteriores.", phase: 5 },
  { id: "settings", label: "Configurações", description: "Caminhos da Steam, método de execução e preferências.", phase: 5 },
];

type BackendState =
  | { status: "loading" }
  | { status: "ready"; info: AppInfo }
  | { status: "unavailable"; reason: string };

function useBackend(): BackendState {
  const [state, setState] = useState<BackendState>({ status: "loading" });

  useEffect(() => {
    if (!isTauri()) {
      setState({ status: "unavailable", reason: "executando fora do Tauri" });
      return;
    }
    let cancelled = false;
    getAppInfo()
      .then((info) => {
        if (!cancelled) setState({ status: "ready", info });
      })
      .catch((error: unknown) => {
        if (!cancelled) setState({ status: "unavailable", reason: String(error) });
      });
    return () => {
      cancelled = true;
    };
  }, []);

  return state;
}

export default function App() {
  const [activeId, setActiveId] = useState<string>(SECTIONS[0]?.id ?? "dashboard");
  const backend = useBackend();
  const active = SECTIONS.find((section) => section.id === activeId) ?? SECTIONS[0];

  return (
    <div className="flex h-full">
      <aside className="flex w-60 shrink-0 flex-col border-r border-line bg-panel">
        <div className="px-5 py-5">
          <h1 className="text-lg font-semibold tracking-tight">
            Proton<span className="text-accent">Trainer</span>
          </h1>
        </div>
        <nav className="flex-1 space-y-1 px-3" aria-label="Navegação principal">
          {SECTIONS.map((section) => (
            <button
              key={section.id}
              type="button"
              onClick={() => setActiveId(section.id)}
              aria-current={section.id === activeId ? "page" : undefined}
              className={
                "w-full rounded-md px-3 py-2 text-left text-sm transition-colors " +
                (section.id === activeId
                  ? "bg-panel-raised text-accent"
                  : "text-muted hover:bg-panel-raised hover:text-fg")
              }
            >
              {section.label}
            </button>
          ))}
        </nav>
        <footer className="border-t border-line px-5 py-3 text-xs text-muted">
          {backend.status === "ready" && <span>v{backend.info.version}</span>}
          {backend.status === "loading" && <span>conectando ao backend…</span>}
          {backend.status === "unavailable" && (
            <span title={backend.reason}>backend indisponível</span>
          )}
        </footer>
      </aside>

      <main className="flex-1 overflow-auto p-8">
        {active && (
          <section>
            <h2 className="text-2xl font-semibold">{active.label}</h2>
            <p className="mt-1 text-muted">{active.description}</p>
            <div className="mt-8 rounded-lg border border-dashed border-line p-10 text-center text-sm text-muted">
              Ainda não implementado — previsto para a Fase {active.phase}.
            </div>
          </section>
        )}
      </main>
    </div>
  );
}
