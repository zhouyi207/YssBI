import os
import sys
from collections import defaultdict

from source_test_detection import is_test_file, rust_inline_test_lines

CODE_EXTENSIONS = {
    ".py", ".rs", ".js", ".ts", ".jsx", ".tsx",
    ".java", ".cpp", ".hpp", ".c", ".h",
    ".go", ".cs", ".swift",
}

IGNORE_DIRS = {
    ".git", ".idea", ".vscode", ".superpowers",
    "node_modules", "dist", "build",
    "target", "__pycache__",
}

C_STYLE_EXTENSIONS = {
    ".rs", ".js", ".ts", ".jsx", ".tsx", ".java",
    ".c", ".cpp", ".hpp", ".h", ".go", ".cs", ".swift",
}


def classify_lines(lines, ext: str):
    kinds = []
    in_block_comment = False

    for line in lines:
        if not line.strip():
            kinds.append("blank")
            continue
        if ext == ".py":
            kinds.append("comment" if line.lstrip().startswith("#") else "code")
            continue
        if ext not in C_STYLE_EXTENSIONS:
            kinds.append("code")
            continue

        index = 0
        has_code = False
        has_comment = in_block_comment
        while index < len(line):
            if in_block_comment:
                has_comment = True
                end = line.find("*/", index)
                if end < 0:
                    index = len(line)
                    continue
                in_block_comment = False
                index = end + 2
                continue

            if line.startswith("//", index):
                has_comment = True
                break
            if line.startswith("/*", index):
                has_comment = True
                in_block_comment = True
                index += 2
                continue
            if not line[index].isspace():
                has_code = True
            index += 1

        kinds.append("code" if has_code else "comment" if has_comment else "blank")

    return kinds


def empty_file_stats():
    return {
        "total": 0,
        "code": 0,
        "comment": 0,
        "blank": 0,
        "production_total": 0,
        "production_code": 0,
        "production_comment": 0,
        "production_blank": 0,
        "test_total": 0,
        "test_code": 0,
        "test_comment": 0,
        "test_blank": 0,
        "has_production": False,
        "has_tests": False,
    }


def analyze_file(path: str):
    ext = os.path.splitext(path)[1]
    result = empty_file_stats()

    try:
        with open(path, "r", encoding="utf-8", errors="ignore") as file:
            source = file.read()
    except OSError as error:
        print(f"⚠️  Failed to read {path}: {error}")
        return result

    lines = source.splitlines(keepends=True)
    if source and not lines:
        lines = [source]
    kinds = classify_lines(lines, ext)
    if is_test_file(path, ext):
        test_lines = set(range(len(lines)))
    elif ext == ".rs":
        test_lines = rust_inline_test_lines(source)
    else:
        test_lines = set()

    for index, kind in enumerate(kinds):
        result["total"] += 1
        result[kind] += 1
        scope = "test" if index in test_lines else "production"
        result[f"{scope}_total"] += 1
        result[f"{scope}_{kind}"] += 1

    result["has_tests"] = bool(test_lines)
    result["has_production"] = any(
        result[f"production_{kind}"] > 0 for kind in ("code", "comment")
    )
    return result


def analyze_project(root: str):
    stats = defaultdict(lambda: {
        "files": 0,
        "production_files": 0,
        "test_files": 0,
        **empty_file_stats(),
    })

    for dirpath, dirnames, filenames in os.walk(root):
        dirnames[:] = [d for d in dirnames if d not in IGNORE_DIRS]

        for filename in filenames:
            ext = os.path.splitext(filename)[1]
            if ext not in CODE_EXTENSIONS:
                continue

            path = os.path.join(dirpath, filename)
            file_stats = analyze_file(path)

            s = stats[ext]
            s["files"] += 1
            s["production_files"] += int(file_stats["has_production"])
            s["test_files"] += int(file_stats["has_tests"])
            for key in (
                "total", "code", "comment", "blank",
                "production_total", "production_code", "production_comment", "production_blank",
                "test_total", "test_code", "test_comment", "test_blank",
            ):
                s[key] += file_stats[key]

    return stats


def print_report(stats):
    totals = defaultdict(int)

    for ext, s in sorted(stats.items()):
        print(f"\nLanguage: {ext}")
        print(f"  Files                : {s['files']}")
        print(f"  Total lines          : {s['total']}")
        print(f"  Code lines           : {s['code']}")
        print(f"  Comment lines        : {s['comment']}")
        print(f"  Blank lines          : {s['blank']}")
        print(f"  Production files     : {s['production_files']}")
        print(f"  Production code lines: {s['production_code']}")
        print(f"  Test files           : {s['test_files']}")
        print(f"  Test total lines     : {s['test_total']}")
        print(f"  Test code lines      : {s['test_code']}")
        print(f"  Test comment lines   : {s['test_comment']}")
        print(f"  Test blank lines     : {s['test_blank']}")
        for key in (
            "files", "total", "code", "comment", "blank",
            "production_files", "production_code", "test_files",
            "test_total", "test_code", "test_comment", "test_blank",
        ):
            totals[key] += s[key]

    ratio = (
        totals["test_code"] / totals["production_code"] * 100
        if totals["production_code"]
        else 0.0
    )
    print(f"\n{'='*40}")
    print("Totals")
    print(f"  Files                : {totals['files']}")
    print(f"  Total lines          : {totals['total']}")
    print(f"  Code lines           : {totals['code']}")
    print(f"  Comment lines        : {totals['comment']}")
    print(f"  Blank lines          : {totals['blank']}")
    print(f"  Production files     : {totals['production_files']}")
    print(f"  Production code lines: {totals['production_code']}")
    print(f"  Test files           : {totals['test_files']}")
    print(f"  Test total lines     : {totals['test_total']}")
    print(f"  Test code lines      : {totals['test_code']}")
    print(f"  Test comment lines   : {totals['test_comment']}")
    print(f"  Test blank lines     : {totals['test_blank']}")
    print(f"  Test/production ratio: {ratio:.1f}%")


def parse_root_from_argv():
    if len(sys.argv) == 1:
        return os.getcwd()

    if sys.argv[1] in ("-h", "--help"):
        print("Usage:")
        print("  python code_stat.py            # analyze current directory")
        print("  python code_stat.py <path>     # analyze target directory")
        sys.exit(0)

    root = sys.argv[1]
    if not os.path.isdir(root):
        print(f"❌ Not a directory: {root}")
        sys.exit(1)

    return os.path.abspath(root)


if __name__ == "__main__":
    project_root = parse_root_from_argv()
    print(f"📂 Analyzing project: {project_root}")
    stats = analyze_project(project_root)
    print_report(stats)
