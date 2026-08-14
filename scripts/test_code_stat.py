import tempfile
import unittest
from pathlib import Path

import code_stat


class CodeStatTest(unittest.TestCase):
    def analyze_sources(self, sources: dict[str, str]):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            for relative_path, source in sources.items():
                path = root / relative_path
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_text(source, encoding="utf-8")
            return code_stat.analyze_project(str(root))

    def test_counts_typescript_test_files_separately(self):
        stats = self.analyze_sources({
            "src/runtime.ts": "export const runtime = 1;\n",
            "src/runtime.test.ts": "import { runtime } from './runtime';\ntest('runtime', () => runtime);\n",
            "src/widget.spec.tsx": "const view = <div />;\ntest('view', () => view);\n",
            "src/__tests__/helper.ts": "export const fixture = 1;\n",
            ".superpowers/worktree/src/duplicate.test.ts": "test('duplicate', () => {});\n",
        })

        self.assertEqual(stats[".ts"]["files"], 3)
        self.assertEqual(stats[".ts"]["test_files"], 2)
        self.assertEqual(stats[".ts"]["test_code"], 3)
        self.assertEqual(stats[".ts"]["production_code"], 1)
        self.assertEqual(stats[".tsx"]["test_files"], 1)
        self.assertEqual(stats[".tsx"]["test_code"], 2)
        self.assertEqual(stats[".tsx"]["production_code"], 0)

    def test_counts_rust_inline_test_modules_without_counting_string_braces(self):
        stats = self.analyze_sources({
            "src/lib.rs": '''pub fn production() -> &'static str { "}" }

#[cfg(test)]
mod tests {
    // A brace in a comment must not end the module: }
    #[test]
    fn works() {
        assert_eq!(super::production(), "}");
    }
}
''',
        })

        self.assertEqual(stats[".rs"]["test_files"], 1)
        self.assertEqual(stats[".rs"]["test_code"], 7)
        self.assertEqual(stats[".rs"]["test_comment"], 1)
        self.assertEqual(stats[".rs"]["production_code"], 1)

    def test_ignores_rust_test_attributes_inside_comments_and_literals(self):
        stats = self.analyze_sources({
            "src/lib.rs": '''const TEXT: &str = r#"#[cfg(test)] mod fake { }"#;
// #[test] fn commented_out() {}
pub fn production() {}
''',
        })

        self.assertEqual(stats[".rs"]["test_files"], 0)
        self.assertEqual(stats[".rs"]["test_code"], 0)
        self.assertEqual(stats[".rs"]["production_code"], 2)

    def test_counts_standalone_rust_test_functions_and_test_files(self):
        stats = self.analyze_sources({
            "src/checks.rs": '''pub fn production() {}

#[tokio::test(flavor = "current_thread")]
async fn async_check() {
    assert!(true);
}
''',
            "tests/integration.rs": '''use crate::something;

#[test]
fn integration() {
    assert!(true);
}
''',
            "src/widget_tests.rs": '''#[test]
fn module_test() {
    assert!(true);
}
''',
        })

        self.assertEqual(stats[".rs"]["files"], 3)
        self.assertEqual(stats[".rs"]["test_files"], 3)
        self.assertEqual(stats[".rs"]["test_code"], 13)
        self.assertEqual(stats[".rs"]["production_code"], 1)


if __name__ == "__main__":
    unittest.main()
