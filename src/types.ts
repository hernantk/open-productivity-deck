export type LaunchKind = "application" | "url";
export type UnreadProvider = "teams" | "whatsapp" | null;

export interface DeckButton {
  id: string;
  label: string;
  target: string;
  kind: LaunchKind;
  color: string;
  icon: string | null;
  showLabel: boolean;
  unreadProvider: UnreadProvider;
}

export interface DeckConfig {
  version: number;
  title: string;
  buttons: DeckButton[];
}

export interface ServiceSettings {
  port: number;
  securePort: number;
}

export interface AudioState {
  volume: number;
  muted: boolean;
  microphoneMuted: boolean | null;
}

export interface DashboardState {
  config: DeckConfig;
  audio: AudioState | null;
  pairingUrl: string;
  localAddress: string;
  port: number;
  securePort: number;
  serviceSettings: ServiceSettings;
  deviceId: string;
  deviceName: string;
  unread: Record<string, number | null>;
}
