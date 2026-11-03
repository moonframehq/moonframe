"""Resolve the source layers of a sealed app, then stage only its closure."""

from pathlib import Path
import re
import shutil
import subprocess
import tomllib

from sealed_luau import references

NATIVES = {"acp": "acp", "chat": "chat", "docs": "docs", "git": "git",
           "github": "forge", "gitlab": "forge", "shell": "shell", "usage": "usage"}
FEATURES = set(NATIVES.values()) | {"alacritty", "browser", "ghostty", "herdr-core",
                                  "map", "markdown", "terminal", "tokio", "wasm"}
SKIP = {".git", ".claude", ".DS_Store", ".moonframe-embedded", "target", "types",
        "__pycache__", ".luaurc"}


def manifest(path):
    with path.open("rb") as source:
        return tomllib.load(source)


def names(value, label):
    if not isinstance(value, list) or any(not isinstance(v, str) or not re.fullmatch(r"[a-z0-9_-]+", v) for v in value):
        raise ValueError(f"{label} must be an array of plain names")
    return set(value)


def profile_paths(user, profile):
    names([profile], "profile")
    folder = user / "profiles" / profile
    entries = manifest(folder / "packages.toml").get("package", {})
    packages, apps = [], []
    for key, entry in sorted(entries.items()):
        if not entry.get("enabled", True):
            continue
        path = Path(entry["url"]) if entry.get("link") else folder / "packages" / key
        path = path.resolve(strict=True)
        data = manifest(path / "moonframe.toml") if (path / "moonframe.toml").is_file() else {}
        (apps if "app" in data else packages).append(path)
    if len(apps) != 1:
        raise ValueError(f"profile {profile} must have exactly one enabled app package")
    return apps[0], packages


def local_packages(app):
    """Direct folder builds accept local requirements; installed ones use profiles."""
    packages, visiting = [], set()

    def visit(folder):
        folder = folder.resolve(strict=True)
        if folder in visiting:
            raise ValueError(f"cyclic package requirement: {folder}")
        visiting.add(folder)
        file = folder / "moonframe.toml"
        for source in manifest(file).get("app", {}).get("requires", []) if file.is_file() else []:
            source = source.strip()
            if not source.startswith(("./", "../")):
                raise ValueError(f"{source}: install/link the app and bundle its profile to resolve remote packages")
            path = (folder / source).resolve(strict=True)
            git = subprocess.run(["git", "-C", str(folder), "rev-parse", "--show-toplevel"],
                                 capture_output=True, text=True)
            root = Path(git.stdout.strip()).resolve() if git.returncode == 0 else folder
            if not path.is_relative_to(root):
                raise ValueError(f"{source} leaves the app's repository, {root}")
            visit(path)
            if path not in packages:
                packages.append(path)
        visiting.remove(folder)

    visit(app)
    # The normal profile loader applies package ids in lexical order.
    return sorted(packages, key=lambda path: path.name)


