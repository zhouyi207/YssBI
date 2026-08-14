from re import Match, compile, match


TEST_FILE_PATTERN = compile(r"\.(?:test|spec)\.(?:ts|tsx)$")
RUST_TEST_ATTRIBUTE = compile(
    r"#\s*\[\s*(?:test|tokio\s*::\s*test(?:\s*\([^]]*\))?)\s*\]",
)
RUST_CFG_TEST_ATTRIBUTE = compile(r"#\s*\[\s*cfg\s*\(\s*test\s*\)\s*\]")


def normalized_parts(path: str):
    return tuple(part for part in path.replace("\\", "/").split("/") if part)


def is_test_file(path: str, ext: str) -> bool:
    parts = normalized_parts(path)
    filename = parts[-1] if parts else ""
    if ext in {".ts", ".tsx"}:
        return "__tests__" in parts or TEST_FILE_PATTERN.search(filename) is not None
    if ext == ".rs":
        return (
            "tests" in parts
            or filename == "tests.rs"
            or filename.endswith(("_test.rs", "_tests.rs"))
        )
    return False


def mask_rust_comments_and_literals(source: str) -> str:
    masked = list(source)
    index = 0
    block_depth = 0
    state = "code"
    raw_hashes = 0

    def hide(position: int):
        if masked[position] not in "\r\n":
            masked[position] = " "

    while index < len(source):
        if state == "line_comment":
            if source[index] == "\n":
                state = "code"
            else:
                hide(index)
            index += 1
            continue

        if state == "block_comment":
            if source.startswith("/*", index):
                hide(index)
                hide(index + 1)
                block_depth += 1
                index += 2
            elif source.startswith("*/", index):
                hide(index)
                hide(index + 1)
                block_depth -= 1
                index += 2
                if block_depth == 0:
                    state = "code"
            else:
                hide(index)
                index += 1
            continue

        if state == "string":
            if source[index] == "\\" and index + 1 < len(source):
                hide(index)
                hide(index + 1)
                index += 2
            elif source[index] == '"':
                hide(index)
                index += 1
                state = "code"
            else:
                hide(index)
                index += 1
            continue

        if state == "raw_string":
            terminator = '"' + "#" * raw_hashes
            if source.startswith(terminator, index):
                for offset in range(len(terminator)):
                    hide(index + offset)
                index += len(terminator)
                state = "code"
            else:
                hide(index)
                index += 1
            continue

        if state == "char":
            if source[index] == "\\" and index + 1 < len(source):
                hide(index)
                hide(index + 1)
                index += 2
            elif source[index] == "'":
                hide(index)
                index += 1
                state = "code"
            else:
                hide(index)
                index += 1
            continue

        if source.startswith("//", index):
            hide(index)
            hide(index + 1)
            state = "line_comment"
            index += 2
            continue
        if source.startswith("/*", index):
            hide(index)
            hide(index + 1)
            block_depth = 1
            state = "block_comment"
            index += 2
            continue

        raw_match = match(r"(?:br|r)(#+)?\"", source[index:])
        if raw_match:
            token = raw_match.group(0)
            raw_hashes = token.count("#")
            for offset in range(len(token)):
                hide(index + offset)
            index += len(token)
            state = "raw_string"
            continue
        if source[index] == '"':
            hide(index)
            state = "string"
            index += 1
            continue
        if source[index] == "'" and match(r"'(?:\\.|[^\\'])'", source[index:]):
            hide(index)
            state = "char"
            index += 1
            continue
        index += 1

    return "".join(masked)


def matching_brace(source: str, opening: int):
    depth = 0
    for index in range(opening, len(source)):
        if source[index] == "{":
            depth += 1
        elif source[index] == "}":
            depth -= 1
            if depth == 0:
                return index
    return None


def rust_attribute_range(source: str, attribute: Match):
    opening = source.find("{", attribute.end())
    semicolon = source.find(";", attribute.end())
    if semicolon >= 0 and (opening < 0 or semicolon < opening):
        return attribute.start(), semicolon
    if opening < 0:
        return None
    closing = matching_brace(source, opening)
    return None if closing is None else (attribute.start(), closing)


def merge_ranges(ranges):
    merged = []
    for start, end in sorted(ranges):
        if merged and start <= merged[-1][1] + 1:
            merged[-1] = (merged[-1][0], max(merged[-1][1], end))
        else:
            merged.append((start, end))
    return merged


def rust_inline_test_lines(source: str):
    masked = mask_rust_comments_and_literals(source)
    ranges = []
    for pattern in (RUST_CFG_TEST_ATTRIBUTE, RUST_TEST_ATTRIBUTE):
        for attribute in pattern.finditer(masked):
            item_range = rust_attribute_range(masked, attribute)
            if item_range is not None:
                ranges.append(item_range)

    line_indexes = set()
    for start, end in merge_ranges(ranges):
        start_line = source.count("\n", 0, start)
        end_line = source.count("\n", 0, end)
        line_indexes.update(range(start_line, end_line + 1))
    return line_indexes
