#!/usr/bin/env python3
"""Shared per-file Apache Camel Java DSL metric emission."""

from __future__ import annotations

import json
import sys
from collections import Counter

from .model import (
    CONCURRENCY_EIPS,
    LOG_LEVELS,
    RESILIENCE_EIPS,
    TRANSFORMATION_EIPS,
    make_match,
    parse_java_file,
)


def module_name(file_path: str) -> str:
    normalized = file_path.replace("\\", "/")
    prefix = normalized.split("/src/", 1)[0] if "/src/" in normalized else ""
    return prefix.rsplit("/", 1)[-1] if prefix else "root"


def metric(value, metric_name, category, dimensions=None, matches=None):
    tags = {"metric": metric_name, "category": category}
    if dimensions:
        tags.update({key: str(val) for key, val in dimensions.items() if val is not None})
    result = {"value": float(value), "tags": tags}
    if matches:
        result["matches"] = matches
    return result


def analyze_file(
    file_path: str,
    content: str,
    expected_file_kind: str | None = None,
) -> list[dict]:
    model = parse_java_file(file_path, content)
    if not file_path.endswith(".java"):
        return []

    results = []
    module = module_name(file_path)
    file_kind = "test" if model.get("is_test") else "production"
    if expected_file_kind is not None and file_kind != expected_file_kind:
        return []
    common = {
        "module": module,
        "class_name": model["class_name"],
        "file_kind": file_kind,
    }

    for route in model["routes"]:
        route_dims = {
            **common,
            "route_id": route["route_id"],
            "route_kind": route["route_kind"],
        }
        first_call = route["calls"][0]
        route_match = make_match(file_path, content, first_call)
        results.extend(
            [
                metric(1, "route_count", "route", route_dims, [route_match]),
                metric(route["eip_count"], "route_eip_count", "route", route_dims),
                metric(route["branch_count"], "route_branch_count", "route", route_dims),
                metric(route["nesting_depth"], "route_nesting_depth", "route", route_dims),
                metric(
                    route["external_call_count"],
                    "route_external_call_count",
                    "integration",
                    route_dims,
                ),
                metric(
                    route["internal_call_count"],
                    "route_internal_call_count",
                    "integration",
                    route_dims,
                ),
                metric(
                    route["dynamic_endpoint_count"],
                    "route_dynamic_endpoint_count",
                    "integration",
                    route_dims,
                ),
            ]
        )

        log_counts = Counter(log["level"] for log in route["logs"])
        log_count = sum(log_counts.values())
        if log_count:
            results.append(metric(log_count, "route_log_count", "observability", route_dims))
            results.append(metric(1, "route_with_log_count", "observability", route_dims))
        else:
            results.append(
                metric(
                    1,
                    "route_without_log_count",
                    "observability",
                    route_dims,
                    [route_match],
                )
            )
        if route["entry_log_count"]:
            results.append(
                metric(
                    route["entry_log_count"],
                    "entry_log_count",
                    "observability",
                    {**route_dims, "window": "first_3_dsl_calls"},
                )
            )
        if route["exit_log_count"]:
            results.append(
                metric(
                    route["exit_log_count"],
                    "exit_log_count",
                    "observability",
                    {**route_dims, "window": "last_3_dsl_calls"},
                )
            )
        if route["external_call_nearby_log_count"]:
            results.append(
                metric(
                    route["external_call_nearby_log_count"],
                    "external_call_nearby_log_count",
                    "observability",
                    {**route_dims, "window": "within_2_dsl_calls"},
                )
            )
        if route["exception_path_count"]:
            results.append(
                metric(
                    route["exception_path_count"],
                    "exception_path_count",
                    "resilience",
                    route_dims,
                )
            )
            if route["exception_warn_error_log_count"]:
                results.append(
                    metric(
                        route["exception_warn_error_log_count"],
                        "exception_route_warn_error_log_count",
                        "observability",
                        {**route_dims, "location": "exception_segment"},
                    )
                )
        for level in LOG_LEVELS:
            if log_counts[level]:
                results.append(
                    metric(
                        log_counts[level],
                        "route_log_level_count",
                        "observability",
                        {**route_dims, "log_level": level},
                    )
                )

        for eip, count in route["eips"].items():
            results.append(
                metric(
                    count,
                    "eip_usage_count",
                    "eip",
                    {**route_dims, "eip_type": eip},
                )
            )
            if eip in TRANSFORMATION_EIPS:
                results.append(
                    metric(
                        count,
                        "transformation_usage_count",
                        "transformation",
                        {**route_dims, "operation": eip},
                    )
                )
            if eip in CONCURRENCY_EIPS:
                results.append(
                    metric(
                        count,
                        "concurrency_usage_count",
                        "concurrency",
                        {**route_dims, "mechanism": eip},
                    )
                )
            if eip in RESILIENCE_EIPS:
                results.append(
                    metric(
                        count,
                        "resilience_usage_count",
                        "resilience",
                        {**route_dims, "mechanism": eip},
                    )
                )

        for endpoint in route["endpoints"]:
            results.append(
                metric(
                    1,
                    "endpoint_usage_count",
                    "integration",
                    {
                        **route_dims,
                        "component": endpoint["scheme"],
                        "direction": endpoint["direction"],
                        "dynamic": str(endpoint["dynamic"]).lower(),
                    },
                )
            )

    for configuration in model["configurations"]:
        config_dims = {
            **common,
            "configuration_type": configuration["configuration_type"],
        }
        if configuration["exception_path_count"]:
            results.append(
                metric(
                    configuration["exception_path_count"],
                    "exception_path_count",
                    "resilience",
                    config_dims,
                )
            )
        if configuration["exception_warn_error_log_count"]:
            results.append(
                metric(
                    configuration["exception_warn_error_log_count"],
                    "exception_route_warn_error_log_count",
                    "observability",
                    {**config_dims, "location": "exception_configuration"},
                )
            )
        config_log_counts = Counter(log["level"] for log in configuration["logs"])
        for level, count in config_log_counts.items():
            results.append(
                metric(
                    count,
                    "route_log_level_count",
                    "observability",
                    {**config_dims, "log_level": level},
                )
            )
        for mechanism, count in configuration["eips"].items():
            if mechanism in RESILIENCE_EIPS or mechanism == "routeConfiguration":
                results.append(
                    metric(
                        count,
                        "resilience_usage_count",
                        "resilience",
                        {**config_dims, "mechanism": mechanism},
                    )
                )

    java_log_counts = Counter(log["level"] for log in model["java_logs"])
    for level, count in java_log_counts.items():
        results.append(
            metric(
                count,
                "java_log_level_count",
                "observability",
                {**common, "log_level": level, "source": "java_logger"},
            )
        )

    if model.get("is_test"):
        results.append(metric(1, "camel_test_class_count", "testing", common))
        if model["test_route_refs"]:
            results.append(
                metric(
                    len(set(model["test_route_refs"])),
                    "test_route_reference_count",
                    "testing",
                    common,
                )
            )
    return results


