"""Isi hasil eksekusi (test-results/*.json) ke kolom N-R sheet "Test Cases".

Pakai: python scripts/update-xlsx.py <file.xlsx> [tester] [tanggal]
Hanya kolom Actual Result, Status, Tester, Tanggal Uji, Bug ID / Catatan yang ditulis;
formula Summary, dropdown, dan format lain tidak disentuh.
File JSON diproses urut nama (runId = timestamp), jadi hasil run terbaru menang.
Format: [{ "id": "TC-REC-02", "status": "Pass", "actual": "...", "notes": "...", "evidence": "..." }]
"""
import glob
import json
import sys

import openpyxl

STATUSES = {"Not Run", "Pass", "Fail", "Blocked", "N/A"}

path = sys.argv[1]
tester = sys.argv[2] if len(sys.argv) > 2 else "Automation (Claude Code + PowerShell E2E)"
date = sys.argv[3] if len(sys.argv) > 3 else None

files = sorted(glob.glob("test-results/*.json"))
results = {}
for f in files:
    data = json.load(open(f, encoding="utf-8-sig"))   # PowerShell 5.1 writes a BOM
    for r in data if isinstance(data, list) else [data]:
        if r["status"] not in STATUSES:
            sys.exit(f"{f}: status tidak valid untuk {r['id']}: {r['status']}")
        results[r["id"]] = r

wb = openpyxl.load_workbook(path)
ws = wb["Test Cases"]
seen = set()
for row in ws.iter_rows(min_row=2):
    tc = row[0].value
    if tc not in results:
        continue
    r = results[tc]
    notes = r.get("notes") or (f"Evidence: {r['evidence']}" if r["status"] == "Fail" and r.get("evidence") else None)
    row[13].value = r.get("actual") or None   # N Actual Result
    row[14].value = r["status"]               # O Status
    row[15].value = tester                    # P Tester
    row[16].value = date                      # Q Tanggal Uji
    row[17].value = notes                     # R Bug ID / Catatan
    seen.add(tc)

missing = sorted(set(results) - seen)
wb.calculation.fullCalcOnLoad = True
wb.save(path)
print(f"{len(seen)} TC diperbarui dari {len(files)} file hasil")
if missing:
    print("ID tidak ada di sheet:", ", ".join(missing))
