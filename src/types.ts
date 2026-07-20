export type LaunchKind = "application" | "url";
export type UnreadProvider = "teams" | "whatsapp" | null;
export type ThemeMode = "light" | "dark";
export type GridSize = 3 | 4 | 5;

export interface DeckButton {
  id: string;
  label: string;
  target: string;
  kind: LaunchKind;
  color: string;
  icon: string | null;
  showLabel: boolean;
  transparentBackground: boolean;
  unreadProvider: UnreadProvider;
}

export interface DeckConfig {
  version: number;
  title: string;
  theme: ThemeMode;
  gridSize: GridSize;
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
