#!/usr/bin/env python3
"""Прогон валидатора bsl-context по .bsl-модулям конфигураций 1С и запись
находок в PostgreSQL (таблицы bsl_validation_runs + bsl_validation_findings).

Назначение — собирать статистику работы валidатора (Уровень 2) на реальном
коде для решения о Phase 8.5 (карточка #1056). Запускается на ВМ rag по cron.

Источник модулей: /home/rag/data/bsl_local/Repo* (исходники конфигураций).
Валидатор: локальный bsl-context на 127.0.0.1:8007 (MCP, stateless HTTP).
Хранилище: PostgreSQL на 127.0.0.1:5432 (пользователь claude_memory_rw).

Примеры:
  python3 bsl_stat_run.py --configs ut --limit 50      # пилот по УТ, 50 модулей
  python3 bsl_stat_run.py --configs all                # полный baseline по всем 4
  python3 bsl_stat_run.py --configs ut,zup --workers 16
"""
import argparse
import json
import os
import sys
import time
import urllib.request
import urllib.error
from concurrent.futures import ThreadPoolExecutor, as_completed

import psycopg

REPOS = {
    "ut": "/home/rag/data/bsl_local/RepoUT",
    "bp-ss": "/home/rag/data/bsl_local/RepoBP_SS",
    "bp-tdk": "/home/rag/data/bsl_local/RepoBP_TDK",
    "zup": "/home/rag/data/bsl_local/RepoZUP",
}

MCP_URL = os.environ.get("BSL_MCP_URL", "http://127.0.0.1:8007/mcp")
# Пароль НЕ хранится в коде — берётся libpq из PGPASSWORD или ~/.pgpass.
# Полный DSN при необходимости переопределяется через BSL_STAT_DSN.
PG_DSN = os.environ.get(
    "BSL_STAT_DSN",
    "host=127.0.0.1 port=5432 dbname=rag user=claude_memory_rw",
)
HTTP_HEADERS = {
    "Content-Type": "application/json",
    "Accept": "application/json, text/event-stream",
}


def mcp_validate(source: str, level: int) -> dict | None:
    """Отправить BSL-фрагмент в validate_expression, вернуть {valid, errors:[...]}."""
    body = {
        "jsonrpc": "2.0",
        "id": 1,
        "method": "tools/call",
        "params": {
            "name": "validate_expression",
            "arguments": {"source": source, "level": level},
        },
    }
    data = json.dumps(body).encode("utf-8")
    req = urllib.request.Request(MCP_URL, data=data, headers=HTTP_HEADERS, method="POST")
    with urllib.request.urlopen(req, timeout=60) as r:
        raw = r.read().decode("utf-8", errors="replace")
    # Ответ может быть чистым JSON (json_response) или SSE (data: ...).
    for line in raw.splitlines():
        line = line.strip()
        if line.startswith("data:"):
            line = line[5:].strip()
        if line.startswith("{"):
            try:
                resp = json.loads(line)
                text = resp["result"]["content"][0]["text"]
                return json.loads(text)
            except Exception:
                continue
    return None


def list_bsl(root: str) -> list[str]:
    out = []
    for dirpath, _dirs, files in os.walk(root):
        for f in files:
            if f.lower().endswith(".bsl"):
                out.append(os.path.join(dirpath, f))
    out.sort()
    return out


def validate_one(path: str, root: str, level: int):
    """Вернуть (rel_path, errors|None, error_str|None)."""
    rel = os.path.relpath(path, root)
    try:
        with open(path, "r", encoding="utf-8", errors="replace") as fh:
            source = fh.read()
        if not source.strip():
            return (rel, [], None)
        res = mcp_validate(source, level)
        if res is None:
            return (rel, None, "пустой/некорректный ответ MCP")
        return (rel, res.get("errors", []), None)
    except Exception as e:
        return (rel, None, f"{type(e).__name__}: {e}")


