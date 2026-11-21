CREATE TABLE IF NOT EXISTS projects (
  id TEXT PRIMARY KEY NOT NULL,
  display_name TEXT NOT NULL,
  primary_root TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS worktrees (
  id TEXT PRIMARY KEY NOT NULL,
  project_id TEXT NOT NULL REFERENCES projects(id),
  path TEXT NOT NULL,
  app_relative TEXT NOT NULL,
  branch TEXT
);

CREATE TABLE IF NOT EXISTS commits (
  id TEXT PRIMARY KEY NOT NULL,
  project_id TEXT NOT NULL REFERENCES projects(id),
  git_sha TEXT,
  dirty INTEGER NOT NULL CHECK (dirty IN (0, 1)),
  tree_digest TEXT
);

CREATE TABLE IF NOT EXISTS runs (
  id TEXT PRIMARY KEY NOT NULL,
  project_id TEXT NOT NULL REFERENCES projects(id),
  worktree_id TEXT NOT NULL REFERENCES worktrees(id),
  commit_id TEXT NOT NULL REFERENCES commits(id),
  kind_json TEXT NOT NULL,
  created_at_ms INTEGER NOT NULL,
  environment_json TEXT NOT NULL,
  source_files_json TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS screens (
  id TEXT PRIMARY KEY NOT NULL,
  path TEXT NOT NULL,
  origin_json TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS states (
  id TEXT PRIMARY KEY NOT NULL,
  screen_id TEXT NOT NULL REFERENCES screens(id),
  scenario TEXT NOT NULL,
  locale TEXT,
  inputs_json TEXT NOT NULL,
  plan_index INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS run_screens (
  run_id TEXT NOT NULL REFERENCES runs(id),
  screen_id TEXT NOT NULL REFERENCES screens(id),
  PRIMARY KEY (run_id, screen_id)
);

CREATE TABLE IF NOT EXISTS run_states (
  run_id TEXT NOT NULL REFERENCES runs(id),
  state_id TEXT NOT NULL REFERENCES states(id),
  PRIMARY KEY (run_id, state_id)
);

CREATE TABLE IF NOT EXISTS image_blobs (
  id TEXT PRIMARY KEY NOT NULL,
  media_type TEXT NOT NULL,
  byte_len INTEGER NOT NULL,
  width_px INTEGER NOT NULL,
  height_px INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS evidence (
  id TEXT PRIMARY KEY NOT NULL,
  payload_json TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS cells (
  id TEXT PRIMARY KEY NOT NULL,
  run_id TEXT NOT NULL REFERENCES runs(id),
  screen_id TEXT NOT NULL REFERENCES screens(id),
  state_id TEXT NOT NULL REFERENCES states(id),
  width_px INTEGER NOT NULL CHECK (width_px > 0),
  locale TEXT,
  status_json TEXT NOT NULL,
  image_blob_id TEXT REFERENCES image_blobs(id),
  evidence_id TEXT REFERENCES evidence(id),
  cache_json TEXT
);

CREATE TABLE IF NOT EXISTS edges (
  id TEXT PRIMARY KEY NOT NULL,
  run_id TEXT NOT NULL REFERENCES runs(id),
  from_json TEXT NOT NULL,
  to_json TEXT NOT NULL,
  source TEXT NOT NULL,
  trigger TEXT,
  selector TEXT,
  observed_widths_json TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS cache_entries (
  cache_key TEXT PRIMARY KEY NOT NULL,
  inputs_json TEXT NOT NULL,
  status_json TEXT NOT NULL,
  image_blob_id TEXT REFERENCES image_blobs(id),
  evidence_id TEXT REFERENCES evidence(id),
  created_at_ms INTEGER NOT NULL,
  last_hit_at_ms INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS comments (
  id TEXT PRIMARY KEY NOT NULL,
  project_id TEXT NOT NULL REFERENCES projects(id),
  run_id TEXT NOT NULL REFERENCES runs(id),
  target_json TEXT NOT NULL,
  author TEXT NOT NULL,
  body TEXT NOT NULL,
  status TEXT NOT NULL,
  created_at_ms INTEGER NOT NULL,
  updated_at_ms INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_runs_project_created ON runs (project_id, created_at_ms DESC);
CREATE INDEX IF NOT EXISTS idx_worktrees_project_branch ON worktrees (project_id, branch);
CREATE INDEX IF NOT EXISTS idx_cells_run ON cells (run_id);
CREATE INDEX IF NOT EXISTS idx_cells_blob ON cells (image_blob_id);
CREATE INDEX IF NOT EXISTS idx_edges_run ON edges (run_id);
CREATE INDEX IF NOT EXISTS idx_cache_blob ON cache_entries (image_blob_id);
CREATE INDEX IF NOT EXISTS idx_comments_run ON comments (run_id, created_at_ms);
