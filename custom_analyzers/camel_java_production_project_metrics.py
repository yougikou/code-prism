#!/usr/bin/env python3
"""Cross-file production Apache Camel Java DSL project metrics for CodePrism."""

from __future__ import annotations

import json
import sys
from collections import defaultdict

from camel_support.model import compact_route_fact, parse_java_file


def extract_facts(file_path: str, content: str) -> list[dict]:
    model = parse_java_file(file_path, content)
    if model.get("is_test"):
        return []
    blocks = []
    for route in model["routes"]:
        fact = compact_route_fact(route)
        encoded = json.dumps(fact, sort_keys=True, ensure_ascii=False)
        blocks.append(
            {
                "block_size": -20,
                "line_start": route["line_start"],
                "line_end": route["line_end"],
                "block_content": encoded,
                "normalized_content": encoded,
                "metric_key": "project_metric",
                "category": "camel_project",
            }
        )
    return blocks


def decode_facts(blocks: list[dict], side: str) -> list[dict]:
    facts = []
    for block in blocks:
        block_side = block.get("str_data2")
        include = (
            block_side in (None, "1")
            if side == "after"
            else block_side == "0"
        )
        if not include:
            continue
        try:
            facts.append(json.loads(block.get("blob_data") or "{}"))
        except json.JSONDecodeError:
            continue
    return facts


def snapshot_metrics(facts: list[dict], scan_basis: str) -> list[dict]:
    routes = [fact for fact in facts if fact.get("kind") == "route"]
    common = {
        "category": "camel_project",
        "file_kind": "production",
        "scan_basis": scan_basis,
    }
    metrics = []

    def add(metric_name, value, category="camel_project", dimensions=None):
        tags = {**common, "metric": metric_name, "category": category}
        if dimensions:
            tags.update({key: str(val) for key, val in dimensions.items()})
        metrics.append(
            {
                "file_path": "__project__",
                "tags": tags,
                "value": float(value),
            }
        )

    route_count = len(routes)
    add("project_route_count", route_count, "route")
    if route_count:
        add("project_route_log_count", sum(route["log_count"] for route in routes), "observability")
        add("route_with_log_count", sum(route["log_count"] > 0 for route in routes), "observability")
        add("route_without_log_count", sum(route["log_count"] == 0 for route in routes), "observability")
        add("entry_log_count", sum(route["entry_log_count"] for route in routes), "observability")
        add("exit_log_count", sum(route["exit_log_count"] for route in routes), "observability")
        add(
            "external_call_nearby_log_count",
            sum(route["external_call_nearby_log_count"] for route in routes),
            "observability",
        )
        add(
            "exception_route_warn_error_log_count",
            sum(route["exception_warn_error_log_count"] for route in routes),
            "observability",
        )
        add("avg_routes_per_builder", route_count / max(1, len({r["class_name"] for r in routes})), "modularity")

    producers = defaultdict(set)
    consumers = defaultdict(set)
    for route in routes:
        for endpoint in route["internal_endpoints"]:
            uri = endpoint.get("uri")
            if not uri:
                continue
            target = consumers if endpoint["direction"] == "consumer" else producers
            target[uri].add(route["route_id"])
    internal_uris = set(producers) | set(consumers)
    add("internal_endpoint_count", len(internal_uris), "topology")
    if internal_uris:
        producer_counts = [len(producers[uri]) for uri in internal_uris]
        consumer_counts = [len(consumers[uri]) for uri in internal_uris]
        add("internal_endpoint_avg_producer_routes", sum(producer_counts) / len(producer_counts), "topology")
        add("internal_endpoint_max_producer_routes", max(producer_counts), "topology")
        add("internal_endpoint_avg_consumer_routes", sum(consumer_counts) / len(consumer_counts), "topology")

    processor_routes = defaultdict(set)
    for route in routes:
        for processor in route["processors"]:
            if processor:
                processor_routes[processor].add(route["route_id"])
    shared = {name: route_ids for name, route_ids in processor_routes.items() if len(route_ids) > 1}
    add("shared_processor_count", len(shared), "modularity")

    return metrics


def finalize_metrics(blocks: list[dict]) -> dict:
    is_diff = any(block.get("str_data2") in {"0", "1"} for block in blocks)
    scan_basis = "changed_files" if is_diff else "scanned_files"
    before = snapshot_metrics(decode_facts(blocks, "before"), scan_basis) if is_diff else []
    after = snapshot_metrics(decode_facts(blocks, "after"), scan_basis)

    merged = {}
    for side, entries in (("before", before), ("after", after)):
        for entry in entries:
            key = (
                entry["file_path"],
                json.dumps(entry["tags"], sort_keys=True),
            )
            current = merged.setdefault(
                key,
                {
                    "file_path": entry["file_path"],
                    "change_type": "M" if is_diff else "A",
                    "tags": entry["tags"],
                    "value_before": None,
                    "value_after": None,
                },
            )
            current[f"value_{side}"] = entry["value"]
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
                "finding_key": "camel_production_project_metrics",
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
    from("direct:start").routeId("start").log("start").to("direct:work");
    from("direct:work").routeId("work").to("http:service");
  }
}
"""
    test_source = """
class RoutesTest extends RouteBuilder {
  public void configure() {
    from("direct:test").routeId("test-only").log("test");
  }
}
"""
    blocks = []
    for path, source in (
        ("src/main/java/Routes.java", production),
        ("src/test/java/RoutesTest.java", test_source),
    ):
        for block in extract_facts(path, source):
            blocks.append(
                {
                    "blob_data": block["block_content"],
                    "str_data2": None,
                }
            )
    output = finalize_metrics(blocks)
    metrics = output["findings"][0]["metrics"]
    by_name = {item["metric_key"]: item for item in metrics}
    assert by_name["project_route_count"]["value_after"] == 2
    assert by_name["project_route_log_count"]["value_after"] == 1
    assert by_name["route_with_log_count"]["value_after"] == 1
    assert by_name["route_without_log_count"]["value_after"] == 1
    assert "route_test_reference_coverage" not in by_name
    assert all(item["tags"]["file_kind"] == "production" for item in metrics)
    print("camel_java_production_project_metrics tests passed")


def main():
    for line in sys.stdin:
        try:
            message = json.loads(line)
            if message.get("action") == "extract":
                result = extract_facts(message.get("file_path", ""), message.get("content", ""))
            elif message.get("action") == "finalize":
                result = finalize_metrics(message.get("blocks", []))
            else:
                result = []
            print(json.dumps(result, ensure_ascii=False), flush=True)
        except Exception as exc:
            print(f"camel_java_production_project_metrics: {exc}", file=sys.stderr)
            print("[]", flush=True)


if __name__ == "__main__":
    if len(sys.argv) > 1 and sys.argv[1] == "test":
        test()
    else:
        main()
