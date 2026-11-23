export type Run = {
  key: string;
  projectId: string;
  project: string;
  branch: string | null;
  version: string;
  liveAvailable?: boolean;
  liveBaseUrl?: string;
  liveBridgePath?: string;
  liveDefaultMode?: "capture" | "live";
};
