#!/usr/bin/env python3
"""
Duplicate XML element detection.

Each complete XML element (including all children) is treated as a "block".
If the same serialized element appears across multiple different XML files,
it is flagged as a duplicate.

Cross-file analyzer protocol (two-phase):
  1. extract  — {"action":"extract","file_path":"...","content":"..."}
     → returns a JSON array of extracted blocks.
  2. finalize — {"action":"finalize","blocks":[...]}
     → returns a JSON array of FinalizeMatchResult groups.

Thresholds are defined here in the script, not in codeprism.yaml.
"""

import json
import sys
import xml.etree.ElementTree as ET

# ── Script-internal thresholds ──────────────────────────────────────────────
MIN_FILE_COUNT = 2
MIN_BLOCK_COUNT = 3


def get_content_blocks(file_path, content):
    if not file_path.endswith(('.xml', '.xsd', '.pom', '.plist', '.launch', '.svg')):
        return []

    try:
        root = ET.fromstring(content)
    except Exception:
        return []

    results = []
    for elem in root.iter():
        # Serialize element to canonical XML string
        try:
            elem_str = ET.tostring(elem, encoding='unicode')
        except Exception:
            continue

        results.append({
            'block_size': -1,  # -1 identifies XML element mode
            'line_start': elem.sourceline or 0,
            'line_end': elem.sourceline or 0,
            'block_content': elem_str,
            'metric_key': 'duplicate_block',
            'category': 'duplication',
        })

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

        first = entries[0]
        results.append({
            'block_hash': h,
            'block_content': first.get('blob_data') or '',
            'block_size': first.get('int_data1') or 0,
            'occurrences': [
                {
                    'file_path': e.get('file_path', ''),
                    'line_start': e.get('int_data2') or 0,
                    'line_end': e.get('int_data3') or 0,
                    'change_type': e.get('str_data1'),
                    'side': e.get('str_data2'),
                }
                for e in entries
            ],
        })

    return results


def test():
    """Tests for duplicate_xml_elements block extraction."""
    print("Running tests for duplicate_xml_elements...")

    # Test 1: Simple XML with nested elements
    xml = """<?xml version="1.0"?>
<root>
    <item id="1">
        <name>foo</name>
    </item>
</root>"""
    blocks = get_content_blocks("test.xml", xml)
    # Should find root, item, and name elements
    assert len(blocks) >= 3, f"Expected at least 3 blocks, got {len(blocks)}"
    # The root element serialization should contain the full XML
    root_block = blocks[0]
    assert root_block["block_size"] == -1
    print("  Test 1 (nested XML elements) passed")

    # Test 2: Non-XML file returns empty
    blocks = get_content_blocks("test.py", xml)
    assert len(blocks) == 0, f"Expected 0 blocks for .py file, got {len(blocks)}"
    print("  Test 2 (non-XML file) passed")

    # Test 3: Malformed XML returns empty
    blocks = get_content_blocks("test.xml", "<root><unclosed>")
    assert len(blocks) == 0, f"Expected 0 blocks for malformed XML, got {len(blocks)}"
    print("  Test 3 (malformed XML) passed")

    # Test 4: Empty content
    blocks = get_content_blocks("test.xml", "")
    assert len(blocks) == 0
    print("  Test 4 (empty content) passed")

    # Test 5: Single root element
    blocks = get_content_blocks("test.xml", "<hello name=\"world\"/>")
    assert len(blocks) == 1, f"Expected 1 block, got {len(blocks)}"
    assert "hello" in blocks[0]["block_content"]
    print("  Test 5 (single root element) passed")

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
