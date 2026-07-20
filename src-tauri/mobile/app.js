const TOKEN_KEY = "open-productivity-deck-token";
const SESSIONS_KEY = "open-productivity-deck-sessions-v1";
const ACTIVE_SESSION_KEY = "open-productivity-deck-active-session";

const elements = {
  deck: document.getElementById("deck-layout"),
  chooser: document.getElementById("device-chooser"),
  deviceList: document.getElementById("device-list"),
  addDevice: document.getElementById("add-device"),
  backToDeck: document.getElementById("back-to-deck"),
  pairingView: document.getElementById("pairing-view"),
  pairingVideo: document.getElementById("pairing-video"),
  pairingUrl: document.getElementById("pairing-url"),
  pairingMessage: document.getElementById("pairing-message"),
  connectDevice: document.getElementById("connect-device"),
  cancelPairing: document.getElementById("cancel-pairing"),
  switchDevice: document.getElementById("switch-device"),
  dial: document.getElementById("dial"),
  volume: document.getElementById("volume"),
  unit: document.getElementById("unit"),
  mute: document.getElementById("mute"),
  microphone: document.getElementById("microphone"),
  actions: document.getElementById("actions"),
  connection: document.getElementById("connection"),
  toast: document.getElementById("toast"),
  installButton: document.getElementById("install-app"),
  spotifyPlayer: document.getElementById("spotify-player"),
  spotifyTitle: document.getElementById("spotify-title"),
  spotifyArtist: document.getElementById("spotify-artist"),
  spotifyPrevious: document.getElementById("spotify-previous"),
  spotifyToggle: document.getElementById("spotify-toggle"),
  spotifyNext: document.getElementById("spotify-next"),
  spotifyPlayIcon: document.getElementById("spotify-play-icon"),
};

let sessions = loadSessions();
let activeSession = null;
let state = null;
let eventSource = null;
let cameraStream = null;
let scanning = false;
let toastTimer = null;
let volumeTimer = null;
let lastVolume = 0;
let lastHapticStep = -1;
let installPrompt = null;

const launchParams = new URLSearchParams(location.search);
const suppliedToken = launchParams.get("token");
const pendingHub = normalizeHub(launchParams.get("hub"));
let importedSession = false;

if (suppliedToken) {
  localStorage.setItem(TOKEN_KEY, suppliedToken);
  upsertSession({
    id: location.origin,
    name: location.hostname,
    deckTitle: "Productivity Deck",
    baseUrl: location.origin,
    token: suppliedToken,
  });
  history.replaceState({}, "", `${location.pathname}${location.hash}`);
} else {
  const legacyToken = localStorage.getItem(TOKEN_KEY) || sessionStorage.getItem("deck-token");
  if (!sessions.length && legacyToken) {
    upsertSession({
      id: location.origin,
      name: location.hostname,
      deckTitle: "Productivity Deck",
      baseUrl: location.origin,
      token: legacyToken,
    });
  }
  sessionStorage.removeItem("deck-token");
}

if (location.hash.startsWith("#import=")) {
  try {
    const imported = JSON.parse(decodePayload(location.hash.slice(8)));
    if (validSession(imported)) {
      upsertSession(imported);
      importedSession = true;
    }
  } catch {}
  history.replaceState({}, "", location.pathname);
}

function loadSessions() {
  try {
    const stored = JSON.parse(localStorage.getItem(SESSIONS_KEY) || "[]");
    return Array.isArray(stored) ? stored.filter(validSession) : [];
  } catch {
    return [];
  }
}

function saveSessions() {
  localStorage.setItem(SESSIONS_KEY, JSON.stringify(sessions));
}

function validSession(session) {
  if (!session || typeof session.token !== "string" || session.token.length < 16) return false;
  try {
    const url = new URL(session.baseUrl);
    return url.protocol === "https:" && isPrivateHost(url.hostname);
  } catch {
    return false;
  }
}

function upsertSession(session) {
  if (!validSession(session)) return;
  const normalized = { ...session, baseUrl: new URL(session.baseUrl).origin };
  const index = sessions.findIndex((item) => item.id === normalized.id || item.baseUrl === normalized.baseUrl);
  if (index >= 0) sessions[index] = { ...sessions[index], ...normalized };
  else sessions.push(normalized);
  saveSessions();
}

