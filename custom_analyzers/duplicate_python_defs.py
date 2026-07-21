#!/usr/bin/env python3
"""
Duplicate block detection: Python function/class/async def bodies.

Each Python def/class/async def block's BODY (not the signature or decorators)
is treated as a "block". Whitespace and comments are stripped for exact-code
matching; the original body is preserved for display.

Cross-file analyzer protocol (two-phase):
  1. extract  — {"action":"extract","file_path":"...","content":"..."}
     → returns a JSON array of extracted blocks.
  2. finalize — {"action":"finalize","blocks":[...]}
     → returns {"findings": [...]} with explicit occurrences and metrics.

Thresholds are defined here in the script, not in codeprism.yaml.
"""

import json
import sys
import re
from _cross_file_protocol import content_group_key, duplicate_finding

# ── Script-internal thresholds ──────────────────────────────────────────────
MIN_FILE_COUNT = 2
MIN_BLOCK_COUNT = 3

# Lines starting a Python function definition (not class — class bodies
# are just collections of defs and aren't useful duplicate targets).
DEF_START_RE = re.compile(r'^\s*(?:async\s+)?def\s+\w+')


def strip_py_comments(text: str) -> str:
    """Remove Python single-line comments (# ...)."""
    return re.sub(r'#.*', '', text)


def normalize_body(body: str) -> str:
    """Remove comments + all whitespace for exact-code dedup."""
    body = strip_py_comments(body)
    body = re.sub(r'/\*[\s\S]*?\*/', '', body)
    body = re.sub(r'\s+', '', body)
    return body


def get_content_blocks(file_path, content):
    if not file_path.endswith(('.py')):
        return []

    lines = content.splitlines()
    results = []
    i = 0

    while i < len(lines):
        line = lines[i]

        # Skip decorator lines (consume them but don't include in block)
        while i < len(lines) and lines[i].strip().startswith('@'):
            i += 1

        if i >= len(lines):
            break

        line = lines[i]
        if not DEF_START_RE.match(line):
            i += 1
            continue

        def_start = i
        orig_indent = len(line) - len(line.lstrip())

        # Find the end of the signature (need `:` at the end)
        sig_end = i
        in_parens = 0
        while sig_end < len(lines):
            sig_line = lines[sig_end]
            for ch in sig_line:
                if ch == '(':
                    in_parens += 1
                elif ch == ')':
                    in_parens = max(0, in_parens - 1)
            stripped = sig_line.rstrip()
            if in_parens == 0 and stripped.endswith(':'):
                break
            sig_end += 1

        if sig_end >= len(lines):
            i = def_start + 1
            continue

        # Body starts after the signature line
        body_start = sig_end + 1
        body_end = body_start

        while body_end < len(lines):
            body_line = lines[body_end]
            if body_line.strip() == '':
                body_end += 1
                continue
            body_indent = len(body_line) - len(body_line.lstrip())
            if body_indent <= orig_indent:
                break
            body_end += 1

        if body_start >= body_end:
            i = body_end
            continue

        # Build body: strip the common indent
        body_lines = lines[body_start:body_end]
        first_real = next((l for l in body_lines if l.strip()), '')
        common_indent = len(first_real) - len(first_real.lstrip()) if first_real else 0

        stripped_body_lines = []
        for bl in body_lines:
            if bl.strip():
                stripped_body_lines.append(bl[common_indent:] if len(bl) > common_indent else bl)
            else:
                stripped_body_lines.append('')

        original_body = '\n'.join(stripped_body_lines).rstrip('\n')
        normalized = normalize_body(original_body)

        if not normalized:
            i = body_end
            continue

        results.append({
            'block_size': -20,
            'line_start': body_start + 1,
            'line_end': body_end,
            'block_content': original_body,
            'group_key': content_group_key(normalized),
            'normalized_content': normalized,
        })

        i = body_end

    return results


