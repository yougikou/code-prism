import sys
import json
import re

# Java Complexity Analyzer
# Reads JSON from Stdin: { "file_path": "...", "content": "...", "change_type": "..." }
# Writes JSON to Stdout: [{ "value": N, "tags": {...}, "matches": [...] }]

def calculate_complexity(code):
    complexity = 1
    keywords = [
        r'\bif\b', r'\belse\s+if\b', r'\bfor\b', r'\bwhile\b',
        r'\bcase\b', r'\bcatch\b', r'&&', r'\|\|', r'\?'
    ]
    for kw in keywords:
        complexity += len(re.findall(kw, code))
    return complexity


def extract_matches(code, file_path):
    """Return list of match detail dicts for each keyword hit."""
    lines = code.splitlines()
    keywords = [
        r'\bif\b', r'\belse\s+if\b', r'\bfor\b', r'\bwhile\b',
        r'\bcase\b', r'\bcatch\b', r'&&', r'\|\|', r'\?'
    ]
    matches = []
    for kw in keywords:
        for m in re.finditer(kw, code):
            line_number = code[:m.start()].count('\n') + 1
            line_start = code.rfind('\n', 0, m.start())
            if line_start == -1:
                line_start = 0
            else:
                line_start += 1
            column_start = m.start() - line_start + 1
            column_end = column_start + (m.end() - m.start())

            line_idx = line_number - 1
            context_before = lines[line_idx - 1].strip() if line_idx > 0 else None
            context_after = lines[line_idx + 1].strip() if line_idx + 1 < len(lines) else None

            matches.append({
                "file_path": file_path,
                "line_number": line_number,
                "column_start": column_start,
                "column_end": column_end,
                "matched_text": m.group(),
                "context_before": context_before,
                "context_after": context_after,
                "analyzer_id": "",
            })
    return matches


def main():
    # Persistent Loop Mode
    sys.stdin = sys.stdin.detach()
    sys.stdout = sys.stdout.detach()

    while True:
        try:
            line = sys.stdin.readline()
            if not line:
                break

            line_str = line.decode('utf-8').strip()
            if not line_str:
                continue

            data = json.loads(line_str)
            content = data.get("content", "")
            file_path = data.get("file_path", "")
            change_type = data.get("change_type", "")

            value = calculate_complexity(content)
            matches = extract_matches(content, file_path)

            output = [
                {
                    "value": float(value),
                    "tags": {
                        "metric": "complexity",
                        "category": "complexity"
                    },
                    "matches": matches,
                }
            ]

            response = json.dumps(output) + "\n"
            sys.stdout.write(response.encode('utf-8'))
            sys.stdout.flush()

        except Exception as e:
            sys.stderr.write(f"Analyzer Error: {str(e)}\n".encode('utf-8'))
            sys.stdout.write(b"[]\n")
            sys.stdout.flush()


def test():
    print("Running tests for java_complexity...")
    sample_code = """
    public void test() {
        if (a) {
            for (int i=0; i<10; i++) {}
        }
    }
    """
    # Expected: 1 (base) + 1 (if) + 1 (for) = 3
    result = calculate_complexity(sample_code)
    assert result == 3, f"Expected 3, got {result}"
    matches = extract_matches(sample_code, "test.java")
    assert len(matches) == 2, f"Expected 2 matches, got {len(matches)}"
    assert matches[0]["matched_text"] == "if"
    assert matches[1]["matched_text"] == "for"
    print(f"Test passed! {len(matches)} matches extracted.")


if __name__ == "__main__":
    if len(sys.argv) > 1 and sys.argv[1] == "test":
        test()
    else:
        main()
