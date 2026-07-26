"""Lightweight Apache Camel Java DSL model extraction.

The parser is deliberately dependency-free so CodePrism release bundles can use
it immediately. It is not a Java compiler: it reconstructs fluent Camel chains
while respecting comments, strings, parentheses, and statement boundaries.
The model is kept separate from metric emission so a future JavaParser/JDT
backend can replace this module without changing dashboards.
"""

from __future__ import annotations

import json
import re
from collections import Counter


BRANCH_EIPS = {"choice", "when", "otherwise", "filter", "doTry", "doCatch", "doFinally"}
NESTING_START = {"choice", "filter", "split", "aggregate", "multicast", "doTry", "saga", "loop"}
NESTING_END = {"end", "endChoice", "endDoTry", "endParent"}
METADATA_CALLS = {
    "from",
    "fromF",
    "routeId",
    "description",
    "autoStartup",
    "startupOrder",
    "routePolicy",
    "routePolicyRef",
    "routeConfigurationId",
    "id",
    "end",
    "endChoice",
    "endDoTry",
    "endParent",
}
ENDPOINT_CALLS = {
    "from",
    "fromF",
    "to",
    "toF",
    "toD",
    "enrich",
    "pollEnrich",
    "wireTap",
    "recipientList",
}
INTERNAL_SCHEMES = {"direct", "seda", "vm", "mock", "stub", "controlbus", "dataset"}
TRANSFORMATION_EIPS = {
    "marshal",
    "unmarshal",
    "transform",
    "setBody",
    "convertBodyTo",
    "setHeader",
    "setProperty",
    "removeHeader",
    "removeHeaders",
}
EXPRESSION_EIPS = {"simple", "xpath", "jsonpath", "spel", "groovy", "jq", "language"}
CONCURRENCY_EIPS = {
    "threads",
    "parallelProcessing",
    "executorServiceRef",
    "streaming",
    "synchronous",
    "timeout",
    "throttle",
    "asyncDelayedRedelivery",
}
RESILIENCE_EIPS = {
    "onException",
    "errorHandler",
    "deadLetterChannel",
    "maximumRedeliveries",
    "redeliveryDelay",
    "useExponentialBackOff",
    "idempotentConsumer",
    "transacted",
    "saga",
    "circuitBreaker",
    "onCompletion",
    "completionTimeout",
    "completionSize",
    "aggregationRepository",
    "streamCaching",
}
LOG_LEVELS = ("TRACE", "DEBUG", "INFO", "WARN", "ERROR")

CALL_RE = re.compile(r"(?:^|\.)\s*([A-Za-z_$][\w$]*)\s*\(")
STRING_RE = re.compile(r'"((?:\\.|[^"\\])*)"')
CLASS_RE = re.compile(r"\bclass\s+([A-Za-z_$][\w$]*)")
ROUTE_BUILDER_CLASS_RE = re.compile(
    r"\bclass\s+([A-Za-z_$][\w$]*)[^{};]*\bextends\s+"
    r"(?:[A-Za-z_$][\w$]*\.)*"
    r"(?:(?:[A-Za-z_$][\w$]*)?RouteBuilder|RouteConfigurationBuilder)\b"
)
JAVA_LOG_RE = re.compile(
    r"\b(?:log|logger|LOGGER)\s*\.\s*(trace|debug|info|warn|error)\s*\(",
    re.IGNORECASE,
)
TEST_FILE_RE = re.compile(
    r"(?:Test|Tests|TestCase|IT|ITCase|Spec)\.java$",
    re.IGNORECASE,
)


def is_test_java_file(file_path: str) -> bool:
    """Classify conventional Maven/Gradle unit and integration-test sources."""
    normalized = "/" + file_path.replace("\\", "/").strip("/").lower()
    test_source_markers = (
        "/src/test/",
        "/src/tests/",
        "/src/it/",
        "/src/integrationtest/",
        "/src/integration-test/",
        "/test/",
        "/tests/",
    )
    return any(marker in normalized for marker in test_source_markers) or bool(
        TEST_FILE_RE.search(file_path.rsplit("/", 1)[-1])
    )


