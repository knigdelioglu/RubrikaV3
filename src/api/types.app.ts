export type AppStatus = {
  app_version: string;
  platform: string;
  tauri_ready: boolean;
  rust_backend_ready: boolean;
};

export type MobileConnectionInfo = {
  localHostName: string;
  deviceHost: string;
  webUrl: string;
  apiUrl: string;
  apiEnabled: boolean;
  tokenConfigured: boolean;
};
