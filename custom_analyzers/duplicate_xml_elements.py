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
     → returns {"findings": [...]} with explicit occurrences and metrics.

Thresholds are defined here in the script, not in codeprism.yaml.
"""

import json
import sys
import xml.etree.ElementTree as ET
from _cross_file_protocol import content_group_key, duplicate_finding
from xml.parsers import expat

# ── Script-internal thresholds ──────────────────────────────────────────────
MIN_FILE_COUNT = 2
MIN_BLOCK_COUNT = 3


def get_element_line_ranges(content):
    """Return element start/end line pairs in document preorder.

    ``xml.etree.ElementTree`` elements do not expose ``sourceline`` in the
    Python standard library. Expat provides line information while using the
    same XML parsing rules, and its start-element event order matches
    ``Element.iter()`` preorder.
    """
    parser = expat.ParserCreate()
    ranges = []
    open_elements = []

    def on_start(_name, _attrs):
        index = len(ranges)
        line = parser.CurrentLineNumber
        ranges.append([line, line])
        open_elements.append(index)

    def on_end(_name):
        if open_elements:
            index = open_elements.pop()
            ranges[index][1] = parser.CurrentLineNumber

    parser.StartElementHandler = on_start
    parser.EndElementHandler = on_end
    parser.Parse(content, True)
    return [(start, end) for start, end in ranges]


def get_content_blocks(file_path, content):
    if not file_path.endswith(('.xml', '.xsd', '.pom', '.plist', '.launch', '.svg')):
        return []

    try:
        root = ET.fromstring(content)
        line_ranges = get_element_line_ranges(content)
    except Exception:
        return []

    results = []
    for index, elem in enumerate(root.iter()):
        # Serialize element to canonical XML string
        try:
            elem_str = ET.tostring(elem, encoding='unicode')
        except Exception:
            continue

        line_start, line_end = line_ranges[index] if index < len(line_ranges) else (0, 0)

        results.append({
            'block_size': -1,  # -1 identifies XML element mode
            'line_start': line_start,
            'line_end': line_end,
            'block_content': elem_str,
            'group_key': content_group_key(elem_str),
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

        results.append(duplicate_finding(h, entries))

    return {'findings': results}


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
    assert all(len(block["group_key"]) == 64 for block in blocks)
    # Should find root, item, and name elements
    assert len(blocks) >= 3, f"Expected at least 3 blocks, got {len(blocks)}"
    # The root element serialization should contain the full XML
    root_block = blocks[0]
    assert root_block["block_size"] == -1
    assert (root_block["line_start"], root_block["line_end"]) == (2, 6)
    assert (blocks[1]["line_start"], blocks[1]["line_end"]) == (3, 5)
    assert (blocks[2]["line_start"], blocks[2]["line_end"]) == (4, 4)
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
    assert (blocks[0]["line_start"], blocks[0]["line_end"]) == (1, 1)
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