def mask_java(text: str, *, keep_strings: bool = True) -> str:
    """Blank comments and optionally strings while preserving offsets/newlines."""
    chars = list(text)
    state = "code"
    escaped = False
    i = 0
    while i < len(chars):
        ch = chars[i]
        nxt = chars[i + 1] if i + 1 < len(chars) else ""
        if state == "code":
            if ch == "/" and nxt == "/":
                chars[i] = chars[i + 1] = " "
                state = "line_comment"
                i += 2
                continue
            if ch == "/" and nxt == "*":
                chars[i] = chars[i + 1] = " "
                state = "block_comment"
                i += 2
                continue
            if ch == '"':
                if not keep_strings:
                    chars[i] = " "
                state = "string"
            elif ch == "'":
                chars[i] = " "
                state = "char"
        elif state == "line_comment":
            if ch == "\n":
                state = "code"
            else:
                chars[i] = " "
        elif state == "block_comment":
            if ch == "*" and nxt == "/":
                chars[i] = chars[i + 1] = " "
                state = "code"
                i += 2
                continue
            if ch != "\n":
                chars[i] = " "
        elif state in {"string", "char"}:
            quote = '"' if state == "string" else "'"
            if not keep_strings or state == "char":
                if ch != "\n":
                    chars[i] = " "
            if escaped:
                escaped = False
            elif ch == "\\":
                escaped = True
            elif ch == quote:
                state = "code"
        i += 1
    return "".join(chars)


def line_number(text: str, offset: int) -> int:
    return text.count("\n", 0, offset) + 1


def find_matching_paren(text: str, open_index: int) -> int:
    depth = 0
    state = "code"
    escaped = False
    for i in range(open_index, len(text)):
        ch = text[i]
        if state == "string":
            if escaped:
                escaped = False
            elif ch == "\\":
                escaped = True
            elif ch == '"':
                state = "code"
            continue
        if state == "char":
            if escaped:
                escaped = False
            elif ch == "\\":
                escaped = True
            elif ch == "'":
                state = "code"
            continue
        if ch == '"':
            state = "string"
        elif ch == "'":
            state = "char"
        elif ch == "(":
            depth += 1
        elif ch == ")":
            depth -= 1
            if depth == 0:
                return i
    return -1


def find_statement_end(text: str, start: int) -> int:
    parens = brackets = braces = 0
    state = "code"
    escaped = False
    for i in range(start, len(text)):
        ch = text[i]
        if state in {"string", "char"}:
            quote = '"' if state == "string" else "'"
            if escaped:
                escaped = False
            elif ch == "\\":
                escaped = True
            elif ch == quote:
                state = "code"
            continue
        if ch == '"':
            state = "string"
        elif ch == "'":
            state = "char"
        elif ch == "(":
            parens += 1
        elif ch == ")":
            parens = max(0, parens - 1)
        elif ch == "[":
            brackets += 1
        elif ch == "]":
            brackets = max(0, brackets - 1)
        elif ch == "{":
            braces += 1
        elif ch == "}":
            braces = max(0, braces - 1)
        elif ch == ";" and parens == 0 and brackets == 0 and braces == 0:
            return i + 1
    return len(text)


def decode_java_string(raw: str) -> str:
    try:
        return json.loads(f'"{raw}"')
    except (json.JSONDecodeError, UnicodeDecodeError):
        return raw.replace(r"\"", '"').replace(r"\\", "\\")


def first_string(argument_text: str) -> str | None:
    match = STRING_RE.search(argument_text)
    return decode_java_string(match.group(1)) if match else None


def endpoint_scheme(uri: str | None, argument_text: str = "") -> str:
    endpoint_dsl = re.match(r"\s*([a-z][A-Za-z0-9]*)\s*\(", argument_text)
    if endpoint_dsl:
        return endpoint_dsl.group(1).lower()
    if not uri:
        return "dynamic"
    if uri.startswith("{{") or uri.startswith("${"):
        return "dynamic"
    match = re.match(r"([A-Za-z][A-Za-z0-9+.-]*):", uri)
    return match.group(1).lower() if match else "unknown"


def extract_calls(fragment: str, base_offset: int, full_text: str) -> list[dict]:
    calls = []
    for match in CALL_RE.finditer(fragment):
        name = match.group(1)
        open_index = fragment.find("(", match.start())
        close_index = find_matching_paren(fragment, open_index)
        if close_index < 0:
            continue
        absolute = base_offset + match.start()
        calls.append(
            {
                "name": name,
                "args": fragment[open_index + 1 : close_index],
                "offset": absolute,
                "line": line_number(full_text, absolute),
            }
        )
    return calls


def dsl_fragments(content: str, starters: tuple[str, ...]) -> list[tuple[int, int, str]]:
    masked = mask_java(content)
    results = []
    occupied_until = 0
    starter_pattern = "|".join(re.escape(starter) for starter in starters)
    for match in re.finditer(rf"(?<![\w.])(?:{starter_pattern})\s*\(", masked):
        if match.start() < occupied_until:
            continue
        end = find_statement_end(masked, match.start())
        occupied_until = end
        results.append((match.start(), end, content[match.start() : end]))
    return results


