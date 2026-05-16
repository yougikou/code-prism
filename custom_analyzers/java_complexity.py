import sys
import json
import re

# Simple Java Complexity Analyzer in Python
# Reads JSON from Stdin: { "file_path": "...", "content": "..." }
# Writes JSON to Stdout: [{ "metric_key": "complexity", "value": N, "category": "complexity" }]

def calculate_complexity(code):
    complexity = 1
    # Keywords that increase complexity
    keywords = [
        r'\bif\b', r'\belse\s+if\b', r'\bfor\b', r'\bwhile\b', 
        r'\bcase\b', r'\bcatch\b', r'&&', r'\|\|', r'\?'
    ]
    
    for kw in keywords:
        complexity += len(re.findall(kw, code))
        
    return complexity

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
            
            value = calculate_complexity(content)
            
            output = [
                {
                    "value": float(value),
                    "tags": {
                        "metric": "complexity",
                        "category": "complexity"
                    }
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
    print("Test passed!")

if __name__ == "__main__":
    if len(sys.argv) > 1 and sys.argv[1] == "test":
        test()
    else:
        main()