def finalize_blocks(blocks):
    """Group intermediate blocks by hash, apply thresholds, return match groups.
    See duplicate_rust_fns.py for the same logic.
    """
    groups = {}
    for b in blocks:
        h = b.get('group_key')
        if h is None:
            continue
        groups.setdefault(h, []).append(b)

    results = []
    for h, entries in groups.items():
        distinct_files = set(e.get('file_path') for e in entries)
        file_count = len(distinct_files)
        block_count = len(entries)

        if file_count < MIN_FILE_COUNT or block_count < MIN_BLOCK_COUNT:
            continue

        results.append(duplicate_finding(h, entries))

    return {'findings': results}


def test():
    print("Running tests for duplicate_python_defs...")

    # Test 1: Simple function — body only is "pass"
    code = """
def foo():
    pass
"""
    blocks = get_content_blocks("test.py", code)
    assert len(blocks) == 1, f"Expected 1, got {len(blocks)}"
    assert blocks[0]["block_content"].strip() == "pass"
    assert blocks[0]["normalized_content"] == "pass"
    assert len(blocks[0]["group_key"]) == 64
    assert blocks[0]["line_start"] == 3
    assert blocks[0]["line_end"] == 3
    assert blocks[0]["block_size"] == -20
    sample = [{
        "file_path": f"file{i}.py", "group_key": "custom-key", "blob_data": "pass",
        "int_data2": 1, "int_data3": 1, "str_data1": "A", "str_data2": None,
    } for i in range(3)]
    output = finalize_blocks(sample)
    assert output["findings"][0]["finding_key"] == "custom-key"
    assert {m["metric_key"] for m in output["findings"][0]["metrics"]} == {
        "finding_count", "occurrence_count", "affected_file_count", "affected_line_count"
    }
    print("  Test 1 (simple body) passed")

    # Test 2: Class with methods — body of each method
    code = """
class MyClass:
    def method_a(self):
        pass

    def method_b(self):
        return 1
"""
    blocks = get_content_blocks("test.py", code)
    # Gets method_a's "pass" and method_b's "return 1" as separate bodies
    assert len(blocks) == 2, f"Expected 2, got {len(blocks)}: {[b['block_content'][:30] for b in blocks]}"
    print("  Test 2 (methods in class) passed")

    # Test 3: Async with decorator — body only, no signature
    code = """
@decorator
async def fetch_data():
    return await api()
"""
    blocks = get_content_blocks("test.py", code)
    assert len(blocks) == 1, f"Expected 1, got {len(blocks)}"
    assert "async def" not in blocks[0]["block_content"]
    assert "@decorator" not in blocks[0]["block_content"]
    assert "return await api()" in blocks[0]["block_content"]
    print("  Test 3 (body only, no sig/decorator) passed")

    # Test 4: Normalization — same body, different whitespace/comments
    code1 = "def foo():\n    return 1\n"
    code2 = "def bar():\n    # comment\n    return 1\n"
    code3 = "def baz():\n    return  1\n"
    b1 = get_content_blocks("test.py", code1)
    b2 = get_content_blocks("test.py", code2)
    b3 = get_content_blocks("test.py", code3)
    assert b1 and b2 and b3
    assert b1[0]["normalized_content"] == b2[0]["normalized_content"] == b3[0]["normalized_content"]
    assert b1[0]["normalized_content"] == "return1"
    print("  Test 4 (normalization) passed")

    # Test 5: Non-Python file
    assert get_content_blocks("test.rs", code1) == []
    print("  Test 5 (non-Python) passed")

    # Test 6: Empty content
    assert get_content_blocks("test.py", "") == []
    print("  Test 6 (empty) passed")

    print("All tests passed!")


if __name__ == '__main__':
    if len(sys.argv) > 1 and sys.argv[1] == "test":
        test()
    else:
        for line in sys.stdin:
            line = line.strip()
            if not line:
                continue
            try:
                input_data = json.loads(line)
                action = input_data.get('action', '')
                if action == 'extract':
                    blocks = get_content_blocks(input_data['file_path'], input_data['content'])
                    print(json.dumps(blocks, ensure_ascii=False))
                elif action == 'finalize':
                    result = finalize_blocks(input_data.get('blocks', []))
                    print(json.dumps(result, ensure_ascii=False))
                else:
                    print(json.dumps([]), flush=True)
                sys.stdout.flush()
            except Exception:
                print(json.dumps([]), flush=True)