def route_fragments(content: str) -> list[tuple[int, int, str]]:
    return dsl_fragments(content, ("from", "fromF", "rest", "routeTemplate"))


def configuration_fragments(content: str) -> list[tuple[int, int, str]]:
    return dsl_fragments(
        content,
        ("onException", "onCompletion", "errorHandler", "routeConfiguration"),
    )


def log_level(call: dict) -> str:
    match = re.search(r"\bLoggingLevel\s*\.\s*(TRACE|DEBUG|INFO|WARN|ERROR)\b", call["args"])
    return match.group(1) if match else "INFO"


def exception_warn_error_log_count(calls: list[dict], logs: list[dict]) -> int:
    """Count WARN/ERROR logs inside an inline exception-handling segment."""
    exception_indexes = set()
    in_exception_segment = False
    for index, call in enumerate(calls):
        if call["name"] in {"onException", "doCatch"}:
            in_exception_segment = True
        elif in_exception_segment and call["name"] in {
            "doFinally",
            "endDoTry",
            "end",
            "endParent",
        }:
            in_exception_segment = False
        if in_exception_segment:
            exception_indexes.add(index)
    return sum(
        log["level"] in {"WARN", "ERROR"} and log["index"] in exception_indexes
        for log in logs
    )


def extract_test_route_references(masked: str) -> list[str]:
    references = []
    method_re = re.compile(r"\b(?:adviceWith|getRouteDefinition|getRoute)\s*\(")
    for match in method_re.finditer(masked):
        open_index = masked.find("(", match.start())
        close_index = find_matching_paren(masked, open_index)
        if close_index < 0:
            continue
        route_id = first_string(masked[open_index + 1 : close_index])
        if route_id:
            references.append(route_id)
    return references


def make_match(file_path: str, content: str, call: dict) -> dict:
    line = call["line"]
    lines = content.splitlines()
    text = lines[line - 1].strip() if 0 < line <= len(lines) else call["name"]
    return {
        "file_path": file_path,
        "line_number": line,
        "line_end": line,
        "column_start": None,
        "column_end": None,
        "matched_text": text,
        "side": None,
        "context_before": lines[line - 2].strip() if line > 1 else None,
        "context_after": lines[line].strip() if line < len(lines) else None,
        "analyzer_id": "",
    }


