"""Sealed builds preserve dependency closures and reject ambiguous pruning."""

import json
from pathlib import Path
import tempfile
import unittest

from sealed_luau import references
from sealed_plan import Plan, profile_paths


class ScannerTests(unittest.TestCase):
    def test_comments_and_documentation_are_not_imports(self):
        refs = references('''-- use "comment"
--[=[ use "long-comment" ]=]
local note = [=[use "documentation"; app.git]=]
local text = "use \\"also-documentation\\""
local card = use -- comment
  ("card")
ui.markdown { text = note }
ui.map { pins = {} }
''', {"git"})
        self.assertEqual(refs.components, {"card"})
        self.assertFalse(refs.providers)
        self.assertEqual(refs.features, {"map", "markdown"})

    def test_escaped_and_long_literal_imports(self):
        refs = references(r'''use "\x63ard"; use [=[badge]=]; use('\099ode')''', set())
        self.assertEqual(refs.components, {"card", "badge", "code"})

    def test_reads_and_destinations_in_every_branch(self):
        refs = references('''if false then return app.git end
return { to = "provider:docs", subscribe = { "provider/system" }, app["usage"] }
''', {"git", "docs", "system", "usage"})
        self.assertEqual(refs.providers, {"git", "docs", "system", "usage"})

    def test_computed_names_require_hints(self):
        refs = references('use("car" .. suffix); app[name]; ui[element]; local import = use', set())
        self.assertEqual({category for category, _ in refs.dynamic}, {"components", "providers", "features"})
        self.assertFalse(refs.components)

    def test_use_record_fields_are_not_imports(self):
        refs = references('''local msg = { use = template.name }
local options = { title = "Card", use = use "card" }
local action = { title = "Open"; use = template.name }
local literal = { use = "=" }
''', set())
        self.assertEqual(refs.components, {"card"})
        self.assertFalse(refs.dynamic)
        # Equality reads the importer, even immediately after a table opener.
        self.assertTrue(references('local same = { use == importer }', set()).dynamic)

    def test_interpolation_scans_expressions_but_not_text(self):
        refs = references('ui.text `app.git {app.system.cpu} {({ nested = app.usage }).nested}`', {"git", "system", "usage"})
        self.assertEqual(refs.providers, {"system", "usage"})
        refs = references('return { to = `provider:{name}` }', set())
        self.assertEqual(refs.dynamic[0][0], "providers")

    def test_terminal_engines_are_selected_per_constructor(self):
        ghostty = 'ui.terminal { id = "a", engine = "ghostty" }'
        self.assertEqual(references(ghostty, set()).features, {"terminal", "ghostty"})
        both = references(ghostty + '; ui.terminal { id = "b" }', set())
        self.assertEqual(both.features, {"terminal", "ghostty", "alacritty"})


class PlanTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name).resolve()
        self.base = self.root / "builtin"
        self.app = self.root / "app"
        self.write(self.app / "moonframe.toml", '[app]\nname = "test"\nkeep = ["status"]\n')
        self.write(self.app / "main.luau", 'local card = use "card"; return mod {}')
        self.write(self.base / "style.luau", 'return style {}')
        self.write(self.base / "mods/status/main.luau", 'return mod { view = function(_, app) return app.git end }')
        self.write(self.base / "mods/unused/main.luau", 'use "missing"')
        self.write(self.base / "components/card.luau", 'local badge = use "badge"; return component {}')
        self.write(self.base / "components/badge.luau", 'return component {}')
        self.write(self.base / "components/unused.luau", 'use "missing"')
        self.write(self.base / "components/demos/card.luau", 'use "missing-demo-dependency"')
        self.write(self.base / "providers/git/provider.toml", 'kind = "native"')
        self.write(self.base / "providers/system/provider.toml", 'kind = "wasm"')

    @staticmethod
    def write(path, text):
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(text)

    def plan(self, packages=()):
        return Plan(self.base, self.app, list(packages))

    def test_transitive_closure_and_native_feature(self):
        plan = self.plan()
        self.assertEqual(plan.chosen["components"], {"card", "badge"})
        self.assertEqual(plan.chosen["providers"], {"git"})
        self.assertEqual(plan.chosen["features"], {"git"})
        self.assertNotIn("wasm", plan.chosen["features"])

    def test_writing_inputs_and_release_metadata_keep_their_capabilities(self):
        for props in ('{ prose = true }', '{ dim = mode }', '{ typewriter = true }',
                      'props', '{ [key] = value }'):
            with self.subTest(props=props):
                self.write(self.app / "main.luau", f'return ui.input({props})')
                self.assertIn("prose", self.plan().chosen["features"])
        self.write(self.app / "main.luau", 'return ui.input { multiline = true }')
        self.assertNotIn("prose", self.plan().chosen["features"])
        with (self.app / "moonframe.toml").open("a") as f:
            f.write('updates = "owner/test"\n')
        self.assertIn("updates", self.plan().chosen["features"])

    def test_staging_removes_unused_fallbacks_and_development_files(self):
        self.write(self.app / "components/card.luau", 'return component {}')
        self.write(self.app / "providers/git/provider.toml", 'kind = "static"\nfile = "data.json"')
        self.write(self.app / "providers/git/data.json", '{}')
        self.write(self.app / ".git/config", "private")
        self.write(self.app / "assets/logo.svg", "<svg/>")
        plan = self.plan()
        out = self.root / "resources"
        plan.stage(out)
        self.assertFalse((out / "app/components/card.luau").exists())
        self.assertFalse((out / "app/components/badge.luau").exists())
        self.assertFalse((out / "app/components/demos").exists())
        self.assertFalse((out / "app/providers").exists())
        self.assertFalse((out / "bundle/app/.git").exists())
        self.assertTrue((out / "bundle/app/components/card.luau").exists())
        self.assertTrue((out / "bundle/app/assets/logo.svg").exists())
        self.assertTrue((out / "bundle/app/providers/git/data.json").exists())
        self.assertNotIn("git", plan.chosen["features"])

    def test_required_package_is_below_app_and_above_toolbox(self):
        package = self.root / "shared"
        self.write(package / "components/card.luau", 'return component { build = function() return ui.browser {} end }')
        self.assertEqual(self.plan([package]).chosen["features"], {"git", "browser"})
        self.write(self.app / "components/card.luau", 'return component {}')
        self.assertEqual(self.plan([package]).chosen["features"], {"git"})

    def test_dynamic_declarations_seed_and_follow_dependencies(self):
        self.write(self.app / "main.luau", 'local item = use(name)')
        with self.assertRaisesRegex(ValueError, "computed components"):
            self.plan()
        with (self.app / "moonframe.toml").open("a") as f:
            f.write('\n[bundle.dynamic]\ncomponents = ["card"]\n')
        self.assertEqual(self.plan().chosen["components"], {"card", "badge"})

    def test_always_start_and_explicit_providers(self):
        self.write(self.base / "providers/system/provider.toml", 'kind = "wasm"\nstart = "always"')
        self.assertIn("wasm", self.plan().chosen["features"])

    def test_missing_import_does_not_ship_a_broken_bundle(self):
        self.write(self.app / "main.luau", 'use "missing"')
        with self.assertRaisesRegex(ValueError, "missing components: missing"):
            self.plan()

    def test_unknown_native_cannot_silently_disappear(self):
        self.write(self.app / "providers/custom/provider.toml", 'kind = "native"')
        self.write(self.app / "main.luau", 'return app.custom')
        with self.assertRaisesRegex(ValueError, "custom Rust host"):
            self.plan()

    def test_profile_uses_enabled_packages_in_id_order(self):
        folder = self.root / "user/profiles/test"
        package = self.root / "shared"
        package.mkdir()
        self.write(folder / "packages.toml", f'''[package."local/test"]
url = {json.dumps(str(self.app))}
link = true
[package."local/shared"]
url = {json.dumps(str(package))}
link = true
[package."local/missing"]
url = "missing"
enabled = false
''')
        self.assertEqual(profile_paths(self.root / "user", "test"), (self.app, [package]))


if __name__ == "__main__":
    unittest.main()
