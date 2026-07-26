#!/usr/bin/env python3
"""Test-source Apache Camel Java DSL usage metrics for CodePrism."""

from __future__ import annotations

import json
import sys
from collections import Counter

from camel_support.file_metrics import analyze_file


def analyze_test_file(file_path: str, content: str) -> list[dict]:
    return analyze_file(file_path, content, expected_file_kind="test")


def test():
    test_source = """
class OrdersRouteTest extends RouteBuilder {
  public void configure() {
    from("direct:test").routeId("test-orders").log("test").to("mock:orders");
  }
  void verify() {
    AdviceWith.adviceWith(context, "orders", advice -> advice.mockEndpoints());
  }
}
"""
    production = test_source.replace("OrdersRouteTest", "OrdersRoute")
    output = analyze_test_file("src/test/java/OrdersRouteTest.java", test_source)
    by_metric = Counter(item["tags"]["metric"] for item in output)
    assert by_metric["camel_test_class_count"] == 1
    assert by_metric["test_route_reference_count"] == 1
    assert by_metric["route_count"] == 1
    assert all(item["tags"]["file_kind"] == "test" for item in output)
    assert analyze_test_file("src/main/java/OrdersRoute.java", production) == []
    print("camel_java_test_metrics tests passed")


def main():
    for line in sys.stdin:
        try:
            data = json.loads(line)
            output = analyze_test_file(
                data.get("file_path", ""),
                data.get("content", ""),
            )
            print(json.dumps(output, ensure_ascii=False), flush=True)
        except Exception as exc:
            print(f"camel_java_test_metrics: {exc}", file=sys.stderr)
            print("[]", flush=True)


if __name__ == "__main__":
    if len(sys.argv) > 1 and sys.argv[1] == "test":
        test()
    else:
        main()
