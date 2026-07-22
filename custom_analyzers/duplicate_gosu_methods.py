#!/usr/bin/env python3
"""
Duplicate block detection: Gosu method bodies (function/property/construct).

Each Gosu method block's BODY (code between `{` and `}`) is treated as a "block".
The signature is stripped; only the body is compared. Whitespace and comments are
removed for exact-code matching; the original body is preserved for display.

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

METHOD_START_RE = re.compile(
    r'^\s*(?:(?:private|internal|protected|public|static|abstract|override|final|transient|hide)\s+)*'
    r'(?:function\b|construct\b|property\s+(?:get|set)\b)'
)


def strip_gosu_comments(text: str) -> str:
    """Remove Gosu line (//) and block (/* */) comments."""
    text = re.sub(r'//.*', '', text)
    text = re.sub(r'/\*[\s\S]*?\*/', '', text)
    return text


def normalize_body(body: str) -> str:
    """Remove comments + all whitespace for exact-code dedup."""
    body = strip_gosu_comments(body)
    body = re.sub(r'\s+', '', body)
    return body


def get_content_blocks(file_path, content):
    if not file_path.endswith(('.gs', '.gst', '.gsx', '.gsp')):
        return []

    lines = content.splitlines()
    results = []
    i = 0

    while i < len(lines):
        line = lines[i]
        if not METHOD_START_RE.match(line):
            i += 1
            continue

        method_start = i

        # Collect signature lines until we hit `{`
        sig_end = i
        brace_open_found = False
        in_generics = 0
        in_parens = 0
        brace_pos_in_line = -1

        while sig_end < len(lines):
            sig_line = lines[sig_end]

            for j, ch in enumerate(sig_line):
                if ch == '(':
                    in_parens += 1
                elif ch == ')':
                    in_parens = max(0, in_parens - 1)
                if ch == '<':
                    prev_ch = sig_line[j - 1] if j > 0 else ' '
                    if prev_ch.isalnum() or prev_ch in (')', '>', ']'):
                        in_generics += 1
                elif ch == '>':
                    if in_generics > 0:
                        in_generics -= 1

            if in_parens == 0 and in_generics == 0:
                brace_idx = sig_line.find('{')
                if brace_idx >= 0:
                    brace_open_found = True
                    brace_pos_in_line = brace_idx
                    break

            sig_end += 1

        if not brace_open_found:
            i = sig_end + 1
            continue

        # Track brace depth to find matching `}`
        brace_depth = 1
        fn_end = sig_end

        # Check for content after `{` on same line
        sig_line_text = lines[sig_end]
        after_brace = sig_line_text[brace_pos_in_line + 1:].strip() if brace_pos_in_line >= 0 else ''

        if after_brace:
            # Check if body ends on the same line
            for ch in after_brace:
                if ch == '{':
                    brace_depth += 1
                elif ch == '}':
                    brace_depth -= 1
                    if brace_depth == 0:
                        break
            if brace_depth == 0:
                original_body = after_brace[:after_brace.rindex('}')].strip() if '}' in after_brace else after_brace
                normalized = normalize_body(original_body)
                if normalized:
                    results.append({
                        'block_size': -30,
                        'line_start': sig_end + 1,
                        'line_end': sig_end + 1,
                        'block_content': original_body,
                        'group_key': content_group_key(normalized),
                        'normalized_content': normalized,
                    })
                i = fn_end + 1
                continue

        # Multi-line body
        fn_end = sig_end
        while fn_end < len(lines):
            m_line = lines[fn_end]
            start_col = 0
            if fn_end == sig_end:
                start_col = brace_pos_in_line + 1

            for ch in m_line[start_col:]:
                if ch == '{':
                    brace_depth += 1
                elif ch == '}':
                    brace_depth -= 1
                    if brace_depth == 0:
                        break
            if brace_depth == 0:
                break
            fn_end += 1

        if brace_depth != 0 and fn_end >= len(lines):
            i = sig_end + 1
            continue

        # Collect body lines (from line after opening brace to line before closing brace)
        body_start_line = sig_end + 1
        body_lines = []
        for ln in range(body_start_line, fn_end):
            body_lines.append(lines[ln])

        if not body_lines and not after_brace:
            i = fn_end + 1
            continue

        original_body = '\n'.join(body_lines).strip() if body_lines else after_brace
        if after_brace and body_lines:
            original_body = after_brace + '\n' + original_body

        normalized = normalize_body(original_body)

        if not normalized:
            i = fn_end + 1
            continue

        results.append({
            'block_size': -30,
            'line_start': body_start_line + 1,
            'line_end': fn_end,
            'block_content': original_body,
            'group_key': content_group_key(normalized),
            'normalized_content': normalized,
        })

        i = fn_end + 1

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
    print("Running tests for duplicate_gosu_methods...")

    # Test 1: Simple function — body only
    code = """
function hello(): String {
    return "Hello"
}
"""
    blocks = get_content_blocks("test.gs", code)
    assert len(blocks) == 1, f"Expected 1, got {len(blocks)}"
    assert "function hello()" not in blocks[0]["block_content"]
    assert 'return "Hello"' in blocks[0]["block_content"]
    assert blocks[0]["block_size"] == -30
    print("  Test 1 (simple body) passed")

    # Test 2: Property get/set — bodies only
    code = """
class MyClass {
    property get Name(): String {
        return _name
    }
    property set Name(value: String) {
        _name = value
    }
}
"""
    blocks = get_content_blocks("test.gs", code)
    assert len(blocks) == 2, f"Expected 2, got {len(blocks)}"
    assert "property get" not in blocks[0]["block_content"]
    assert "property set" not in blocks[1]["block_content"]
    print("  Test 2 (property bodies) passed")

    # Test 3: Construct
    code = """
construct(name: String, age: int) {
    _name = name
    _age = age
}
"""
    blocks = get_content_blocks("test.gs", code)
    assert len(blocks) == 1, f"Expected 1, got {len(blocks)}"
    assert "construct(" not in blocks[0]["block_content"]
    assert "_name = name" in blocks[0]["block_content"]
    print("  Test 3 (construct body) passed")

    # Test 4: Normalization — identical body, different modifiers
    code1 = "private static function foo(): String {\n    return \"x\"\n}\n"
    code2 = "public function foo(): String {\n    return \"x\"\n}\n"
    b1 = get_content_blocks("test.gs", code1)
    b2 = get_content_blocks("test.gs", code2)
    assert b1 and b2
    assert b1[0]["normalized_content"] == b2[0]["normalized_content"]
    assert len(b1[0]["group_key"]) == 64
    assert '"x"' in b1[0]["block_content"]
    print("  Test 4 (normalization) passed")

    # Test 5: Non-Gosu file
    assert get_content_blocks("test.py", "") == []
    print("  Test 5 (non-Gosu) passed")

    # Test 6: Empty
    assert get_content_blocks("test.gs", "") == []
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
