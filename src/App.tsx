import { useEffect, useRef, useState, type PointerEvent as ReactPointerEvent } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { QRCodeSVG } from "qrcode.react";
import type { AudioState, DashboardState, DeckButton, DeckConfig, GridSize, ServiceSettings, ThemeMode } from "./types";

const ACCENTS = ["#e9592f", "#286c64", "#d39a22", "#675a9e", "#a63d53", "#3f658c"];
type Screen = "deck" | "settings";

function Icon({ name }: { name: "volume" | "mute" | "microphone" | "phone" | "plus" | "save" | "refresh" | "folder" | "up" | "down" | "trash" | "image" | "globe" | "sun" | "moon" | "link" | "settings" | "back" }) {
  const paths = {
    volume: <><path d="M11 5 6 9H2v6h4l5 4V5Z"/><path d="M15.5 8.5a5 5 0 0 1 0 7"/><path d="M18 6a8.5 8.5 0 0 1 0 12"/></>,
    mute: <><path d="M11 5 6 9H2v6h4l5 4V5Z"/><path d="m16 10 6 6m0-6-6 6"/></>,
    microphone: <><rect x="9" y="3" width="6" height="12" rx="3"/><path d="M5 11a7 7 0 0 0 14 0M12 18v3M8 21h8"/></>,
    phone: <><rect x="6" y="2" width="12" height="20" rx="2"/><path d="M10 18h4"/></>,
    plus: <path d="M12 5v14M5 12h14"/>,
    save: <><path d="M5 3h12l3 3v15H4V3h1Z"/><path d="M8 3v6h8V3M8 21v-8h8v8"/></>,
    refresh: <><path d="M20 7v5h-5"/><path d="M18.5 15a7.5 7.5 0 1 1-.8-7.8L20 9"/></>,
    folder: <><path d="M3 6h7l2 2h9v11H3V6Z"/><path d="M3 10h18"/></>,
    up: <path d="m6 15 6-6 6 6"/>,
    down: <path d="m6 9 6 6 6-6"/>,
    trash: <><path d="M4 7h16M9 3h6l1 4H8l1-4ZM7 7l1 14h8l1-14"/><path d="M10 11v6m4-6v6"/></>,
    image: <><rect x="3" y="4" width="18" height="16" rx="1"/><circle cx="9" cy="10" r="2"/><path d="m4 17 5-4 3 3 3-2 5 4"/></>,
    globe: <><circle cx="12" cy="12" r="9"/><path d="M3 12h18M12 3a14 14 0 0 1 0 18M12 3a14 14 0 0 0 0 18"/></>,
    sun: <><circle cx="12" cy="12" r="4"/><path d="M12 2v2M12 20v2M4.9 4.9l1.4 1.4M17.7 17.7l1.4 1.4M2 12h2M20 12h2M4.9 19.1l1.4-1.4M17.7 6.3l1.4-1.4"/></>,
    moon: <path d="M21 14.5A8.5 8.5 0 1 1 9.5 3 7 7 0 0 0 21 14.5Z"/>,
    link: <><path d="M10 13a5 5 0 0 0 7.07 0l2.12-2.12a5 5 0 0 0-7.07-7.07L11 5"/><path d="M14 11a5 5 0 0 0-7.07 0L4.8 13.12a5 5 0 0 0 7.07 7.07L13 19"/></>,
    settings: <><circle cx="12" cy="12" r="3"/><path d="M12 1v2M12 21v2M4.2 4.2l1.4 1.4M18.4 18.4l1.4 1.4M1 12h2M21 12h2M4.2 19.8l1.4-1.4M18.4 5.6l1.4-1.4"/></>,
    back: <path d="M15 6 9 12l6 6"/>,
  };
  return <svg aria-hidden="true" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round">{paths[name]}</svg>;
}