function removeSession(session) {
  sessions = sessions.filter((item) => item.id !== session.id && item.baseUrl !== session.baseUrl);
  if (activeSession && (activeSession.id === session.id || activeSession.baseUrl === session.baseUrl)) activeSession = null;
  saveSessions();
}

function isPrivateHost(hostname) {
  if (hostname === "localhost" || hostname === "127.0.0.1") return true;
  const parts = hostname.split(".").map(Number);
  if (parts.length !== 4 || parts.some((part) => !Number.isInteger(part) || part < 0 || part > 255)) return false;
  return parts[0] === 10 || (parts[0] === 192 && parts[1] === 168) || (parts[0] === 172 && parts[1] >= 16 && parts[1] <= 31);
}

function normalizeHub(value) {
  if (!value) return null;
  try {
    const url = new URL(value);
    return url.protocol === "https:" && isPrivateHost(url.hostname) ? url.origin : null;
  } catch {
    return null;
  }
}

function encodePayload(value) {
  const bytes = new TextEncoder().encode(JSON.stringify(value));
  let binary = "";
  for (const byte of bytes) binary += String.fromCharCode(byte);
  return btoa(binary).replaceAll("+", "-").replaceAll("/", "_").replaceAll("=", "");
}

function decodePayload(value) {
  const base64 = value.replaceAll("-", "+").replaceAll("_", "/").padEnd(Math.ceil(value.length / 4) * 4, "=");
  const binary = atob(base64);
  return new TextDecoder().decode(Uint8Array.from(binary, (character) => character.charCodeAt(0)));
}

function api(path, options = {}) {
  if (!activeSession) return Promise.reject(new Error("Escolha um computador"));
  const url = new URL(path, activeSession.baseUrl);
  url.searchParams.set("token", activeSession.token);
  return fetch(url, {
    ...options,
    headers: { "Content-Type": "application/json", ...(options.headers || {}) },
  }).then(async (response) => {
    if (response.status === 401) {
      const expired = activeSession;
      removeSession(expired);
      showChooser();
      throw new Error(`O acesso a ${expired.name} expirou`);
    }
    if (!response.ok) throw new Error((await response.text()) || "Falha na comunicação");
    return response.status === 204 ? null : response.json();
  });
}

function showToast(message) {
  elements.toast.textContent = message;
  elements.toast.classList.add("show");
  clearTimeout(toastTimer);
  toastTimer = setTimeout(() => elements.toast.classList.remove("show"), 2600);
}

function showChooser() {
  closeEvents();
  stopCamera();
  elements.deck.style.display = "none";
  elements.chooser.style.display = "grid";
  elements.pairingView.classList.remove("active");
  elements.deviceList.style.display = "grid";
  elements.backToDeck.style.display = activeSession ? "block" : "none";
  renderDeviceList();
}

function hideChooser() {
  stopCamera();
  elements.chooser.style.display = "none";
  elements.deck.style.display = "grid";
  elements.switchDevice.style.display = "block";
}

function renderDeviceList() {
  elements.deviceList.replaceChildren();
  if (!sessions.length) {
    const empty = document.createElement("div");
    empty.className = "empty";
    empty.textContent = "Nenhum computador conectado. Leia o primeiro QR Code para começar.";
    elements.deviceList.append(empty);
    return;
  }

  for (const session of sessions) {
    const row = document.createElement("div");
    row.className = "device-row";
    const button = document.createElement("button");
    button.type = "button";
    button.className = "device-card";
    const status = document.createElement("i");
    const copy = document.createElement("span");
    const name = document.createElement("strong");
    name.textContent = session.name || "Computador";
    const address = document.createElement("span");
    address.textContent = `${session.deckTitle || "Productivity Deck"} · ${new URL(session.baseUrl).host}`;
    copy.append(name, address);
    button.append(status, copy);
    button.addEventListener("click", () => activateSession(session));

    const remove = document.createElement("button");
    remove.type = "button";
    remove.className = "remove-device";
    remove.setAttribute("aria-label", `Remover ${session.name}`);
    remove.textContent = "×";
    remove.addEventListener("click", () => {
      removeSession(session);
      renderDeviceList();
    });
    row.append(button, remove);
    elements.deviceList.append(row);
    pingSession(session, button);
  }
}