def parse_java_file(file_path: str, content: str) -> dict:
    if not file_path.endswith(".java"):
        return {
            "class_name": "",
            "routes": [],
            "configurations": [],
            "java_logs": [],
            "test_route_refs": [],
            "is_test": False,
        }

    masked = mask_java(content)
    code_only = mask_java(content, keep_strings=False)
    route_builder_match = ROUTE_BUILDER_CLASS_RE.search(masked)
    class_match = route_builder_match or CLASS_RE.search(masked)
    class_name = class_match.group(1) if class_match else file_path.rsplit("/", 1)[-1][:-5]
    routes = []

    fragments = route_fragments(content) if route_builder_match else []
    for start, end, fragment in fragments:
        calls = extract_calls(fragment, start, content)
        if not calls:
            continue
        route_kind = {
            "rest": "rest",
            "routeTemplate": "template",
        }.get(calls[0]["name"], "route")
        route_id = next(
            (first_string(call["args"]) for call in calls if call["name"] == "routeId"),
            None,
        )
        if not route_id and route_kind == "rest":
            route_id = f"rest:{first_string(calls[0]['args']) or 'root'}@{line_number(content, start)}"
        if not route_id and route_kind == "template":
            route_id = f"route-template:{first_string(calls[0]['args']) or 'anonymous'}"
        route_id = route_id or f"{class_name}#from@{line_number(content, start)}"
        endpoint_calls = []
        for index, call in enumerate(calls):
            if call["name"] not in ENDPOINT_CALLS:
                continue
            uri = first_string(call["args"])
            scheme = endpoint_scheme(uri, call["args"])
            endpoint_calls.append(
                {
                    "method": call["name"],
                    "uri": uri,
                    "scheme": scheme,
                    "direction": (
                        "consumer" if call["name"] in {"from", "fromF"} else "producer"
                    ),
                    "dynamic": call["name"] in {"toD", "recipientList"} or scheme == "dynamic",
                    "index": index,
                    "line": call["line"],
                }
            )
        if route_kind == "rest":
            endpoint_calls.insert(
                0,
                {
                    "method": "rest",
                    "uri": first_string(calls[0]["args"]),
                    "scheme": "rest",
                    "direction": "consumer",
                    "dynamic": False,
                    "index": 0,
                    "line": calls[0]["line"],
                },
            )

        camel_logs = [
            {"level": log_level(call), "index": index, "line": call["line"]}
            for index, call in enumerate(calls)
            if call["name"] == "log"
        ]
        eip_calls = [call for call in calls if call["name"] not in METADATA_CALLS]
        depth = max_depth = 0
        for call in calls:
            if call["name"] in NESTING_START:
                depth += 1
                max_depth = max(max_depth, depth)
            elif call["name"] in NESTING_END:
                depth = max(0, depth - 1)

        external_calls = [
            endpoint
            for endpoint in endpoint_calls
            if endpoint["direction"] == "producer" and endpoint["scheme"] not in INTERNAL_SCHEMES
        ]
        internal_calls = [
            endpoint
            for endpoint in endpoint_calls
            if endpoint["direction"] == "producer" and endpoint["scheme"] in INTERNAL_SCHEMES
        ]
        external_nearby_logs = sum(
            1
            for log in camel_logs
            if any(abs(log["index"] - endpoint["index"]) <= 2 for endpoint in external_calls)
        )
        exception_paths = sum(call["name"] in {"onException", "doCatch"} for call in calls)
        exception_error_logs = exception_warn_error_log_count(calls, camel_logs)
        route = {
            "route_id": route_id,
            "route_kind": route_kind,
            "class_name": class_name,
            "file_path": file_path,
            "line_start": line_number(content, start),
            "line_end": line_number(content, end),
            "calls": calls,
            "eips": Counter(call["name"] for call in eip_calls),
            "eip_count": len(eip_calls),
            "branch_count": sum(call["name"] in BRANCH_EIPS for call in calls),
            "nesting_depth": max_depth,
            "endpoints": endpoint_calls,
            "external_call_count": len(external_calls),
            "internal_call_count": len(internal_calls),
            "dynamic_endpoint_count": sum(endpoint["dynamic"] for endpoint in endpoint_calls),
            "logs": camel_logs,
            "entry_log_count": sum(log["index"] < 3 for log in camel_logs),
            "exit_log_count": sum(
                log["index"] >= max(0, len(calls) - 3) for log in camel_logs
            ),
            "external_call_nearby_log_count": external_nearby_logs,
            "exception_path_count": exception_paths,
            "exception_warn_error_log_count": exception_error_logs,
            "processors": [
                first_string(call["args"]) or call["args"].strip()
                for call in calls
                if call["name"] in {"process", "bean"}
            ],
        }
        routes.append(route)

    configurations = []
    if route_builder_match:
        for start, end, fragment in configuration_fragments(content):
            calls = extract_calls(fragment, start, content)
            if not calls:
                continue
            logs = [
                {"level": log_level(call), "index": index, "line": call["line"]}
                for index, call in enumerate(calls)
                if call["name"] == "log"
            ]
            configurations.append(
                {
                    "configuration_type": calls[0]["name"],
                    "line_start": line_number(content, start),
                    "line_end": line_number(content, end),
                    "calls": calls,
                    "eips": Counter(call["name"] for call in calls),
                    "logs": logs,
                    "exception_path_count": sum(
                        call["name"] in {"onException", "doCatch"} for call in calls
                    ),
                    "exception_warn_error_log_count": (
                        exception_warn_error_log_count(calls, logs)
                    ),
                }
            )

    java_logs = []
    for match in JAVA_LOG_RE.finditer(code_only):
        java_logs.append(
            {
                "level": match.group(1).upper(),
                "line": line_number(content, match.start()),
            }
        )

    is_test = is_test_java_file(file_path)
    test_route_refs = extract_test_route_references(masked) if is_test else []

    return {
        "class_name": class_name,
        "routes": routes,
        "configurations": configurations,
        "java_logs": java_logs,
        "test_route_refs": test_route_refs,
        "is_test": is_test,
    }


def compact_route_fact(route: dict) -> dict:
    return {
        "kind": "route",
        "route_id": route["route_id"],
        "route_kind": route["route_kind"],
        "class_name": route["class_name"],
        "file_path": route["file_path"],
        "line_start": route["line_start"],
        "line_end": route["line_end"],
        "eip_count": route["eip_count"],
        "log_count": len(route["logs"]),
        "entry_log_count": route["entry_log_count"],
        "exit_log_count": route["exit_log_count"],
        "external_call_count": route["external_call_count"],
        "external_call_nearby_log_count": route["external_call_nearby_log_count"],
        "exception_path_count": route["exception_path_count"],
        "exception_warn_error_log_count": route["exception_warn_error_log_count"],
        "internal_endpoints": [
            {
                "scheme": endpoint["scheme"],
                "uri": endpoint["uri"],
                "direction": endpoint["direction"],
            }
            for endpoint in route["endpoints"]
            if endpoint["scheme"] in INTERNAL_SCHEMES
        ],
        "processors": route["processors"],
    }
