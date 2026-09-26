import crypto from "node:crypto";
import { DatabaseSync } from "node:sqlite";
import { RELAY_VERSION } from "./config.mjs";

export const STORAGE_SCHEMA_VERSION = 1;

const isoNow = () => new Date().toISOString();
const opaqueId = prefix => `${prefix}-${crypto.randomUUID()}`;
const json = value => JSON.stringify(value ?? null);

export class RelayStorage {
  constructor(dbPath) {
    this.dbPath = dbPath;
    this.db = new DatabaseSync(dbPath);
    try {
      this.db.exec("PRAGMA journal_mode=WAL; PRAGMA synchronous=FULL; PRAGMA foreign_keys=ON; PRAGMA busy_timeout=3000;");
      this.migrate();
    } catch (error) {
      this.db.close();
      throw error;
    }
  }

  migrate() {
    this.db.exec(`
      CREATE TABLE IF NOT EXISTS schema_migrations (
        version INTEGER PRIMARY KEY,
        applied_at TEXT NOT NULL
      );
    `);
    const row = this.db.prepare("SELECT COALESCE(MAX(version), 0) AS version FROM schema_migrations").get();
    const current = Number(row.version);
    if (current > STORAGE_SCHEMA_VERSION) {
      throw Object.assign(new Error(`Storage schema ${current} is newer than supported ${STORAGE_SCHEMA_VERSION}`), { code: "STORAGE_SCHEMA_FUTURE" });
    }
    if (current < 1) {
      this.db.exec("BEGIN IMMEDIATE");
      try {
        this.db.exec(`
          CREATE TABLE projects (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            root_uri TEXT NOT NULL,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
          );
          CREATE TABLE results (
            id TEXT PRIMARY KEY,
            project_id TEXT REFERENCES projects(id) ON DELETE SET NULL,
            kind TEXT NOT NULL,
            schema_version INTEGER NOT NULL,
            producer_version TEXT NOT NULL,
            payload_json TEXT NOT NULL,
            payload_sha256 TEXT NOT NULL,
            created_at TEXT NOT NULL
          );
          CREATE TABLE jobs (
            id TEXT PRIMARY KEY,
            project_id TEXT REFERENCES projects(id) ON DELETE SET NULL,
            command TEXT NOT NULL,
            state TEXT NOT NULL,
            checkpoint_json TEXT NOT NULL,
            result_id TEXT REFERENCES results(id) ON DELETE SET NULL,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
          );
          CREATE INDEX results_project_created ON results(project_id, created_at);
          CREATE INDEX jobs_project_updated ON jobs(project_id, updated_at);
        `);
        this.db.prepare("INSERT INTO schema_migrations(version, applied_at) VALUES (?, ?)").run(1, isoNow());
        this.db.exec("COMMIT");
      } catch (error) {
        this.db.exec("ROLLBACK");
        throw error;
      }
    }
  }

  integrity() {
    const row = this.db.prepare("PRAGMA quick_check").get();
    const value = Object.values(row)[0];
    return { ok: value === "ok", check: value, schema_version: this.schemaVersion() };
  }

  schemaVersion() {
    const row = this.db.prepare("SELECT COALESCE(MAX(version), 0) AS version FROM schema_migrations").get();
    return Number(row.version);
  }

  registerProject({ id = opaqueId("PRJ"), name, root_uri }) {
    if (!name || !root_uri) throw Object.assign(new Error("name and root_uri are required"), { code: "VALIDATION_FAILED" });
    const now = isoNow();
    this.db.prepare(`
      INSERT INTO projects(id, name, root_uri, created_at, updated_at)
      VALUES (?, ?, ?, ?, ?)
      ON CONFLICT(id) DO UPDATE SET name=excluded.name, root_uri=excluded.root_uri, updated_at=excluded.updated_at
    `).run(id, name, root_uri, now, now);
    return this.getProject(id);
  }

  getProject(id) {
    return this.db.prepare("SELECT * FROM projects WHERE id = ?").get(id) ?? null;
  }

  listProjects() {
    return this.db.prepare("SELECT * FROM projects ORDER BY created_at, id").all();
  }

  putResult({ project_id = null, kind = "GENERIC", payload }) {
    const id = opaqueId("RES");
    const payloadJson = json(payload);
    const hash = crypto.createHash("sha256").update(payloadJson).digest("hex");
    const created = isoNow();
    this.db.prepare(`
      INSERT INTO results(id, project_id, kind, schema_version, producer_version, payload_json, payload_sha256, created_at)
      VALUES (?, ?, ?, ?, ?, ?, ?, ?)
    `).run(id, project_id, kind, STORAGE_SCHEMA_VERSION, RELAY_VERSION, payloadJson, hash, created);
    return this.getResult(id);
  }

  getResult(id) {
    const row = this.db.prepare("SELECT * FROM results WHERE id = ?").get(id);
    if (!row) return null;
    return { ...row, payload: JSON.parse(row.payload_json) };
  }

  checkpointJob({ id = opaqueId("JOB"), project_id = null, command, state, checkpoint = {}, result_id = null }) {
    if (!command || !state) throw Object.assign(new Error("command and state are required"), { code: "VALIDATION_FAILED" });
    const now = isoNow();
    this.db.prepare(`
      INSERT INTO jobs(id, project_id, command, state, checkpoint_json, result_id, created_at, updated_at)
      VALUES (?, ?, ?, ?, ?, ?, ?, ?)
      ON CONFLICT(id) DO UPDATE SET state=excluded.state, checkpoint_json=excluded.checkpoint_json,
        result_id=excluded.result_id, updated_at=excluded.updated_at
    `).run(id, project_id, command, state, json(checkpoint), result_id, now, now);
    return this.getJob(id);
  }

  getJob(id) {
    const row = this.db.prepare("SELECT * FROM jobs WHERE id = ?").get(id);
    return row ? { ...row, checkpoint: JSON.parse(row.checkpoint_json) } : null;
  }

  close() {
    this.db.close();
  }
}
