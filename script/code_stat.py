import os
import sys
from collections import defaultdict

CODE_EXTENSIONS = {
    ".py", ".rs", ".js", ".ts", ".jsx", ".tsx",
    ".java", ".cpp", ".c", ".h",
    ".go", ".cs", ".swift",
}

IGNORE_DIRS = {
    ".git", ".idea", ".vscode",
    "node_modules", "dist", "build",
    "target", "__pycache__",
}

def is_comment(line: str, ext: str) -> bool:
    if ext == ".py":
        return line.startswith("#")
    if ext in {".rs", ".js", ".ts", ".java", ".c", ".cpp", ".go", ".cs"}:
        return line.startswith("//")
    return False


def analyze_file(path: str):
    ext = os.path.splitext(path)[1]
    total = blank = comment = code = 0

    try:
        with open(path, "r", encoding="utf-8", errors="ignore") as f:
            for line in f:
                total += 1
                stripped = line.strip()

                if not stripped:
                    blank += 1
                elif is_comment(stripped, ext):
                    comment += 1
                else:
                    code += 1
    except Exception as e:
        print(f"⚠️  Failed to read {path}: {e}")

    return total, code, comment, blank


def analyze_project(root: str):
    stats = defaultdict(lambda: {
        "files": 0,
        "total": 0,
        "code": 0,
        "comment": 0,
        "blank": 0,
    })

    for dirpath, dirnames, filenames in os.walk(root):
        dirnames[:] = [d for d in dirnames if d not in IGNORE_DIRS]

        for filename in filenames:
            ext = os.path.splitext(filename)[1]
            if ext not in CODE_EXTENSIONS:
                continue

            path = os.path.join(dirpath, filename)
            total, code, comment, blank = analyze_file(path)

            s = stats[ext]
            s["files"] += 1
            s["total"] += total
            s["code"] += code
            s["comment"] += comment
            s["blank"] += blank

    return stats


def print_report(stats):
    for ext, s in sorted(stats.items()):
        print(f"\nLanguage: {ext}")
        print(f"  Files        : {s['files']}")
        print(f"  Total lines  : {s['total']}")
        print(f"  Code lines   : {s['code']}")
        print(f"  Comment lines: {s['comment']}")
        print(f"  Blank lines  : {s['blank']}")


def parse_root_from_argv():
    if len(sys.argv) == 1:
        return os.getcwd()

    if sys.argv[1] in ("-h", "--help"):
        print("Usage:")
        print("  python code_stats.py            # analyze current directory")
        print("  python code_stats.py <path>     # analyze target directory")
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