async function pingSession(session, button) {
  try {
    const response = await fetch(`${session.baseUrl}/health`, { cache: "no-store" });
    button.classList.toggle("online", response.ok);
  } catch {
    button.classList.remove("online");
  }
}

function activateSession(session) {
  activeSession = session;
  localStorage.setItem(ACTIVE_SESSION_KEY, session.id);
  state = null;
  hideChooser();
  load();
  connectEvents();
}

function beginPairing() {
  elements.deviceList.style.display = "none";
  elements.pairingView.classList.add("active");
  elements.pairingMessage.textContent = "Aponte a câmera para o QR do outro computador.";
  startCamera();
}

async function startCamera() {
  if (!("BarcodeDetector" in window) || !navigator.mediaDevices?.getUserMedia) {
    elements.pairingMessage.textContent = "Leitura automática indisponível. Cole o endereço exibido no QR.";
    return;
  }
  try {
    cameraStream = await navigator.mediaDevices.getUserMedia({ video: { facingMode: { ideal: "environment" } }, audio: false });
    elements.pairingVideo.srcObject = cameraStream;
    await elements.pairingVideo.play();
    scanning = true;
    const detector = new BarcodeDetector({ formats: ["qr_code"] });
    scanCamera(detector);
  } catch {
    elements.pairingMessage.textContent = "Não foi possível usar a câmera. Cole o endereço do QR abaixo.";
  }
}

async function scanCamera(detector) {
  if (!scanning) return;
  try {
    const codes = await detector.detect(elements.pairingVideo);
    if (codes[0]?.rawValue) {
      connectPairingUrl(codes[0].rawValue);
      return;
    }
  } catch {}
  requestAnimationFrame(() => scanCamera(detector));
}

function stopCamera() {
  scanning = false;
  if (cameraStream) cameraStream.getTracks().forEach((track) => track.stop());
  cameraStream = null;
  elements.pairingVideo.srcObject = null;
}

function connectPairingUrl(rawValue) {
  try {
    const url = new URL(rawValue.trim());
    if (!["http:", "https:"].includes(url.protocol) || !isPrivateHost(url.hostname) || !url.searchParams.get("token")) {
      throw new Error();
    }
    if (url.hostname === location.hostname) {
      showToast("Este computador já é o hub atual");
      return;
    }
    url.searchParams.set("hub", location.origin);
    stopCamera();
    location.assign(url.toString());
  } catch {
    elements.pairingMessage.textContent = "Endereço inválido. Use o QR gerado pelo Open Productivity Deck.";
  }
}

function initials(label) {
  return label.trim().split(/\s+/).slice(0, 2).map((word) => word[0]).join("").toUpperCase() || "?";
}

function render(next) {
  state = next;
  document.title = `${next.title} · Productivity Deck`;
  elements.connection.classList.add("online");
  elements.connection.querySelector("span").textContent = next.deviceName || "local";

  if (activeSession) {
    const oldId = activeSession.id;
    activeSession = {
      ...activeSession,
      id: next.deviceId || activeSession.id,
      name: next.deviceName || activeSession.name,
      deckTitle: next.title || activeSession.deckTitle,
    };
    sessions = sessions.map((session) => session.id === oldId || session.baseUrl === activeSession.baseUrl ? activeSession : session);
    saveSessions();
    localStorage.setItem(ACTIVE_SESSION_KEY, activeSession.id);
  }

  if (pendingHub && pendingHub !== location.origin && activeSession) {
    location.replace(`${pendingHub}/#import=${encodePayload(activeSession)}`);
    return;
  }

  if (next.audio) setAudio(next.audio);
  if (next.spotify) setSpotify(next.spotify);
  elements.actions.replaceChildren();
  if (!next.buttons.length) {
    const empty = document.createElement("div");
    empty.className = "empty";
    empty.textContent = "Configure os primeiros atalhos no computador.";
    elements.actions.append(empty);
  }
  next.buttons.forEach((button) => {
    const showLabel = Boolean(button.showLabel);
    const element = document.createElement("button");
    element.type = "button";
    element.className = showLabel ? "action" : "action icon-only";
    element.style.setProperty("--color", button.color);
    element.setAttribute("aria-label", button.label);
    const glyph = document.createElement("span");
    glyph.className = "glyph";
    if (button.icon) {
      const image = document.createElement("img");
      image.src = button.icon;
      image.alt = "";
      glyph.append(image);
    } else glyph.textContent = initials(button.label);
    const label = document.createElement("span");
    label.className = "action-label";
    label.textContent = button.label;
    element.append(glyph, label);
    const count = button.unreadProvider ? next.unread[button.unreadProvider] : null;
    if (Number.isFinite(count) && count > 0) {
      const badge = document.createElement("b");
      badge.className = "badge";
      badge.textContent = count > 99 ? "99+" : String(count);
      element.append(badge);
    }
    element.addEventListener("click", () => launch(button));
    elements.actions.append(element);
  });
}