def run_config(conn, alias: str, root: str, level: int, workers: int, limit: int | None):
    if not os.path.isdir(root):
        print(f"[{alias}] каталог не найден: {root}", flush=True)
        return
    modules = list_bsl(root)
    if limit:
        modules = modules[:limit]
    total = len(modules)
    print(f"[{alias}] модулей к обработке: {total} (workers={workers}, level={level})", flush=True)

    with conn.cursor() as cur:
        cur.execute(
            "INSERT INTO bsl_validation_runs (config_alias, validation_level, modules_total, status) "
            "VALUES (%s, %s, %s, 'running') RETURNING run_id",
            (alias, level, total),
        )
        run_id = cur.fetchone()[0]
        conn.commit()

    scanned = 0
    with_errors = 0
    errors_total = 0
    failed = 0
    t0 = time.time()
    batch = []

    def flush_batch():
        if not batch:
            return
        with conn.cursor() as cur:
            cur.executemany(
                "INSERT INTO bsl_validation_findings "
                "(run_id, config_alias, module_path, line, col, kind, confidence, message, suggestion) "
                "VALUES (%s,%s,%s,%s,%s,%s,%s,%s,%s)",
                batch,
            )
        conn.commit()
        batch.clear()

    with ThreadPoolExecutor(max_workers=workers) as ex:
        futs = {ex.submit(validate_one, p, root, level): p for p in modules}
        for fut in as_completed(futs):
            rel, errors, err = fut.result()
            scanned += 1
            if err is not None:
                failed += 1
                if failed <= 10:
                    print(f"[{alias}] FAIL {rel}: {err}", flush=True)
            elif errors:
                with_errors += 1
                errors_total += len(errors)
                for e in errors:
                    batch.append((
                        run_id, alias, rel,
                        e.get("line"), e.get("col"), e.get("kind"),
                        e.get("confidence"), e.get("message"), e.get("suggestion"),
                    ))
                if len(batch) >= 500:
                    flush_batch()
            if scanned % 1000 == 0:
                rate = scanned / max(time.time() - t0, 0.001)
                print(f"[{alias}] {scanned}/{total} | с ошибками: {with_errors} | "
                      f"находок: {errors_total} | fail: {failed} | {rate:.0f} мод/с", flush=True)
                # промежуточное обновление прогресса run
                with conn.cursor() as cur:
                    cur.execute(
                        "UPDATE bsl_validation_runs SET modules_scanned=%s, modules_with_errors=%s, errors_total=%s "
                        "WHERE run_id=%s", (scanned, with_errors, errors_total, run_id))
                conn.commit()

    flush_batch()
    with conn.cursor() as cur:
        cur.execute(
            "UPDATE bsl_validation_runs SET finished_at=now(), modules_scanned=%s, "
            "modules_with_errors=%s, errors_total=%s, status=%s WHERE run_id=%s",
            (scanned, with_errors, errors_total,
             "done" if failed == 0 else f"done_with_{failed}_fails", run_id),
        )
    conn.commit()
    dt = time.time() - t0
    print(f"[{alias}] ГОТОВО run_id={run_id}: {scanned} модулей за {dt:.0f}с | "
          f"с ошибками: {with_errors} | находок: {errors_total} | fail: {failed}", flush=True)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--configs", default="all",
                    help="all или через запятую: ut,bp-ss,bp-tdk,zup")
    ap.add_argument("--level", type=int, default=2)
    ap.add_argument("--workers", type=int, default=12)
    ap.add_argument("--limit", type=int, default=None, help="ограничить число модулей (тест)")
    args = ap.parse_args()

    if args.configs == "all":
        aliases = list(REPOS.keys())
    else:
        aliases = [a.strip() for a in args.configs.split(",") if a.strip()]
    bad = [a for a in aliases if a not in REPOS]
    if bad:
        print(f"неизвестные конфигурации: {bad}; допустимо: {list(REPOS.keys())}", file=sys.stderr)
        sys.exit(2)

    print(f"=== bsl_stat_run | configs={aliases} level={args.level} "
          f"workers={args.workers} limit={args.limit} | {MCP_URL} ===", flush=True)
    with psycopg.connect(PG_DSN, autocommit=False) as conn:
        for alias in aliases:
            run_config(conn, alias, REPOS[alias], args.level, args.workers, args.limit)
    print("=== ВСЕ КОНФИГУРАЦИИ ОБРАБОТАНЫ ===", flush=True)


if __name__ == "__main__":
    main()
