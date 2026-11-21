import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { DatabaseSync } from "node:sqlite";
import { fileURLToPath } from "node:url";
import { desc, eq } from "drizzle-orm";
import { drizzle } from "drizzle-orm/node-sqlite";
import * as schema from "./schema.ts";

export * as schema from "./schema.ts";

const migrationsDir = join(dirname(fileURLToPath(import.meta.url)), "..", "migrations");

type Journal = {
  entries: { idx: number; tag: string }[];
};

export function openDuiDatabase(dbPath: string) {
  const client = new DatabaseSync(dbPath);
  client.exec("PRAGMA journal_mode = WAL;");
  client.exec("PRAGMA synchronous = NORMAL;");
  client.exec("PRAGMA foreign_keys = ON;");
  client.exec("PRAGMA busy_timeout = 10000;");

  const applied = applyMigrations(client);
  const db = drizzle({ client });

  return {
    db,
    client,
    appliedMigrations: applied,
    close() {
      client.close();
    },
    async listHistory() {
      return db
        .select({
          runId: schema.runs.id,
          projectId: schema.runs.projectId,
          projectName: schema.projects.displayName,
          projectRoot: schema.projects.primaryRoot,
          worktreePath: schema.worktrees.path,
          appRelative: schema.worktrees.appRelative,
          branch: schema.worktrees.branch,
          commitSha: schema.commits.gitSha,
          dirty: schema.commits.dirty,
          createdAtMs: schema.runs.createdAtMs,
        })
        .from(schema.runs)
        .innerJoin(schema.projects, eq(schema.runs.projectId, schema.projects.id))
        .innerJoin(schema.worktrees, eq(schema.runs.worktreeId, schema.worktrees.id))
        .innerJoin(schema.commits, eq(schema.runs.commitId, schema.commits.id))
        .orderBy(desc(schema.runs.createdAtMs), desc(schema.runs.id));
    },
  };
}

function applyMigrations(client: DatabaseSync): string[] {
  const journal = JSON.parse(
    readFileSync(join(migrationsDir, "meta", "_journal.json"), "utf8"),
  ) as Journal;
  const applied: string[] = [];

  client.exec("BEGIN IMMEDIATE;");
  try {
    client.exec(
      "CREATE TABLE IF NOT EXISTS dui_migrations (tag TEXT PRIMARY KEY NOT NULL, applied_at_ms INTEGER NOT NULL);",
    );
    const seen = new Set(
      (client.prepare("SELECT tag FROM dui_migrations").all() as { tag: string }[]).map(
        (row) => row.tag,
      ),
    );
    const insert = client.prepare("INSERT INTO dui_migrations (tag, applied_at_ms) VALUES (?, ?);");
    for (const entry of [...journal.entries].sort((a, b) => a.idx - b.idx)) {
      if (seen.has(entry.tag)) continue;
      const sql = readFileSync(join(migrationsDir, `${entry.tag}.sql`), "utf8");
      client.exec(sql);
      insert.run(entry.tag, Date.now());
      applied.push(entry.tag);
    }
    client.exec("COMMIT;");
  } catch (error) {
    client.exec("ROLLBACK;");
    throw error;
  }
  return applied;
}
