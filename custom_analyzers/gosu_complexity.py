import sys
import json
import re

# Gosu Complexity Analyzer
# Reads JSON from Stdin: { "file_path": "...", "content": "..." }
# Writes JSON to Stdout: [{ "metric_key": "complexity", "value": N, "category": "complexity" }]

def calculate_complexity(code):
    complexity = 1
    
    # Gosu Keywords and Operators that increase complexity
    # Based on grammar provided
    # Branching: if, else, for, foreach, while, do, case, catch
    # Boolean logic: &&, ||, and, or
    # Ternary/Elvis: ?, ?:
    
    # Regex patterns
    # We use word boundaries \b for keywords
    
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
        r'\?',  # Matches ternary ? and ?: (partially, but sufficient for count if not overlapping)
    ]
    
    # Note: '?' matches both ternary and elvis '?:'. 
    # If we want to be strict, we might count them separately or ensuring we don't double count.
    # regex matches are non-overlapping usually if we just scan, but '?:' contains '?'.
    # If we search for '?' it finds the one in '?:'. That equates to 1 complexity point which is generally correct for Elvis too.
    
    for pat in patterns:
        complexity += len(re.findall(pat, code))
        
    return complexity

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
    
    # Allow some flexibility in 'else' counting depending on interpretation, 
    # but based on my code:
    # Patterns: if, else, for, foreach, while, case, catch, and, or, &&, ||, ?
    # if -> 1
    # and -> 1
    # else -> 1
    # ? -> 1
    # foreach -> 1
    # case -> 1
    # case -> 1
    # Total = 1 (base) + 7 = 8.
    
    assert result == 8, f"Expected 8, got {result}"
    print("Test passed!")

def main():
    if len(sys.argv) > 1 and sys.argv[1] == "test":
        test()
        return

    # Persistent Loop Mode
    # Read line-by-line. Each line is a separate JSON request.
    sys.stdin = sys.stdin.detach() # Binary mode for robust UTF-8 reading
    sys.stdout = sys.stdout.detach() # Binary mode for output

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
            
            # Calculate
            value = calculate_complexity(content)
            
            # Output
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
            # Log error but try to keep alive or return empty result for that line
            # We write an empty JSON array to satisfy the protocol for this request
            err_msg = [
               # Optional: You could return an error metric or just empty
            ]
            sys.stderr.write(f"Analyzer Error: {str(e)}\n".encode('utf-8'))
            sys.stdout.write(b"[]\n")
            sys.stdout.flush()

if __name__ == "__main__":
    main()
