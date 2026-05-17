import sys
import json
import re

# Gosu Complexity Analyzer
# Reads JSON from Stdin: { "file_path": "...", "content": "...", "change_type": "..." }
# Writes JSON to Stdout: [{ "value": N, "tags": {...}, "matches": [...] }]

def calculate_complexity(code):
    complexity = 1

    # Gosu Keywords and Operators that increase complexity
    patterns = [
        r'\bif\b',
        r'\belse\b',
        r'\bfor\b',
        r'\bforeach\b',
        r'\bwhile\b',
        r'\bcase\b',
        r'\bcatch\b',
        r'\band\b',
        r'\bor\b',
        r'&&',
        r'\|\|',
        r'\?',
    ]

    for pat in patterns:
        complexity += len(re.findall(pat, code))

    return complexity


def extract_matches(code, file_path):
    """Return list of match detail dicts for each keyword/operator hit."""
    lines = code.splitlines()
    patterns = [
        r'\bif\b',
        r'\belse\b',
        r'\bfor\b',
        r'\bforeach\b',
        r'\bwhile\b',
        r'\bcase\b',
        r'\bcatch\b',
        r'\band\b',
        r'\bor\b',
        r'&&',
        r'\|\|',
        r'\?',
    ]
    matches = []
    for pat in patterns:
        for m in re.finditer(pat, code):
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


def test():
    print("Running tests for gosu_complexity...")
    sample_code = """
    class Sample {
        function foo() {
            if (x and y) {
                print("Basic")
            } else {
                return
            }

            var z = a ?: b

            foreach (i in list) {
                switch(i) {
                    case 1: break
                    case 2: break
                }
            }
        }
    }
    """

    # Expected Base: 1
    # if: +1
    # and: +1
    # else: +1
    # ?: (+1) matches '?' pattern
    # foreach: +1
    # case 1: +1
    # case 2: +1
    # Total: 1 + 1 + 1 + 1 + 1 + 1 + 1 + 1 = 8

    result = calculate_complexity(sample_code)
    print(f"Calculated complexity: {result}")

    assert result == 8, f"Expected 8, got {result}"

    matches = extract_matches(sample_code, "sample.gsp")
    print(f"Extracted {len(matches)} matches")
    assert len(matches) == 7, f"Expected 7 matches, got {len(matches)}"

    print("Test passed!")


def main():
    if len(sys.argv) > 1 and sys.argv[1] == "test":
        test()
        return

    # Persistent Loop Mode
    # Read line-by-line. Each line is a separate JSON request.
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

            # Calculate
            value = calculate_complexity(content)
            matches = extract_matches(content, file_path)

            # Output
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


if __name__ == "__main__":
    main()
