#!/usr/bin/env python3
"""Production-source Apache Camel Java DSL usage metrics for CodePrism."""

from __future__ import annotations

import json
import sys
from collections import Counter

from camel_support.file_metrics import analyze_file


def analyze_production_file(file_path: str, content: str) -> list[dict]:
    return analyze_file(file_path, content, expected_file_kind="production")


def test():
    production = """
class OrdersRoute extends RouteBuilder {
  public void configure() {
    onException(Exception.class).handled(true).log(LoggingLevel.WARN, "failed");
    routeConfiguration("shared").onCompletion().log("complete");
    fromF("direct:%s", "orders").routeId("orders")
      .log("received").doTry().toF("http:%s", "orders")
      .doCatch(Exception.class).log(LoggingLevel.ERROR, "request failed").end();
  }
}
"""
    test_source = production.replace("OrdersRoute", "OrdersRouteTest")
    output = analyze_production_file("src/main/java/OrdersRoute.java", production)
    by_metric = Counter(item["tags"]["metric"] for item in output)
    assert by_metric["route_count"] == 1
    assert by_metric["route_log_count"] == 1
    assert by_metric["exception_path_count"] == 2
    assert by_metric["exception_route_warn_error_log_count"] == 2
    assert by_metric["resilience_usage_count"] >= 3
    assert all(item["tags"]["file_kind"] == "production" for item in output)
    assert analyze_production_file("src/test/java/OrdersRouteTest.java", test_source) == []
    ordinary_java = """
class NotARoute {
  Object from(String value) { return value; }
  void work() { from("not:camel").toString(); }
}
"""
    ordinary_output = analyze_production_file(
        "src/main/java/NotARoute.java",
        ordinary_java,
    )
    assert not any(item["tags"]["metric"] == "route_count" for item in ordinary_output)
    print("camel_java_production_metrics tests passed")


def main():
    for line in sys.stdin:
        try:
            data = json.loads(line)
            output = analyze_production_file(
                data.get("file_path", ""),
                data.get("content", ""),
            )
            print(json.dumps(output, ensure_ascii=False), flush=True)
        except Exception as exc:
            print(f"camel_java_production_metrics: {exc}", file=sys.stderr)
            print("[]", flush=True)


if __name__ == "__main__":
    if len(sys.argv) > 1 and sys.argv[1] == "test":
        test()
    else:
        main()
