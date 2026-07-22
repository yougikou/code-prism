#!/usr/bin/env python3
"""
Duplicate block detection: Rust function/method bodies.

Each Rust fn block's BODY (code between `{` and `}`) is treated as a "block".
Whitespace and comments are stripped for exact-code matching; the original body
is preserved for display.

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
# These replace the old min_file_count / min_block_count YAML settings.
# A block hash must appear in at least MIN_FILE_COUNT different files AND
# have at least MIN_BLOCK_COUNT total occurrences to be reported as a duplicate.
MIN_FILE_COUNT = 3
MIN_BLOCK_COUNT = 3

FN_START_RE = re.compile(
    r'^\s*(?:pub\s+(?:crate\s+)?|pub\s+|async\s+|unsafe\s+)*fn\s+\w+'
)


def strip_rust_comments(text: str) -> str:
    """Remove Rust line comments (//) and block comments (/* */)."""
    text = re.sub(r'//.*', '', text)
    text = re.sub(r'/\*[\s\S]*?\*/', '', text)
    return text


def normalize_body(body: str) -> str:
    """Remove comments + all whitespace for exact-code dedup."""
    body = strip_rust_comments(body)
    body = re.sub(r'\s+', '', body)
    return body


def get_content_blocks(file_path, content):
    if not file_path.endswith(('.rs')):
        return []

    lines = content.splitlines()
    results = []
    i = 0

    while i < len(lines):
        line = lines[i]
        if not FN_START_RE.match(line):
            i += 1
            continue

        fn_start = i

        # Collect signature lines until we hit `{`
        sig_end = i
        brace_open_found = False
        in_generics = 0
        in_parens = 0
        brace_pos_in_line = -1  # column of the opening `{`

        while sig_end < len(lines):
            sig_line = lines[sig_end]

            for ch in sig_line:
                if ch == '<':
                    in_generics += 1
                elif ch == '>':
                    in_generics = max(0, in_generics - 1)
                elif ch == '(':
                    in_parens += 1
                elif ch == ')':
                    in_parens = max(0, in_parens - 1)

            if in_generics == 0 and in_parens == 0:
                brace_idx = sig_line.find('{')
                if brace_idx >= 0:
                    brace_open_found = True
                    brace_pos_in_line = brace_idx
                    break

            sig_end += 1

        if not brace_open_found:
            i = sig_end + 1
            continue

        # Now track brace depth to find matching `}`
        brace_depth = 1  # we found the opening `{`
        body_started = False
        fn_end = sig_end

        # If there's code before `{` on the same line as `{`
        # Check if there's content after `{` on that line
        sig_line = lines[sig_end]
        content_after_brace = sig_line[brace_pos_in_line + 1:].strip() if brace_pos_in_line >= 0 else ''
        if content_after_brace:
            # There's content on the same line as `{` (e.g. `fn foo() { let x = 1; }`)
            # We need to count braces in the remaining part
            for ch in content_after_brace:
                if ch == '{':
                    brace_depth += 1
                elif ch == '}':
                    brace_depth -= 1
                    if brace_depth == 0:
                        body_started = True
                        break
            if brace_depth == 0:
                # Body ends on same line
                block_body_line = content_after_brace[:content_after_brace.rindex('}')] if '}' in content_after_brace else content_after_brace
                original_body = block_body_line.strip()
                normalized = normalize_body(original_body)
                if normalized:
                    results.append({
                        'block_size': -10,
                        'line_start': sig_end + 1,
                        'line_end': sig_end + 1,
                        'block_content': original_body,
                        'group_key': content_group_key(normalized),
                        'normalized_content': normalized,
                    })
                i = fn_end + 1
                continue

        # Multi-line body: track from line after `{`
        fn_end = sig_end
        while fn_end < len(lines):
            fn_line = lines[fn_end]
            if fn_end == sig_end:
                # Start from after the opening `{`
                remainder = fn_line[brace_pos_in_line + 1:] if brace_pos_in_line >= 0 else fn_line
                for ch in remainder:
                    if ch == '{':
                        brace_depth += 1
                    elif ch == '}':
                        brace_depth -= 1
                        if brace_depth == 0:
                            break
                if brace_depth == 0:
                    break
            else:
                for ch in fn_line:
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

        # Body lines: from line after opening brace to line before closing brace
        body_start_line = sig_end
        if brace_pos_in_line >= 0 and not lines[sig_end][brace_pos_in_line + 1:].strip():
            # `{` is the last thing on its line — body starts next line
            body_start_line = sig_end + 1
        else:
            # Content after `{` on same line — body starts on this line
            # but we need to handle carefully
            pass

        # Collect body lines
        body_lines = []
        for ln in range(body_start_line, fn_end):
            body_lines.append(lines[ln])

        # Also include content after `{` on the sig_end line if any
        if brace_pos_in_line >= 0:
            after_brace = lines[sig_end][brace_pos_in_line + 1:].strip()
            if after_brace:
                body_lines.insert(0, after_brace)

        if not body_lines:
            i = fn_end + 1
            continue

        original_body = '\n'.join(body_lines).strip()
        normalized = normalize_body(original_body)

        if not normalized:
            i = fn_end + 1
            continue

        results.append({
            'block_size': -10,
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

    Called once after all files have been scanned. The blocks list contains
    every block extracted across all files for this analyzer. The script groups
    them by ``group_key`` (block_hash), checks against its own thresholds, and
    returns only the groups that qualify as true duplicates.
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
    print("Running tests for duplicate_rust_fns...")

    # Test 1: Simple function — body only
    code = """
fn foo() {
    let x = 1;
}
"""
    blocks = get_content_blocks("test.rs", code)
    assert len(blocks) == 1, f"Expected 1, got {len(blocks)}"
    # Body should be dedented "let x = 1;"
    assert "fn foo()" not in blocks[0]["block_content"], f"Signature leaked: {blocks[0]['block_content']!r}"
    assert "let x = 1;" in blocks[0]["block_content"]
    assert blocks[0]["normalized_content"] == "letx=1;"
    assert len(blocks[0]["group_key"]) == 64
    assert blocks[0]["block_size"] == -10
    print("  Test 1 (simple body) passed")

    # Test 2: Pub async unsafe with generics — body only
    code = """
pub async unsafe fn process<T: Debug>(input: T) -> Result<()>
where
    T: Send,
{
    do_work(input).await
}
"""
    blocks = get_content_blocks("test.rs", code)
    assert len(blocks) == 1, f"Expected 1, got {len(blocks)}"
    assert "pub async unsafe fn" not in blocks[0]["block_content"]
    assert "do_work(input).await" in blocks[0]["block_content"]
    print("  Test 2 (body only, no sig) passed")

    # Test 3: Multiple functions
    code = """
fn first() {
    let a = 1;
}
fn second() {
    let b = 2;
}
"""
    blocks = get_content_blocks("test.rs", code)
    assert len(blocks) == 2, f"Expected 2, got {len(blocks)}"
    assert "first" not in blocks[0]["block_content"]
    assert "second" not in blocks[1]["block_content"]
    assert blocks[0]["line_end"] < blocks[1]["line_start"]
    print("  Test 3 (multiple bodies) passed")

    # Test 4: Normalization — same body, different comments
    code1 = "fn foo() {\n    return 1;\n}\n"
    code2 = "fn bar() {\n    // comment\n    return 1;\n}\n"
    code3 = "fn baz() {\n    return  1;\n}\n"
    b1 = get_content_blocks("test.rs", code1)
    b2 = get_content_blocks("test.rs", code2)
    b3 = get_content_blocks("test.rs", code3)
    assert b1 and b2 and b3
    assert b1[0]["normalized_content"] == b2[0]["normalized_content"] == b3[0]["normalized_content"]
    assert b1[0]["normalized_content"] == "return1;"
    print("  Test 4 (normalization) passed")

    # Test 5: Non-Rust file
    assert get_content_blocks("test.py", "") == []
    print("  Test 5 (non-Rust) passed")

    # Test 6: Empty
    assert get_content_blocks("test.rs", "") == []
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
