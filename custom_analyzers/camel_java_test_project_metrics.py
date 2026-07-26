#!/usr/bin/env python3
"""Cross-file Apache Camel Java DSL test metrics for CodePrism."""

from __future__ import annotations

import json
import sys

from camel_support.model import parse_java_file


def fact_block(fact: dict, block_size: int, line_start: int, line_end: int) -> dict:
    encoded = json.dumps(fact, sort_keys=True, ensure_ascii=False)
    return {
        "block_size": block_size,
        "line_start": line_start,
        "line_end": line_end,
        "block_content": encoded,
        "normalized_content": encoded,
        "metric_key": "test_project_metric",
        "category": "camel_testing",
    }


def extract_facts(file_path: str, content: str) -> list[dict]:
    model = parse_java_file(file_path, content)
    if model.get("is_test"):
        return [
            fact_block(
                {
                    "kind": "test_class",
                    "file_path": file_path,
                    "class_name": model["class_name"],
                    "route_refs": sorted(set(model["test_route_refs"])),
                },
                -22,
                1,
                max(1, content.count("\n") + 1),
            )
        ]

    blocks = []
    for route in model["routes"]:
        route_id = route["route_id"]
        if "#from@" in route_id:
            continue
        blocks.append(
            fact_block(
                {
                    "kind": "production_route_id",
                    "route_id": route_id,
                },
                -23,
                route["line_start"],
                route["line_end"],
            )
        )
    return blocks


def decode_facts(blocks: list[dict], side: str) -> list[dict]:
    facts = []
    for block in blocks:
        block_side = block.get("str_data2")
        include = block_side in (None, "1") if side == "after" else block_side == "0"
        if not include:
            continue
        try:
            facts.append(json.loads(block.get("blob_data") or "{}"))
        except json.JSONDecodeError:
            continue
    return facts


def snapshot_metrics(facts: list[dict], scan_basis: str) -> list[dict]:
    tests = [fact for fact in facts if fact.get("kind") == "test_class"]
    production_route_ids = {
        fact["route_id"]
        for fact in facts
        if fact.get("kind") == "production_route_id" and fact.get("route_id")
    }
    all_refs = {
        route_id
        for test in tests
        for route_id in test.get("route_refs", [])
    }
    matched_refs = all_refs & production_route_ids
    common = {
        "category": "camel_testing",
        "file_kind": "test",
        "scan_basis": scan_basis,
    }

    def entry(metric_name: str, value: float) -> dict:
        return {
            "file_path": "__tests__",
            "tags": {**common, "metric": metric_name},
            "value": float(value),
        }

    metrics = [
        entry("camel_test_class_count", len(tests)),
        entry("test_route_reference_count", len(all_refs)),
        entry("matched_test_route_reference_count", len(matched_refs)),
    ]
    if production_route_ids:
        metrics.append(
            entry(
                "route_test_reference_coverage",
                len(matched_refs) / len(production_route_ids),
            )
        )
    return metrics


def finalize_metrics(blocks: list[dict]) -> dict:
    is_diff = any(block.get("str_data2") in {"0", "1"} for block in blocks)
    scan_basis = "changed_files" if is_diff else "scanned_files"
    before = snapshot_metrics(decode_facts(blocks, "before"), scan_basis) if is_diff else []
    after = snapshot_metrics(decode_facts(blocks, "after"), scan_basis)

    merged = {}
    for side, entries in (("before", before), ("after", after)):
        for item in entries:
            key = (item["file_path"], json.dumps(item["tags"], sort_keys=True))
            current = merged.setdefault(
                key,
                {
                    "file_path": item["file_path"],
                    "change_type": "M" if is_diff else "A",
                    "tags": item["tags"],
                    "value_before": None,
                    "value_after": None,
                },
            )
            current[f"value_{side}"] = item["value"]
    metrics = []
    for item in merged.values():
        tags = dict(item["tags"])
        metric_key = tags.pop("metric")
        metrics.append(
            {
                "metric_key": metric_key,
                "file_path": item["file_path"],
                "change_type": item["change_type"],
                "tags": tags,
                "value_before": item["value_before"],
                "value_after": item["value_after"],
            }
        )
    return {
        "findings": [
            {
                "finding_key": "camel_test_project_metrics",
                "content": None,
                "occurrences": [],
                "tags": {},
                "metrics": metrics,
            }
        ]
    }


def test():
    production = """
class Routes extends RouteBuilder {
  public void configure() {
    from("direct:start").routeId("start").to("direct:work");
    from("direct:work").routeId("work").to("mock:result");
  }
}
"""
    test_source = """
class RoutesTest extends CamelTestSupport {
  void testRoute() {
    getRouteDefinition("start");
    getRouteDefinition("missing");
  }
}
"""
    blocks = []
    for path, source in (
        ("src/main/java/Routes.java", production),
        ("src/test/java/RoutesTest.java", test_source),
    ):
        for block in extract_facts(path, source):
            blocks.append({"blob_data": block["block_content"], "str_data2": None})

    output = finalize_metrics(blocks)
    metrics = output["findings"][0]["metrics"]
    by_name = {item["metric_key"]: item for item in metrics}
    assert by_name["camel_test_class_count"]["value_after"] == 1
    assert by_name["test_route_reference_count"]["value_after"] == 2
    assert by_name["matched_test_route_reference_count"]["value_after"] == 1
    assert by_name["route_test_reference_coverage"]["value_after"] == 0.5
    assert all(item["tags"]["file_kind"] == "test" for item in metrics)
    print("camel_java_test_project_metrics tests passed")


def main():
    for line in sys.stdin:
        try:
            message = json.loads(line)
            if message.get("action") == "extract":
                result = extract_facts(
                    message.get("file_path", ""),
                    message.get("content", ""),
                )
            elif message.get("action") == "finalize":
                result = finalize_metrics(message.get("blocks", []))
            else:
                result = []
            print(json.dumps(result, ensure_ascii=False), flush=True)
        except Exception as exc:
            print(f"camel_java_test_project_metrics: {exc}", file=sys.stderr)
            print("[]", flush=True)


if __name__ == "__main__":
    if len(sys.argv) > 1 and sys.argv[1] == "test":
        test()
    else:
        main()