function setAudio(audio) {
  if (!audio) return;
  state = { ...(state || {}), audio };
  lastVolume = Math.round(audio.volume * 100);
  elements.volume.textContent = lastVolume;
  elements.dial.style.setProperty("--value", lastVolume);
  elements.dial.style.setProperty("--angle", `${135 + lastVolume * 2.7}deg`);
  elements.dial.setAttribute("aria-valuenow", String(lastVolume));
  elements.dial.setAttribute("aria-valuetext", `${lastVolume} por cento${audio.muted ? ", mudo" : ""}`);
  elements.dial.classList.toggle("muted", audio.muted);
  elements.mute.classList.toggle("active", audio.muted);
  elements.mute.querySelector("span").textContent = audio.muted ? "Som mudo" : "Som";
  elements.unit.textContent = audio.muted ? "MUDO" : "VOLUME";
  elements.microphone.disabled = audio.microphoneMuted === null;
  elements.microphone.classList.toggle("active", audio.microphoneMuted === true);
  elements.microphone.querySelector("span").textContent = audio.microphoneMuted === null ? "Sem mic" : audio.microphoneMuted ? "Mic mudo" : "Microfone";
}

function setSpotify(spotify) {
  if (!spotify) return;
  state = { ...(state || {}), spotify };
  elements.spotifyPlayer.classList.toggle("unavailable", !spotify.available);
  elements.spotifyTitle.textContent = spotify.title || "Faixa sem título";
  elements.spotifyArtist.textContent = spotify.artist || "Spotify";
  [elements.spotifyPrevious, elements.spotifyToggle, elements.spotifyNext].forEach((button) => button.disabled = !spotify.available);
  elements.spotifyToggle.setAttribute("aria-label", spotify.playing ? "Pausar" : "Reproduzir");
  elements.spotifyPlayIcon.innerHTML = spotify.playing ? '<path d="M7 5h4v14H7zM14 5h4v14h-4z"/>' : '<path d="M8 5v14l11-7z"/>';
}

function load() {
  if (!activeSession) return;
  api("/api/state").then(render).catch((error) => {
    elements.connection.classList.remove("online");
    elements.connection.querySelector("span").textContent = "offline";
    if (!error.message.includes("expirou")) showToast(error.message);
  });
}

function launch(button) {
  navigator.vibrate?.(20);
  api(`/api/actions/${button.id}`, { method: "POST" })
    .then(() => showToast(`${button.label} aberto em ${activeSession.name}`))
    .catch((error) => showToast(error.message));
}

function loadSpotify() {
  if (activeSession && elements.chooser.style.display !== "grid") api("/api/spotify").then(setSpotify).catch(() => {});
}

async function controlSpotify(action) {
  navigator.vibrate?.(15);
  [elements.spotifyPrevious, elements.spotifyToggle, elements.spotifyNext].forEach((button) => button.disabled = true);
  try {
    const spotify = await api(`/api/spotify/${action}`, { method: "POST" });
    setSpotify(spotify);
    setTimeout(loadSpotify, 350);
  } catch (error) {
    showToast(error.message);
    setSpotify(state?.spotify);
  }
}

function sendVolume(value) {
  lastVolume = Math.max(0, Math.min(100, Math.round(value)));
  setAudio({ volume: lastVolume / 100, muted: false, microphoneMuted: state?.audio?.microphoneMuted ?? null });
  clearTimeout(volumeTimer);
  volumeTimer = setTimeout(() => api("/api/volume", { method: "POST", body: JSON.stringify({ value: lastVolume / 100 }) }).then(setAudio).catch((error) => showToast(error.message)), 55);
}

