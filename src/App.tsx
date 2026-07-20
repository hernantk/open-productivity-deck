import { useEffect, useRef, useState, type PointerEvent as ReactPointerEvent } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { QRCodeSVG } from "qrcode.react";
import type { AudioState, DashboardState, DeckButton, DeckConfig } from "./types";

const ACCENTS = ["#e9592f", "#286c64", "#d39a22", "#675a9e", "#a63d53", "#3f658c"];

function Icon({ name }: { name: "volume" | "mute" | "microphone" | "phone" | "plus" | "save" | "refresh" | "folder" | "up" | "down" | "trash" | "image" }) {
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

function ActionPreview({ button, unread }: { button: DeckButton; unread: number | null | undefined }) {
  const initials = button.label.trim().split(/\s+/).slice(0, 2).map((word) => word[0]).join("").toUpperCase() || "?";
  return (
    <div className="action-preview" style={{ "--button-color": button.color } as React.CSSProperties}>
      <span className="action-glyph">{button.icon ? <img src={button.icon} alt="" /> : initials}</span>
      <span>{button.label || "Sem nome"}</span>
      {typeof unread === "number" && unread > 0 && <b className="unread-badge">{unread > 99 ? "99+" : unread}</b>}
    </div>
  );
}

function ButtonEditor({ button, index, total, onChange, onMove, onRemove, onPick, onPickIcon, onClearIcon }: {
  button: DeckButton;
  index: number;
  total: number;
  onChange: (button: DeckButton) => void;
  onMove: (direction: -1 | 1) => void;
  onRemove: () => void;
  onPick: () => void;
  onPickIcon: () => void;
  onClearIcon: () => void;
}) {
  return (
    <article className="button-editor">
      <ActionPreview button={button} unread={null} />
      <div className="editor-fields">
        <label>Nome<input value={button.label} maxLength={32} onChange={(event) => onChange({ ...button, label: event.target.value })} /></label>
        <label>Tipo<select value={button.kind} onChange={(event) => onChange({ ...button, kind: event.target.value as DeckButton["kind"] })}><option value="application">Aplicativo ou arquivo</option><option value="url">Endereço ou protocolo</option></select></label>
        <label className="target-field">Destino<span className="input-with-action"><input value={button.target} placeholder={button.kind === "url" ? "https://... ou protocolo:" : "C:\\...\\aplicativo.exe"} onChange={(event) => onChange({ ...button, target: event.target.value })} />{button.kind === "application" && <button type="button" className="field-action" onClick={onPick} aria-label="Procurar aplicativo"><Icon name="folder" /></button>}</span></label>
        <label>Contador<select value={button.unreadProvider ?? ""} onChange={(event) => onChange({ ...button, unreadProvider: (event.target.value || null) as DeckButton["unreadProvider"] })}><option value="">Nenhum</option><option value="teams">Microsoft Teams</option><option value="whatsapp">WhatsApp</option></select></label>
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

export default function App() {
  const [dashboard, setDashboard] = useState<DashboardState | null>(null);
  const [draft, setDraft] = useState<DeckConfig | null>(null);
  const [status, setStatus] = useState("Conectando ao serviço local...");
  const [saving, setSaving] = useState(false);
  const volumeTimer = useRef<number | null>(null);

  async function loadState(silent = false) {
    try {
      const next = await invoke<DashboardState>("get_dashboard_state");
      setDashboard(next);
      setDraft((current) => current ?? next.config);
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

  async function save() {
    if (!draft) return;
    setSaving(true);
    try {
      const config = await invoke<DeckConfig>("save_config", { config: draft });
      setDraft(config);
      setDashboard((current) => current ? { ...current, config } : current);
      setStatus("Alterações publicadas no celular");
    } catch (error) {
      setStatus(`Falha ao salvar: ${String(error)}`);
    } finally {
      setSaving(false);
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
    if (typeof selected === "string") changeButton(index, { ...draft.buttons[index], target: selected });
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

  function addButton() {
    if (!draft) return;
    setDraft({ ...draft, buttons: [...draft.buttons, { id: crypto.randomUUID(), label: "Novo atalho", target: "", kind: "application", color: ACCENTS[draft.buttons.length % ACCENTS.length], icon: null, unreadProvider: null }] });
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

  return (
    <main>
      <header className="topbar">
        <div className="brand-mark">OP</div>
        <div><p>OPEN PRODUCTIVITY</p><h1>Deck de controle</h1></div>
        <div className="service-status"><span className={dashboard ? "online" : ""} />{status}</div>
      </header>

      <section className="hero-grid">
        <div className="audio-panel">
          <div className="section-kicker"><span>01</span> SAÍDA PADRÃO DO WINDOWS</div>
          <VolumeDial audio={dashboard?.audio ?? null} onChange={updateVolume} />
          <div className="audio-actions"><button className={`mute-button ${dashboard?.audio?.muted ? "active" : ""}`} type="button" onClick={() => void toggleMute()}><Icon name={dashboard?.audio?.muted ? "volume" : "mute"} />{dashboard?.audio?.muted ? "Restaurar som" : "Mutar saída"}</button><button className={`mute-button ${microphoneMuted ? "active" : ""}`} disabled={microphoneMuted == null} type="button" onClick={() => void toggleMicrophone()}><Icon name="microphone" />{microphoneMuted == null ? "Microfone indisponível" : microphoneMuted ? "Ligar microfone" : "Mutar microfone"}</button></div>
        </div>

        <div className="pairing-panel">
          <div className="section-kicker"><span>02</span> CONECTAR CELULAR</div>
          <div className="pairing-content">
            <div className="qr-frame">{dashboard?.pairingUrl ? <QRCodeSVG value={dashboard.pairingUrl} size={248} bgColor="#f5f0e5" fgColor="#17211f" level="M" marginSize={1} /> : <div className="qr-placeholder" />}</div>
            <div className="pairing-copy"><Icon name="phone" /><h2>Aponte. Instale. Controle.</h2><p>O QR prepara o certificado local e abre a PWA segura no celular.</p><code>{dashboard ? `${dashboard.localAddress}:${dashboard.port} → HTTPS :${dashboard.securePort}` : "localizando..."}</code><button type="button" className="text-button" onClick={() => void regeneratePairing()}><Icon name="refresh" />Invalidar e gerar novo QR</button></div>
          </div>
        </div>
      </section>

      <section className="deck-section">
        <div className="deck-heading">
          <div><div className="section-kicker"><span>03</span> ATALHOS DO DECK</div><h2>Monte sua superfície de trabalho.</h2><p>O celular recebe apenas ações já aprovadas aqui.</p></div>
          <div className="deck-heading-actions"><button type="button" className="secondary-button" onClick={addButton}><Icon name="plus" />Adicionar botão</button><button type="button" className="primary-button" disabled={saving || !draft} onClick={() => void save()}><Icon name="save" />{saving ? "Publicando..." : "Publicar alterações"}</button></div>
        </div>

        <div className="button-list">
          {draft?.buttons.map((button, index) => <ButtonEditor key={button.id} button={button} index={index} total={draft.buttons.length} onChange={(next) => changeButton(index, next)} onMove={(direction) => moveButton(index, direction)} onRemove={() => setDraft({ ...draft, buttons: draft.buttons.filter((_, itemIndex) => itemIndex !== index) })} onPick={() => void pickTarget(index)} onPickIcon={() => void pickIcon(index)} onClearIcon={() => changeButton(index, { ...button, icon: null })} />)}
          {draft?.buttons.length === 0 && <div className="empty-state"><span>+</span><h3>Seu deck está limpo.</h3><p>Adicione o primeiro atalho para começar.</p><button type="button" className="secondary-button" onClick={addButton}>Adicionar botão</button></div>}
        </div>
      </section>

      <footer><span>Open Productivity Deck</span><span>GPL-3.0-or-later · sem garantia · consulte LICENSE</span></footer>
    </main>
  );
}
