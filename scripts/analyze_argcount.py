#!/usr/bin/env python3
"""Разовый анализ находок wrong_argument_count: группировка по методам.

Помогает понять, реальные это ошибки или массовый false-positive (см. baseline
2026-05-21 — 918k срабатываний на high-confidence виде, карточка #1232).
"""
import os
import psycopg

DSN = os.environ.get(
    "BSL_STAT_DSN",
    "host=127.0.0.1 port=5432 dbname=rag user=claude_memory_rw",
)
METHOD_RE = r"Метод '([^']+)'"

with psycopg.connect(DSN) as c, c.cursor() as cur:
    # Топ методов по числу срабатываний
    cur.execute(
        """
        SELECT substring(message from %s) AS method,
               count(*) AS cnt,
               count(DISTINCT module_path) AS mods
        FROM bsl_validation_findings
        WHERE kind = 'wrong_argument_count'
        GROUP BY 1 ORDER BY 2 DESC LIMIT 25
        """,
        (METHOD_RE,),
    )
    rows = cur.fetchall()
    print(f"{'МЕТОД':50} {'СРАБ':>9} {'МОДУЛЕЙ':>9}")
    print("-" * 70)
    total = 0
    for m, cnt, mods in rows:
        total += cnt
        print(f"{(m or '?')[:50]:50} {cnt:>9} {mods:>9}")
    print("-" * 70)
    print(f"топ-25 методов суммарно: {total} срабатываний")

    # Сколько РАЗНЫХ методов вообще
    cur.execute(
        "SELECT count(DISTINCT substring(message from %s)) "
        "FROM bsl_validation_findings WHERE kind='wrong_argument_count'",
        (METHOD_RE,),
    )
    print(f"всего различных методов в wrong_argument_count: {cur.fetchone()[0]}")
