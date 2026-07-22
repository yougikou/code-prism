"""Helpers for the generic CodePrism cross-file analyzer protocol."""

import hashlib


def content_group_key(normalized_content):
    return hashlib.sha256(normalized_content.encode("utf-8")).hexdigest()


def duplicate_finding(group_key, entries):
    """Build duplication findings and metrics inside the analyzer domain."""
    occurrences = [{
        "file_path": entry.get("file_path", ""),
        "line_start": entry.get("int_data2") or 0,
        "line_end": entry.get("int_data3") or 0,
        "change_type": entry.get("str_data1"),
        "side": entry.get("str_data2"),
    } for entry in entries]
    by_file = {}
    for occurrence in occurrences:
        by_file.setdefault(occurrence["file_path"], []).append(occurrence)

    metrics = []
    finding_before = any(item.get("side") == "0" for item in occurrences)
    finding_after = any(item.get("side") != "0" for item in occurrences)
    for index, (file_path, file_occurrences) in enumerate(sorted(by_file.items())):
        before = [item for item in file_occurrences if item.get("side") == "0"]
        after = [item for item in file_occurrences if item.get("side") != "0"]
        values = {
            "occurrence_count": (len(before), len(after)),
            "affected_file_count": (1 if before else 0, 1 if after else 0),
            "affected_line_count": (
                sum(max(0, item["line_end"] - item["line_start"] + 1) for item in before),
                sum(max(0, item["line_end"] - item["line_start"] + 1) for item in after),
            ),
        }
        if index == 0:
            values["finding_count"] = (1 if finding_before else 0, 1 if finding_after else 0)
        for metric_key, (value_before, value_after) in values.items():
            metrics.append({
                "metric_key": metric_key,
                "file_path": file_path,
                "value_before": value_before,
                "value_after": value_after,
                "change_type": file_occurrences[0].get("change_type"),
                "scope": "{}:{}-{}".format(group_key, file_occurrences[0]["line_start"], file_occurrences[0]["line_end"]),
            })
    return {
        "finding_key": group_key,
        "content": entries[0].get("blob_data") or "",
        "occurrences": occurrences,
        "tags": {"category": "duplication"},
        "metrics": metrics,
    }
