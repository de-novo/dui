export type Run = {
  key: string;
  projectId: string;
  project: string;
  branch: string | null;
  version: string;
  diffWith?: string[];
  responsiveAvailable?: boolean;
  liveAvailable?: boolean;
};
