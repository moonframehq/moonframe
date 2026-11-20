"""Regression tests for provider declarations and external app paths."""
import importlib.util
import pathlib
import sys
import unittest

sys.dont_write_bytecode = True
HERE = pathlib.Path(__file__).resolve().parent


def load(name):
    spec = importlib.util.spec_from_file_location(name, HERE / f"{name}.py")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


provider = load("provider-types")
checker = load("luau-types")


class ProviderTypesTests(unittest.TestCase):
    def test_output_stays_inside_target(self):
        for root in (pathlib.Path("/Users/example/.config/moonframe"),
                     pathlib.Path("examples/helpdesk")):
            self.assertEqual(checker.definitions_path(root).parent, checker.OUT)
        self.assertNotEqual(checker.definitions_path(pathlib.Path("a/b-c")),
                            checker.definitions_path(pathlib.Path("a-b/c")))
        self.assertEqual(checker.definitions_path(pathlib.Path("examples/helpdesk")),
                         checker.definitions_path(pathlib.Path("examples/helpdesk").resolve()))

    def test_only_type_references_are_renamed(self):
        body = '{ Message: Published, kind: "Published", other: \"Message\", callback: (Message: Published) -> Message } -- Published\n'
        expected = '{ Message: ProviderFooPublished, kind: "Published", other: \"Message\", callback: (Message: ProviderFooPublished) -> ProviderFooMessage } -- Published\n'
        self.assertEqual(provider.rename_references(body, {"Message", "Published"}, "ProviderFoo", ""), expected)

    def test_comments_and_escaped_literals(self):
        body = r'{ value: "escaped\\\"Published", Message --[[ Published ]] : Published }'
        expected = body.rsplit("Published", 1)[0] + "ProviderFooPublished }"
        self.assertEqual(provider.rename_references(body, {"Message", "Published"}, "ProviderFoo", ""), expected)

    def test_generic_parameters_shadow_aliases(self):
        self.assertEqual(provider.rename_references("{ value: Published, msg: Message }", {"Published", "Message"}, "ProviderFoo", "<Published>"),
                         "{ value: Published, msg: ProviderFooMessage }")


if __name__ == "__main__":
    unittest.main()
