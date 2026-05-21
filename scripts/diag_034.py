#!/usr/bin/env python3
"""Диагностика сдвигов 0.3.4 (run_id 14-17) vs 0.3.3 (10-13):
- argcount по методам (ушло ли Число?)
- type_member и new_type: примеры новых/исчезнувших находок."""
import os
import psycopg

DSN = os.environ.get("BSL_STAT_DSN",
                     "host=127.0.0.1 port=5432 dbname=rag user=claude_memory_rw")
MRE = r"Метод '([^']+)'"

with psycopg.connect(DSN) as c, c.cursor() as cur:
    print("=== argcount по методам в 0.3.4 (run 14-17) ===")
    cur.execute("""
        SELECT substring(message from %s) m, count(*) n
        FROM bsl_validation_findings
        WHERE kind='wrong_argument_count' AND run_id BETWEEN 14 AND 17
        GROUP BY 1 ORDER BY 2 DESC LIMIT 12
    """, (MRE,))
    for m, n in cur.fetchall():
        print(f"  {n:>6}  {m}")

    print("\n=== new_type: топ значений в 0.3.3 vs 0.3.4 ===")
    for lo, hi, tag in [(10, 13, "0.3.3"), (14, 17, "0.3.4")]:
        cur.execute("""
            SELECT count(*) FROM bsl_validation_findings
            WHERE kind='unknown_new_type' AND run_id BETWEEN %s AND %s
        """, (lo, hi))
        print(f"  {tag}: new_type = {cur.fetchone()[0]}")
    print("  --- примеры new_type ТОЛЬКО в 0.3.4 (по сообщению) ---")
    cur.execute("""
        SELECT message, count(*) n FROM bsl_validation_findings
        WHERE kind='unknown_new_type' AND run_id BETWEEN 14 AND 17
        GROUP BY 1 ORDER BY 2 DESC LIMIT 8
    """)
    for msg, n in cur.fetchall():
        print(f"    {n:>4}  {msg[:80]}")

    print("\n=== type_member: всего по этапам ===")
    for lo, hi, tag in [(10, 13, "0.3.3"), (14, 17, "0.3.4")]:
        cur.execute("""
            SELECT count(*) FROM bsl_validation_findings
            WHERE kind='unknown_type_member' AND run_id BETWEEN %s AND %s
        """, (lo, hi))
        print(f"  {tag}: type_member = {cur.fetchone()[0]}")