def test():
    sample = """
import org.apache.camel.LoggingLevel;
class OrdersRoute extends RouteBuilder {
  public void configure() {
    from("kafka:orders").routeId("orders")
      .log(LoggingLevel.INFO, "received ${header.orderId}")
      .choice().when(simple("${body.valid}"))
        .to("direct:save")
      .otherwise()
        .log(LoggingLevel.ERROR, "invalid order")
        .to("jms:queue:invalid")
      .end();
  }
}
"""
    output = analyze_file("orders/src/main/java/OrdersRoute.java", sample)
    by_metric = Counter(item["tags"]["metric"] for item in output)
    assert by_metric["route_count"] == 1
    assert by_metric["endpoint_usage_count"] == 3
    assert by_metric["route_log_level_count"] == 2
    assert by_metric["route_log_count"] == 1
    assert by_metric["entry_log_count"] == 1
    assert by_metric["exit_log_count"] == 1
    assert by_metric["external_call_nearby_log_count"] == 1
    assert not any("coverage" in name or "density" in name for name in by_metric)
    route_count = next(item for item in output if item["tags"]["metric"] == "route_count")
    assert route_count["tags"]["module"] == "orders"

    endpoint_dsl = """
class ApiRoute extends EndpointRouteBuilder {
  public void configure() {
    from(kafka("orders")).routeId("typed").to(direct("work"));
    rest("/orders").get("/{id}").to("direct:load");
    routeTemplate("consumer").from("direct:{{name}}").to("mock:result");
  }
}
"""
    typed_output = analyze_file("src/main/java/ApiRoute.java", endpoint_dsl)
    components = {
        item["tags"].get("component")
        for item in typed_output
        if item["tags"]["metric"] == "endpoint_usage_count"
    }
    route_kinds = {
        item["tags"].get("route_kind")
        for item in typed_output
        if item["tags"]["metric"] == "route_count"
    }
    assert {"kafka", "direct", "rest", "mock"} <= components
    assert route_kinds == {"route", "rest", "template"}
    print("camel_java_file_metrics tests passed")


def main():
    for line in sys.stdin:
        try:
            data = json.loads(line)
            output = analyze_file(data.get("file_path", ""), data.get("content", ""))
            print(json.dumps(output, ensure_ascii=False), flush=True)
        except Exception as exc:
            print(f"camel_java_file_metrics: {exc}", file=sys.stderr)
            print("[]", flush=True)


if __name__ == "__main__":
    if len(sys.argv) > 1 and sys.argv[1] == "test":
        test()
    else:
        main()
