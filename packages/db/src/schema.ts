// Portable Drizzle SQLite schema for the dui domain model (#9, #10).
// Every column uses text IDs/hashes, integer Unix epoch ms timestamps, and JSON text
// so a future Postgres dialect maps 1:1 without schema redesign.
import { index, integer, primaryKey, sqliteTable, text } from "drizzle-orm/sqlite-core";

export const duiMigrations = sqliteTable("dui_migrations", {
  tag: text("tag").primaryKey(),
  appliedAtMs: integer("applied_at_ms").notNull(),
});

export const projects = sqliteTable("projects", {
  id: text("id").primaryKey(),
  displayName: text("display_name").notNull(),
  primaryRoot: text("primary_root").notNull(),
});

export const worktrees = sqliteTable(
  "worktrees",
  {
    id: text("id").primaryKey(),
    projectId: text("project_id")
      .notNull()
      .references(() => projects.id),
    path: text("path").notNull(),
    appRelative: text("app_relative").notNull(),
    branch: text("branch"),
  },
  (table) => [index("idx_worktrees_project_branch").on(table.projectId, table.branch)],
);

export const commits = sqliteTable("commits", {
  id: text("id").primaryKey(),
  projectId: text("project_id")
    .notNull()
    .references(() => projects.id),
  gitSha: text("git_sha"),
  dirty: integer("dirty", { mode: "boolean" }).notNull(),
  treeDigest: text("tree_digest"),
});

export const runs = sqliteTable(
  "runs",
  {
    id: text("id").primaryKey(),
    projectId: text("project_id")
      .notNull()
      .references(() => projects.id),
    worktreeId: text("worktree_id")
      .notNull()
      .references(() => worktrees.id),
    commitId: text("commit_id")
      .notNull()
      .references(() => commits.id),
    kindJson: text("kind_json", { mode: "json" }).$type<Record<string, unknown>>().notNull(),
    createdAtMs: integer("created_at_ms").notNull(),
    environmentJson: text("environment_json", { mode: "json" })
      .$type<Record<string, unknown>>()
      .notNull(),
    sourceFilesJson: text("source_files_json", { mode: "json" }).$type<string[]>().notNull(),
  },
  (table) => [index("idx_runs_project_created").on(table.projectId, table.createdAtMs)],
);

export const screens = sqliteTable("screens", {
  id: text("id").primaryKey(),
  path: text("path").notNull(),
  originJson: text("origin_json", { mode: "json" }).$type<Record<string, unknown>>().notNull(),
});

export const states = sqliteTable("states", {
  id: text("id").primaryKey(),
  screenId: text("screen_id")
    .notNull()
    .references(() => screens.id),
  scenario: text("scenario").notNull(),
  locale: text("locale"),
  inputsJson: text("inputs_json", { mode: "json" }).$type<Record<string, unknown>>().notNull(),
  planIndex: integer("plan_index").notNull(),
});

export const runScreens = sqliteTable(
  "run_screens",
  {
    runId: text("run_id")
      .notNull()
      .references(() => runs.id),
    screenId: text("screen_id")
      .notNull()
      .references(() => screens.id),
  },
  (table) => [primaryKey({ columns: [table.runId, table.screenId] })],
);

export const runStates = sqliteTable(
  "run_states",
  {
    runId: text("run_id")
      .notNull()
      .references(() => runs.id),
    stateId: text("state_id")
      .notNull()
      .references(() => states.id),
  },
  (table) => [primaryKey({ columns: [table.runId, table.stateId] })],
);

export const imageBlobs = sqliteTable("image_blobs", {
  id: text("id").primaryKey(),
  mediaType: text("media_type").notNull(),
  byteLen: integer("byte_len").notNull(),
  widthPx: integer("width_px").notNull(),
  heightPx: integer("height_px").notNull(),
});

export const evidence = sqliteTable("evidence", {
  id: text("id").primaryKey(),
  payloadJson: text("payload_json", { mode: "json" }).$type<Record<string, unknown>>().notNull(),
});

export const cells = sqliteTable(
  "cells",
  {
    id: text("id").primaryKey(),
    runId: text("run_id")
      .notNull()
      .references(() => runs.id),
    screenId: text("screen_id")
      .notNull()
      .references(() => screens.id),
    stateId: text("state_id")
      .notNull()
      .references(() => states.id),
    widthPx: integer("width_px").notNull(),
    locale: text("locale"),
    statusJson: text("status_json", { mode: "json" }).$type<Record<string, unknown>>().notNull(),
    imageBlobId: text("image_blob_id").references(() => imageBlobs.id),
    evidenceId: text("evidence_id").references(() => evidence.id),
    cacheJson: text("cache_json", { mode: "json" }).$type<Record<string, unknown> | null>(),
  },
  (table) => [
    index("idx_cells_run").on(table.runId),
    index("idx_cells_blob").on(table.imageBlobId),
  ],
);

export const edges = sqliteTable(
  "edges",
  {
    id: text("id").primaryKey(),
    runId: text("run_id")
      .notNull()
      .references(() => runs.id),
    fromJson: text("from_json", { mode: "json" }).$type<Record<string, unknown>>().notNull(),
    toJson: text("to_json", { mode: "json" }).$type<Record<string, unknown>>().notNull(),
    source: text("source").notNull(),
    trigger: text("trigger"),
    selector: text("selector"),
    observedWidthsJson: text("observed_widths_json", { mode: "json" }).$type<number[]>().notNull(),
  },
  (table) => [index("idx_edges_run").on(table.runId)],
);

export const cacheEntries = sqliteTable(
  "cache_entries",
  {
    cacheKey: text("cache_key").primaryKey(),
    inputsJson: text("inputs_json", { mode: "json" }).$type<Record<string, unknown>>().notNull(),
    statusJson: text("status_json", { mode: "json" }).$type<Record<string, unknown>>().notNull(),
    imageBlobId: text("image_blob_id").references(() => imageBlobs.id),
    evidenceId: text("evidence_id").references(() => evidence.id),
    createdAtMs: integer("created_at_ms").notNull(),
    lastHitAtMs: integer("last_hit_at_ms").notNull(),
  },
  (table) => [index("idx_cache_blob").on(table.imageBlobId)],
);

export const comments = sqliteTable(
  "comments",
  {
    id: text("id").primaryKey(),
    projectId: text("project_id")
      .notNull()
      .references(() => projects.id),
    runId: text("run_id")
      .notNull()
      .references(() => runs.id),
    targetJson: text("target_json", { mode: "json" }).$type<Record<string, unknown>>().notNull(),
    author: text("author").notNull(),
    body: text("body").notNull(),
    status: text("status").notNull(),
    createdAtMs: integer("created_at_ms").notNull(),
    updatedAtMs: integer("updated_at_ms").notNull(),
  },
  (table) => [index("idx_comments_run").on(table.runId, table.createdAtMs)],
);

export const schema = {
  duiMigrations,
  projects,
  worktrees,
  commits,
  runs,
  screens,
  states,
  runScreens,
  runStates,
  imageBlobs,
  evidence,
  cells,
  edges,
  cacheEntries,
  comments,
};