function VolumeDial({ audio, onChange }: { audio: AudioState | null; onChange: (value: number) => void }) {
  const value = audio ? Math.round(audio.volume * 100) : 0;
  const circumference = 2 * Math.PI * 76;
  const dash = circumference * value / 100;

  function setFromPointer(event: ReactPointerEvent<HTMLDivElement>) {
    const rect = event.currentTarget.getBoundingClientRect();
    const x = event.clientX - rect.left - rect.width / 2;
    const y = event.clientY - rect.top - rect.height / 2;
    let angle = Math.atan2(y, x) * 180 / Math.PI + 90;
    if (angle < 0) angle += 360;
    onChange(Math.max(0, Math.min(100, Math.round(angle / 360 * 100))));
  }

  return (
    <div
      className={`volume-dial ${audio?.muted ? "is-muted" : ""}`}
      role="slider"
      tabIndex={0}
      aria-label="Volume de saída"
      aria-valuemin={0}
      aria-valuemax={100}
      aria-valuenow={value}
      onPointerDown={(event) => {
        event.currentTarget.setPointerCapture(event.pointerId);
        setFromPointer(event);
      }}
      onPointerMove={(event) => event.currentTarget.hasPointerCapture(event.pointerId) && setFromPointer(event)}
      onKeyDown={(event) => {
        if (event.key === "ArrowUp" || event.key === "ArrowRight") onChange(Math.min(100, value + 2));
        if (event.key === "ArrowDown" || event.key === "ArrowLeft") onChange(Math.max(0, value - 2));
      }}
    >
      <svg viewBox="0 0 180 180">
        <circle className="dial-track" cx="90" cy="90" r="76" />
        <circle className="dial-progress" cx="90" cy="90" r="76" strokeDasharray={`${dash} ${circumference - dash}`} />
      </svg>
      <div className="dial-readout">
        <Icon name={audio?.muted ? "mute" : "volume"} />
        <strong>{audio ? value : "--"}</strong>
        <span>{audio?.muted ? "MUDO" : "VOLUME"}</span>
      </div>
    </div>
  );
}

function DeckTile({ button, unread, onClick }: { button: DeckButton; unread: number | null | undefined; onClick: () => void }) {
  const initials = button.label.trim().split(/\s+/).slice(0, 2).map((word) => word[0]).join("").toUpperCase() || "?";
  const classes = ["deck-tile", button.showLabel ? "" : "icon-only", button.transparentBackground ? "transparent-bg" : ""].filter(Boolean).join(" ");
  return (
    <button type="button" className={classes} style={{ "--button-color": button.color } as React.CSSProperties} title={button.label} aria-label={button.label} onClick={onClick}>
      <span className="deck-glyph">{button.icon ? <img src={button.icon} alt="" /> : initials}</span>
      {button.showLabel && <span className="deck-label">{button.label}</span>}
      {typeof unread === "number" && unread > 0 && <b className="unread-badge">{unread > 99 ? "99+" : unread}</b>}
    </button>
  );
}

function ActionPreview({ button }: { button: DeckButton }) {
  const initials = button.label.trim().split(/\s+/).slice(0, 2).map((word) => word[0]).join("").toUpperCase() || "?";
  const classes = ["action-preview", button.showLabel ? "" : "icon-only", button.transparentBackground ? "transparent-bg" : ""].filter(Boolean).join(" ");
  return (
    <div className={classes} style={{ "--button-color": button.color } as React.CSSProperties} title={button.label || "Sem nome"}>
      <span className="action-glyph">{button.icon ? <img src={button.icon} alt="" /> : initials}</span>
      {button.showLabel && <span className="action-name">{button.label || "Sem nome"}</span>}
    </div>
  );
}

