#!/usr/bin/env python3
"""Разбор остаточных wrong_argument_count после v0.3.3 (run_id>9, ~199 шт):
группировка по методам + реальный код строк. Цель — отделить реальные ошибки
от оставшихся false-positive."""
import os
import psycopg

DSN = os.environ.get("BSL_STAT_DSN",
                     "host=127.0.0.1 port=5432 dbname=rag user=claude_memory_rw")
ROOTS = {
    "ut": "/home/rag/data/bsl_local/RepoUT",
    "bp-ss": "/home/rag/data/bsl_local/RepoBP_SS",
    "bp-tdk": "/home/rag/data/bsl_local/RepoBP_TDK",
    "zup": "/home/rag/data/bsl_local/RepoZUP",
}
METHOD_RE = r"Метод '([^']+)'"


def code_line(alias, path, line):
    full = os.path.join(ROOTS[alias], path)
    try:
        with open(full, "r", encoding="utf-8", errors="replace") as fh:
            lines = fh.read().splitlines()
        if line and 1 <= line <= len(lines):
            lo, hi = max(0, line - 1), min(len(lines), line + 1)
            return " ¦ ".join(l.strip() for l in lines[lo:hi])
    except Exception as e:
        return f"<{e}>"
    return "<нет строки>"


with psycopg.connect(DSN) as c, c.cursor() as cur:
    cur.execute(
        """
        SELECT substring(message from %s) AS method, count(*) cnt
        FROM bsl_validation_findings
        WHERE kind='wrong_argument_count' AND run_id>9
        GROUP BY 1 ORDER BY 2 DESC
        """, (METHOD_RE,))
    groups = cur.fetchall()
    print(f"ОСТАТОК wrong_argument_count после v0.3.3: {sum(g[1] for g in groups)} шт, {len(groups)} методов")
    print("=" * 90)
    for method, cnt in groups:
        print(f"\n### {method}  ({cnt} шт)")
        cur.execute(
            """
            SELECT config_alias, module_path, line, message
            FROM bsl_validation_findings
            WHERE kind='wrong_argument_count' AND run_id>9
              AND message LIKE %s
            LIMIT 3
            """, (f"%'{method}'%",))
        for alias, path, line, msg in cur.fetchall():
            print(f"  msg: {msg[:75]}")
            print(f"  [{alias}] {path.split('/')[-3] if path.count('/')>=2 else path}:{line}")
            print(f"  код: {code_line(alias, path, line)[:160]}")