function volumeFromPointer(event) {
  const rect = elements.dial.getBoundingClientRect();
  const x = event.clientX - rect.left - rect.width / 2;
  const y = event.clientY - rect.top - rect.height / 2;
  if (Math.hypot(x, y) < rect.width * .25) return false;
  let angle = Math.atan2(y, x) * 180 / Math.PI;
  if (angle < 0) angle += 360;
  let travel = (angle - 135 + 360) % 360;
  if (travel > 270) travel = lastVolume >= 50 ? 270 : 0;
  const next = Math.round(travel / 270 * 100);
  const step = Math.floor(next / 10);
  if (step !== lastHapticStep) {
    navigator.vibrate?.(4);
    lastHapticStep = step;
  }
  sendVolume(next);
  return true;
}

function closeEvents() {
  if (eventSource) eventSource.close();
  eventSource = null;
}

function connectEvents() {
  closeEvents();
  if (!activeSession) return;
  const url = new URL("/api/events", activeSession.baseUrl);
  url.searchParams.set("token", activeSession.token);
  eventSource = new EventSource(url);
  eventSource.addEventListener("unread", (event) => {
    if (!state) return;
    try { render({ ...state, unread: JSON.parse(event.data) }); } catch {}
  });
}

elements.dial.addEventListener("pointerdown", (event) => {
  if (volumeFromPointer(event)) {
    elements.dial.setPointerCapture(event.pointerId);
    elements.dial.classList.add("dragging");
  }
});
elements.dial.addEventListener("pointermove", (event) => {
  if (elements.dial.hasPointerCapture(event.pointerId)) volumeFromPointer(event);
});
elements.dial.addEventListener("lostpointercapture", () => elements.dial.classList.remove("dragging"));
elements.dial.addEventListener("pointercancel", () => elements.dial.classList.remove("dragging"));
elements.dial.addEventListener("keydown", (event) => {
  if (["ArrowUp", "ArrowRight"].includes(event.key)) { event.preventDefault(); sendVolume(lastVolume + 2); }
  if (["ArrowDown", "ArrowLeft"].includes(event.key)) { event.preventDefault(); sendVolume(lastVolume - 2); }
});
elements.mute.addEventListener("click", () => { navigator.vibrate?.(15); api("/api/mute", { method: "POST" }).then(setAudio).catch((error) => showToast(error.message)); });
elements.microphone.addEventListener("click", () => { navigator.vibrate?.(15); api("/api/microphone", { method: "POST" }).then(setAudio).catch((error) => showToast(error.message)); });
elements.spotifyPrevious.addEventListener("click", () => controlSpotify("previous"));
elements.spotifyToggle.addEventListener("click", () => controlSpotify("toggle"));
elements.spotifyNext.addEventListener("click", () => controlSpotify("next"));
elements.switchDevice.addEventListener("click", showChooser);
elements.addDevice.addEventListener("click", beginPairing);
elements.backToDeck.addEventListener("click", hideChooser);
elements.cancelPairing.addEventListener("click", showChooser);
elements.connectDevice.addEventListener("click", () => connectPairingUrl(elements.pairingUrl.value));
elements.pairingUrl.addEventListener("keydown", (event) => { if (event.key === "Enter") connectPairingUrl(elements.pairingUrl.value); });

window.addEventListener("beforeinstallprompt", (event) => {
  event.preventDefault();
  installPrompt = event;
  elements.installButton.style.display = "block";
});
elements.installButton.addEventListener("click", async () => {
  if (!installPrompt) return;
  installPrompt.prompt();
  await installPrompt.userChoice;
  installPrompt = null;
  elements.installButton.style.display = "none";
});
window.addEventListener("appinstalled", () => elements.installButton.style.display = "none");
if ("serviceWorker" in navigator) navigator.serviceWorker.register("/sw.js").catch(() => {});

const rememberedId = localStorage.getItem(ACTIVE_SESSION_KEY);
const remembered = sessions.find((session) => session.id === rememberedId);
if (importedSession || sessions.length > 1) showChooser();
else if (sessions.length === 1) activateSession(sessions[0]);
else if (remembered) activateSession(remembered);
else showChooser();

setInterval(load, 4000);
setInterval(loadSpotify, 1500);