function ButtonEditor({ button, index, total, fetchingIcon, onChange, onMove, onRemove, onPick, onPickIcon, onClearIcon, onFetchIcon }: {
  button: DeckButton;
  index: number;
  total: number;
  fetchingIcon: boolean;
  onChange: (button: DeckButton) => void;
  onMove: (direction: -1 | 1) => void;
  onRemove: () => void;
  onPick: () => void;
  onPickIcon: () => void;
  onClearIcon: () => void;
  onFetchIcon: () => void;
}) {
  return (
    <article className="button-editor">
      <ActionPreview button={button} />
      <div className="editor-fields">
        <label>Nome<input value={button.label} maxLength={32} onChange={(event) => onChange({ ...button, label: event.target.value })} /></label>
        <label>Tipo<select value={button.kind} onChange={(event) => onChange({ ...button, kind: event.target.value as DeckButton["kind"] })}><option value="application">Aplicativo ou arquivo</option><option value="url">Site, endereço ou protocolo</option></select></label>
        <label className="target-field">Destino<span className="input-with-action"><input value={button.target} placeholder={button.kind === "url" ? "https://exemplo.com ou protocolo:" : "C:\\...\\aplicativo.exe"} onChange={(event) => onChange({ ...button, target: event.target.value })} />{button.kind === "application" && <button type="button" className="field-action" onClick={onPick} aria-label="Procurar aplicativo"><Icon name="folder" /></button>}{button.kind === "url" && <button type="button" className="field-action" onClick={onFetchIcon} disabled={fetchingIcon || !button.target.trim()} aria-label="Importar ícone do site"><Icon name="globe" /></button>}</span></label>
        <label>Contador<select value={button.unreadProvider ?? ""} onChange={(event) => onChange({ ...button, unreadProvider: (event.target.value || null) as DeckButton["unreadProvider"] })}><option value="">Nenhum</option><option value="teams">Microsoft Teams</option><option value="whatsapp">WhatsApp</option></select></label>
        <label className="toggle-field"><span>Mostrar nome</span><button type="button" className={`toggle ${button.showLabel ? "on" : ""}`} role="switch" aria-checked={button.showLabel} onClick={() => onChange({ ...button, showLabel: !button.showLabel })}><i /></button></label>
        <label className="toggle-field"><span>Fundo transparente</span><button type="button" className={`toggle ${button.transparentBackground ? "on" : ""}`} role="switch" aria-checked={button.transparentBackground} onClick={() => onChange({ ...button, transparentBackground: !button.transparentBackground })}><i /></button></label>
        <div className="icon-field"><span>Ícone</span><div><button type="button" onClick={onPickIcon}><Icon name="image" />{button.icon ? "Trocar" : "Escolher"}</button>{button.icon && <button type="button" className="clear-icon" onClick={onClearIcon} aria-label="Remover ícone">×</button>}</div></div>
        <fieldset className="color-field"><legend>Cor</legend><div>{ACCENTS.map((color) => <button key={color} type="button" className={button.color === color ? "selected" : ""} style={{ backgroundColor: color }} aria-label={`Usar cor ${color}`} onClick={() => onChange({ ...button, color })} />)}</div></fieldset>
      </div>
      <div className="row-actions">
        <button type="button" disabled={index === 0} onClick={() => onMove(-1)} aria-label="Mover para cima"><Icon name="up" /></button>
        <button type="button" disabled={index === total - 1} onClick={() => onMove(1)} aria-label="Mover para baixo"><Icon name="down" /></button>
        <button type="button" className="danger" onClick={onRemove} aria-label="Excluir botão"><Icon name="trash" /></button>
      </div>
    </article>
  );
}

function normalizeConfig(config: DeckConfig): DeckConfig {
  return {
    ...config,
    theme: config.theme === "dark" ? "dark" : "light",
    gridSize: ([3, 4, 5].includes(config.gridSize) ? config.gridSize : 4) as GridSize,
    buttons: config.buttons.map((button) => ({
      ...button,
      showLabel: Boolean(button.showLabel),
      transparentBackground: Boolean(button.transparentBackground),
    })),
  };
}

