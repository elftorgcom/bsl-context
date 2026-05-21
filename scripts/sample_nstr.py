#!/usr/bin/env python3
"""Выборка примеров wrong_argument_count для НСтр: что насчитал валидатор и как
выглядит реальная строка кода (по module_path + line из RepoUT)."""
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

with psycopg.connect(DSN) as c, c.cursor() as cur:
    # Распределение по тексту сообщения (сколько аргументов насчитал)
    cur.execute("""
        SELECT message, count(*) cnt
        FROM bsl_validation_findings
        WHERE kind='wrong_argument_count' AND message LIKE %s
        GROUP BY 1 ORDER BY 2 DESC LIMIT 8
    """, ("%'НСтр'%",))
    print("=== распределение сообщений НСтр ===")
    for msg, cnt in cur.fetchall():
        print(f"{cnt:>8}  {msg[:90]}")

    # 5 конкретных примеров с исходным кодом
    cur.execute("""
        SELECT config_alias, module_path, line, message
        FROM bsl_validation_findings
        WHERE kind='wrong_argument_count' AND message LIKE %s
        LIMIT 5
    """, ("%'НСтр'%",))
    print("\n=== примеры (исходная строка кода) ===")
    for alias, path, line, msg in cur.fetchall():
        full = os.path.join(ROOTS[alias], path)
        src_line = "<файл не прочитан>"
        try:
            with open(full, "r", encoding="utf-8", errors="replace") as fh:
                lines = fh.read().splitlines()
            if line and 1 <= line <= len(lines):
                # строка + соседние для контекста многострочных вызовов
                lo = max(0, line - 1)
                hi = min(len(lines), line + 2)
                src_line = " | ".join(l.strip() for l in lines[lo:hi])
        except Exception as e:
            src_line = f"<ошибка чтения: {e}>"
        print(f"\n[{alias}] {path}:{line}")
        print(f"  msg: {msg[:80]}")
        print(f"  код: {src_line[:200]}")