class Plan:
    def __init__(self, builtin, app, packages):
        self.builtin, self.app = builtin.resolve(), app.resolve()
        # Bundled packages load by folder name, just like paths::bundled.
        self.packages = sorted((p.resolve() for p in packages), key=lambda path: path.name)
        if len({p.name for p in packages}) != len(packages):
            raise ValueError("required packages need distinct folder names")
        data = manifest(self.app / "moonframe.toml")
        self.metadata = data["app"]
        if not isinstance(self.metadata, dict):
            raise ValueError("moonframe.toml needs an [app] table")
        names([self.metadata["name"]], "app.name")
        title = self.metadata.get("title", self.metadata["name"])
        if not isinstance(title, str) or not title or len(title) > 64 or title.startswith(".") or any(ord(c) < 32 or c in '/\\<>&:' for c in title):
            raise ValueError("app.title must be a plain bundle name, up to 64 characters")
        icon = self.metadata.get("icon", "icon.png")
        if not isinstance(icon, str) or Path(icon).is_absolute() or ".." in Path(icon).parts:
            raise ValueError("app.icon must name a file inside the app folder")
        self.config = data.get("bundle", {})
        if not isinstance(self.config, dict):
            raise ValueError("bundle must be a TOML table")
        unknown = self.config.keys() - {"components", "providers", "features", "dynamic"}
        if unknown:
            raise ValueError(f"unknown bundle settings: {sorted(unknown)}")
        self.dynamic = self.config.get("dynamic", {})
        if not isinstance(self.dynamic, dict) or self.dynamic.keys() - {"components", "providers", "features"}:
            raise ValueError("bundle.dynamic accepts components, providers and features arrays")
        self.chosen = {key: names(self.config.get(key, []), f"bundle.{key}") |
                       names(self.dynamic.get(key, []), f"bundle.dynamic.{key}")
                       for key in ("components", "providers", "features")}
        self.layers = [self.builtin, *self.packages, self.app]
        self.entries = {"components": {}, "providers": {}, "mods": {}}
        keep = names(self.metadata.get("keep", []), "app.keep")
        for layer in self.layers:
            if layer in self.packages and (layer / "main.luau").is_file():
                name = re.sub(r"[^a-z0-9_-]", "-", layer.name.lower())
                self.entries["mods"][name] = layer
                continue
            for path in sorted((layer / "components").glob("*.luau")):
                self.entries["components"][path.stem] = path
            for path in sorted((layer / "providers").glob("*/provider.toml")):
                self.entries["providers"][path.parent.name] = path.parent
            for path in sorted((layer / "mods").glob("*/main.luau")):
                if layer != self.builtin or path.parent.name in keep:
                    self.entries["mods"][path.parent.name] = path.parent
        missing = keep - self.entries["mods"].keys()
        if missing:
            raise ValueError(f"app.keep names missing mods: {sorted(missing)}")
        self.scanned, self.hints = set(), []
        self.scan(self.app / "main.luau")
        for layer in self.layers:
            single_mod = layer in self.packages and (layer / "main.luau").is_file()
            if not single_mod and (layer / "style.luau").is_file():
                self.scan(layer / "style.luau")
        for folder in self.entries["mods"].values():
            self.scan(folder / "main.luau")
        # Components may import components and read providers in any branch.
        done = set()
        while pending := self.chosen["components"] - done:
            for name in sorted(pending):
                file = self.require("components", name)
                self.scan(file)
                done.add(name)
        # Always-start providers are observable even without an app read.
        for name, folder in self.entries["providers"].items():
            if manifest(folder / "provider.toml").get("start") == "always":
                self.chosen["providers"].add(name)
        for name in sorted(self.chosen["providers"]):
            folder = self.require("providers", name)
            kind = manifest(folder / "provider.toml").get("kind")
            if kind == "native":
                if name == "demos":
                    raise ValueError("the demos provider needs the development tools omitted by sealed builds")
                if name not in NATIVES:
                    raise ValueError(f"native provider {name} is not part of the starter; bundle it from a custom Rust host")
                self.chosen["features"].add(NATIVES[name])
            elif kind == "wasm":
                self.chosen["features"].add("wasm")
            elif kind not in ("process", "static"):
                raise ValueError(f"{folder}: unknown provider kind {kind}")
        features = self.chosen["features"]
        if features - FEATURES:
            raise ValueError(f"unknown bundle features: {sorted(features - FEATURES)}")
        if "terminal" in features and not features & {"alacritty", "ghostty"}:
            features.add("alacritty")
        if features & {"alacritty", "ghostty"}:
            features.add("terminal")

    def require(self, category, name):
        try:
            return self.entries[category][name]
        except KeyError:
            raise ValueError(f"missing {category}: {name}") from None

    def scan(self, path):
        if path in self.scanned:
            return
        self.scanned.add(path)
        refs = references(path.read_text(), self.entries["providers"])
        for category in self.chosen:
            self.chosen[category].update(getattr(refs, category))
        for category, line in refs.dynamic:
            if category not in self.dynamic:
                raise ValueError(f"{path}:{line}: computed {category}; declare all possible names in [bundle.dynamic] {category} = [...]")
            self.hints.append(f"{self.label(path)}:{line}: uses bundle.dynamic.{category}")

    def label(self, path):
        for layer in reversed(self.layers):
            if path.is_relative_to(layer):
                label = "app" if layer == self.app else ("toolbox" if layer == self.builtin else f"packages/{layer.name}")
                return f"{label}/{path.relative_to(layer)}"
        raise ValueError(f"source outside build layers: {path}")

    def report(self):
        return {"app": self.metadata["name"], "mode": "sealed",
                **{key: sorted(value) for key, value in self.chosen.items()},
                "mods": sorted(self.entries["mods"]),
                "removed": {**{key: sorted(self.entries[key].keys() - self.chosen[key])
                               for key in ("components", "providers")},
                            "features": sorted(FEATURES - self.chosen["features"]),
                            "builtin_mods": sorted({p.parent.name for p in (self.builtin / "mods").glob("*/main.luau")} - self.entries["mods"].keys())},
                "dynamic_declarations": sorted(set(self.hints)),
                "files_scanned": sorted(self.label(p) for p in self.scanned)}

    def stage(self, resources):
        roots = [(self.builtin, resources / "app"), (self.app, resources / "bundle/app")]
        roots += [(p, resources / "bundle/packages" / p.name) for p in self.packages
                  if not (p / "main.luau").is_file() or p in self.entries["mods"].values()]
        for source, dest in roots:
            dest.mkdir(parents=True)
            for path in sorted(source.iterdir()):
                if path.name in SKIP or path.name in ("mods", "components", "providers"):
                    continue
                if source == self.builtin and path.name != "style.luau":
                    continue
                if source in self.packages and path.name == "main.luau" and source not in self.entries["mods"].values():
                    continue
                self.copy(path, dest / path.name)
            for category in ("mods", "components", "providers"):
                selected = self.entries[category] if category == "mods" else self.chosen[category]
                for name in sorted(selected):
                    path = self.entries[category][name]
                    if path == source:  # A single-mod package's root was copied above.
                        continue
                    if path.parent != source / category:
                        continue
                    relative = path.relative_to(source)
                    self.copy(path, dest / relative)
        return roots

    @staticmethod
    def copy(source, dest):
        dest.parent.mkdir(parents=True, exist_ok=True)
        if source.is_dir():
            shutil.copytree(source, dest, ignore=lambda _, items: set(items) & SKIP)
        else:
            shutil.copy2(source, dest)