export default function App() {
  const [screen, setScreen] = useState<Screen>("deck");
  const [dashboard, setDashboard] = useState<DashboardState | null>(null);
  const [draft, setDraft] = useState<DeckConfig | null>(null);
  const [ports, setPorts] = useState<ServiceSettings | null>(null);
  const [status, setStatus] = useState("Conectando ao serviço local...");
  const [saving, setSaving] = useState(false);
  const [savingPorts, setSavingPorts] = useState(false);
  const [fetchingIconId, setFetchingIconId] = useState<string | null>(null);
  const volumeTimer = useRef<number | null>(null);

  const live = dashboard?.config ? normalizeConfig(dashboard.config) : null;
  const theme = (draft?.theme ?? live?.theme ?? "light") as ThemeMode;
  const gridSize = (live?.gridSize ?? 4) as GridSize;

  useEffect(() => {
    document.documentElement.dataset.theme = theme;
  }, [theme]);

  async function loadState(silent = false) {
    try {
      const next = await invoke<DashboardState>("get_dashboard_state");
      const config = normalizeConfig(next.config);
      setDashboard({ ...next, config });
      setDraft((current) => current ?? config);
      setPorts((current) => current ?? next.serviceSettings);
      if (!silent) setStatus("Serviço local pronto");
    } catch (error) {
      setStatus(`Não foi possível iniciar: ${String(error)}`);
    }
  }

  useEffect(() => {
    void loadState();
    const timer = window.setInterval(() => void loadState(true), 4000);
    return () => {
      window.clearInterval(timer);
      if (volumeTimer.current !== null) window.clearTimeout(volumeTimer.current);
    };
  }, []);

  async function publish(config: DeckConfig, message = "Alterações publicadas no celular") {
    setSaving(true);
    try {
      const saved = normalizeConfig(await invoke<DeckConfig>("save_config", { config }));
      setDraft(saved);
      setDashboard((current) => current ? { ...current, config: saved } : current);
      setStatus(message);
      return saved;
    } catch (error) {
      setStatus(`Falha ao salvar: ${String(error)}`);
      return null;
    } finally {
      setSaving(false);
    }
  }

  async function save() {
    if (!draft) return;
    await publish(normalizeConfig(draft));
  }

  async function toggleTheme() {
    const nextTheme: ThemeMode = theme === "dark" ? "light" : "dark";
    const base = draft ?? live;
    if (!base) return;
    const next = normalizeConfig({ ...base, theme: nextTheme });
    setDraft(next);
    await publish(next, nextTheme === "dark" ? "Modo escuro publicado" : "Modo claro publicado");
  }

  async function savePorts() {
    if (!ports) return;
    setSavingPorts(true);
    try {
      const settings = await invoke<ServiceSettings>("save_service_settings", { settings: ports });
      setPorts(settings);
      setDashboard((current) => current ? { ...current, serviceSettings: settings } : current);
      const restartNeeded = dashboard && (settings.port !== dashboard.port || settings.securePort !== dashboard.securePort);
      setStatus(restartNeeded ? "Portas salvas. Reinicie o aplicativo para aplicar." : "Portas confirmadas");
    } catch (error) {
      setStatus(`Falha nas portas: ${String(error)}`);
    } finally {
      setSavingPorts(false);
    }
  }

  function updateVolume(value: number) {
    setDashboard((current) => current ? { ...current, audio: { volume: value / 100, muted: false, microphoneMuted: current.audio?.microphoneMuted ?? null } } : current);
    if (volumeTimer.current !== null) window.clearTimeout(volumeTimer.current);
    volumeTimer.current = window.setTimeout(async () => {
      try {
        const audio = await invoke<AudioState>("set_output_volume", { value: value / 100 });
        setDashboard((current) => current ? { ...current, audio } : current);
      } catch (error) {
        setStatus(`Falha no áudio: ${String(error)}`);
      }
    }, 45);
  }

  async function toggleMute() {
    try {
      const audio = await invoke<AudioState>("toggle_output_mute");
      setDashboard((current) => current ? { ...current, audio } : current);
    } catch (error) {
      setStatus(`Falha no áudio: ${String(error)}`);
    }
  }

  async function toggleMicrophone() {
    try {
      const audio = await invoke<AudioState>("toggle_input_mute");
      setDashboard((current) => current ? { ...current, audio } : current);
    } catch (error) {
      setStatus(`Falha no microfone: ${String(error)}`);
    }
  }

  function changeButton(index: number, button: DeckButton) {
    if (!draft) return;
    const buttons = [...draft.buttons];
    buttons[index] = button;
    setDraft({ ...draft, buttons });
  }

  function moveButton(index: number, direction: -1 | 1) {
    if (!draft) return;
    const buttons = [...draft.buttons];
    const target = index + direction;
    [buttons[index], buttons[target]] = [buttons[target], buttons[index]];
    setDraft({ ...draft, buttons });
  }

  async function pickTarget(index: number) {
    if (!draft) return;
    const selected = await open({ multiple: false, directory: false, title: "Escolha um aplicativo ou atalho", filters: [{ name: "Aplicativos e atalhos", extensions: ["exe", "lnk", "bat", "cmd"] }] });
    if (typeof selected !== "string") return;
    const button = { ...draft.buttons[index], target: selected };
    try {
      const icon = await invoke<string>("extract_app_icon", { path: selected });
      changeButton(index, { ...button, icon });
      setStatus("Aplicativo e ícone importados; publique para enviar ao celular");
    } catch {
      changeButton(index, button);
      setStatus("Aplicativo selecionado; o Windows não forneceu um ícone utilizável");
    }
  }

  async function pickIcon(index: number) {
    if (!draft) return;
    const selected = await open({ multiple: false, directory: false, title: "Escolha o ícone do aplicativo", filters: [{ name: "Imagens", extensions: ["png", "jpg", "jpeg", "webp", "svg"] }] });
    if (typeof selected !== "string") return;
    try {
      const icon = await invoke<string>("read_icon_data_url", { path: selected });
      changeButton(index, { ...draft.buttons[index], icon });
      setStatus("Ícone adicionado; publique para enviar ao celular");
    } catch (error) {
      setStatus(`Falha no ícone: ${String(error)}`);
    }
  }

  async function fetchIcon(index: number) {
    if (!draft) return;
    const button = draft.buttons[index];
    setFetchingIconId(button.id);
    try {
      const icon = await invoke<string>("fetch_site_icon", { url: button.target });
      changeButton(index, { ...button, icon });
      setStatus("Ícone do site importado; publique para enviar ao celular");
    } catch (error) {
      setStatus(`Falha no ícone do site: ${String(error)}`);
    } finally {
      setFetchingIconId(null);
    }
  }

  function addButton(kind: DeckButton["kind"] = "application") {
    if (!draft) return;
    if (draft.buttons.length >= 25) {
      setStatus("Limite de 25 atalhos atingido");
      return;
    }
    setDraft({
      ...draft,
      buttons: [...draft.buttons, {
        id: crypto.randomUUID(),
        label: kind === "url" ? "Novo site" : "Novo atalho",
        target: kind === "url" ? "https://" : "",
        kind,
        color: ACCENTS[draft.buttons.length % ACCENTS.length],
        icon: null,
        showLabel: false,
        transparentBackground: false,
        unreadProvider: null,
      }],
    });
  }

  async function launchTile(id: string) {
    try {
      await invoke("launch_button", { id });
    } catch (error) {
      setStatus(`Falha ao abrir: ${String(error)}`);
    }
  }

  async function regeneratePairing() {
    try {
      const pairingUrl = await invoke<string>("regenerate_pairing");
      setDashboard((current) => current ? { ...current, pairingUrl } : current);
      setStatus("Novo acesso gerado; o QR anterior foi invalidado");
    } catch (error) {
      setStatus(`Falha ao gerar acesso: ${String(error)}`);
    }
  }

  const microphoneMuted = dashboard?.audio?.microphoneMuted;
  const qrFg = theme === "dark" ? "#e8e4dc" : "#17211f";
  const qrBg = theme === "dark" ? "#1a2220" : "#f5f0e5";
  const portsDirty = ports && dashboard && (ports.port !== dashboard.serviceSettings.port || ports.securePort !== dashboard.serviceSettings.securePort);
  const portsRestart = ports && dashboard && (ports.port !== dashboard.port || ports.securePort !== dashboard.securePort);

  if (screen === "deck") {
    return (
      <main className="deck-screen">
        <header className="deck-topbar">
          <div className="brand-block">
            <div className="brand-mark">OP</div>
            <div>
              <p>OPEN PRODUCTIVITY</p>
              <h1>{live?.title || "Deck"}</h1>
            </div>
          </div>
          <div className="topbar-tools">
            <button type="button" className="theme-toggle" onClick={() => void toggleTheme()} aria-label={theme === "dark" ? "Usar modo claro" : "Usar modo escuro"}>
              <Icon name={theme === "dark" ? "sun" : "moon"} />
            </button>
            <button type="button" className="theme-toggle" onClick={() => setScreen("settings")} aria-label="Abrir configurações">
              <Icon name="settings" />
            </button>
          </div>
        </header>

        <section className="deck-stage">
          <div className="deck-grid" style={{ "--grid-size": String(gridSize) } as React.CSSProperties}>
            {live?.buttons.map((button) => (
              <DeckTile
                key={button.id}
                button={button}
                unread={button.unreadProvider ? dashboard?.unread[button.unreadProvider] : null}
                onClick={() => void launchTile(button.id)}
              />
            ))}
            {(!live || live.buttons.length === 0) && (
              <div className="deck-empty">
                <h2>Nenhum atalho publicado</h2>
                <p>Abra as configurações para montar o deck do celular.</p>
                <button type="button" className="primary-button" onClick={() => setScreen("settings")}>Abrir configurações</button>
              </div>
            )}
          </div>
        </section>

        <footer className="deck-footer">
          <span className={`service-status inline`}><span className={dashboard ? "online" : ""} />{status}</span>
          <span>{gridSize}×{gridSize}</span>
        </footer>
      </main>
    );
  }

  return (
    <main className="settings-screen">
      <header className="topbar">
        <div className="brand-block">
          <button type="button" className="theme-toggle" onClick={() => setScreen("deck")} aria-label="Voltar ao deck">
            <Icon name="back" />
          </button>
          <div>
            <p>CONFIGURAÇÕES</p>
            <h1>Deck</h1>
          </div>
        </div>
        <div className="topbar-tools">
          <button type="button" className="theme-toggle" onClick={() => void toggleTheme()} aria-label={theme === "dark" ? "Usar modo claro" : "Usar modo escuro"}>
            <Icon name={theme === "dark" ? "sun" : "moon"} />
          </button>
          <div className="service-status"><span className={dashboard ? "online" : ""} />{status}</div>
        </div>
      </header>

      <section className="hero-grid">
        <div className="audio-panel">
          <h2 className="panel-title">Áudio</h2>
          <VolumeDial audio={dashboard?.audio ?? null} onChange={updateVolume} />
          <div className="audio-actions">
            <button className={`mute-button ${dashboard?.audio?.muted ? "active" : ""}`} type="button" onClick={() => void toggleMute()}><Icon name={dashboard?.audio?.muted ? "volume" : "mute"} />{dashboard?.audio?.muted ? "Restaurar" : "Mutar"}</button>
            <button className={`mute-button ${microphoneMuted ? "active" : ""}`} disabled={microphoneMuted == null} type="button" onClick={() => void toggleMicrophone()}><Icon name="microphone" />{microphoneMuted == null ? "Sem mic" : microphoneMuted ? "Ligar mic" : "Mutar mic"}</button>
          </div>
        </div>

        <div className="pairing-panel">
          <h2 className="panel-title">Celular</h2>
          <div className="pairing-content">
            <div className="qr-frame">{dashboard?.pairingUrl ? <QRCodeSVG value={dashboard.pairingUrl} size={220} bgColor={qrBg} fgColor={qrFg} level="M" marginSize={1} /> : <div className="qr-placeholder" />}</div>
            <div className="pairing-copy">
              <Icon name="phone" />
              <h3>Conecte o celular</h3>
              <p>Leia o QR para instalar o certificado e abrir a PWA na rede local.</p>
              <code>{dashboard ? `${dashboard.localAddress}:${dashboard.port} → :${dashboard.securePort}` : "localizando..."}</code>
              <button type="button" className="text-button" onClick={() => void regeneratePairing()}><Icon name="refresh" />Gerar novo QR</button>
            </div>
          </div>
        </div>
      </section>

      <section className="service-section appearance-section">
        <div className="service-copy">
          <h2 className="panel-title">Aparência</h2>
          <p>Tema e grade do deck no computador e no celular.</p>
        </div>
        <div className="appearance-fields">
          <label>Grade
            <select
              value={draft?.gridSize ?? 4}
              onChange={(event) => draft && setDraft({ ...draft, gridSize: Number(event.target.value) as GridSize })}
            >
              <option value={3}>3×3</option>
              <option value={4}>4×4</option>
              <option value={5}>5×5</option>
            </select>
          </label>
          <label className="toggle-field"><span>Modo escuro</span>
            <button type="button" className={`toggle ${theme === "dark" ? "on" : ""}`} role="switch" aria-checked={theme === "dark"} onClick={() => void toggleTheme()}><i /></button>
          </label>
        </div>
      </section>

      <section className="service-section">
        <div className="service-copy">
          <h2 className="panel-title">Portas do serviço</h2>
          <p>Use outras portas se 37621/37622 estiverem ocupadas. Reinicie o app após salvar.</p>
        </div>
        <div className="service-fields">
          <label>HTTP<input type="number" min={1024} max={65535} value={ports?.port ?? ""} onChange={(event) => setPorts((current) => current ? { ...current, port: Number(event.target.value) || 0 } : current)} /></label>
          <label>HTTPS<input type="number" min={1024} max={65535} value={ports?.securePort ?? ""} onChange={(event) => setPorts((current) => current ? { ...current, securePort: Number(event.target.value) || 0 } : current)} /></label>
          <button type="button" className="secondary-button" disabled={savingPorts || !ports || !portsDirty} onClick={() => void savePorts()}>{savingPorts ? "Salvando..." : "Salvar portas"}</button>
        </div>
        {portsRestart && <p className="service-hint">Reinício necessário para aplicar {ports?.port}/{ports?.securePort}.</p>}
      </section>

      <section className="deck-section">
        <div className="deck-heading">
          <div>
            <h2 className="panel-title">Atalhos</h2>
            <p>Publique para atualizar a grade do celular e a tela inicial.</p>
          </div>
          <div className="deck-heading-actions">
            <button type="button" className="secondary-button" onClick={() => addButton("application")}><Icon name="plus" />App</button>
            <button type="button" className="secondary-button" onClick={() => addButton("url")}><Icon name="link" />Site</button>
            <button type="button" className="primary-button" disabled={saving || !draft} onClick={() => void save()}><Icon name="save" />{saving ? "Publicando..." : "Publicar"}</button>
          </div>
        </div>

        <div className="button-list">
          {draft?.buttons.map((button, index) => (
            <ButtonEditor
              key={button.id}
              button={button}
              index={index}
              total={draft.buttons.length}
              fetchingIcon={fetchingIconId === button.id}
              onChange={(next) => changeButton(index, next)}
              onMove={(direction) => moveButton(index, direction)}
              onRemove={() => setDraft({ ...draft, buttons: draft.buttons.filter((_, itemIndex) => itemIndex !== index) })}
              onPick={() => void pickTarget(index)}
              onPickIcon={() => void pickIcon(index)}
              onClearIcon={() => changeButton(index, { ...button, icon: null })}
              onFetchIcon={() => void fetchIcon(index)}
            />
          ))}
          {draft?.buttons.length === 0 && (
            <div className="empty-state">
              <h3>Nenhum atalho ainda</h3>
              <p>Adicione um aplicativo ou um site para começar.</p>
              <div className="deck-heading-actions">
                <button type="button" className="secondary-button" onClick={() => addButton("application")}>Adicionar app</button>
                <button type="button" className="secondary-button" onClick={() => addButton("url")}>Adicionar site</button>
              </div>
            </div>
          )}
        </div>
      </section>

      <footer><span>Open Productivity Deck 0.7.0</span><span>GPL-3.0-or-later · sem garantia</span></footer>
    </main>
  );
}
